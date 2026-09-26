//! Running an explanation end to end: plan, send, check, and record provenance.

use std::path::Path;

use repodna_core::CancellationToken;
use repodna_core::config::AiConfig;
use repodna_core::hash::StableHasher;
use repodna_core::model::artifact::{RepositoryDna, compute_dna_hash};
use repodna_core::model::metadata::AiUsage;
use repodna_core::time::Timestamp;
use serde::{Deserialize, Serialize};

use crate::context::{Context, ContextOptions, EvidenceItem, select_context};
use crate::error::AiError;
use crate::prompt::{PROMPT_VERSION, Prompt, build_prompt};
use crate::provider::{AiProvider, CompletionRequest};
use crate::response::{Answer, check_answer};
use crate::task::AiTask;
use crate::text::estimate_tokens;

/// Format identifier of explanation documents.
pub const EXPLANATION_FORMAT: &str = "repodna-explanation/1";

/// Prices configured by the user, per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Prices {
    /// Price per million input tokens.
    pub input_per_million: Option<f64>,
    /// Price per million output tokens.
    pub output_per_million: Option<f64>,
}

impl Prices {
    /// Reads the prices from the AI configuration.
    pub fn from_config(config: &AiConfig) -> Self {
        Self {
            input_per_million: config.input_cost_per_million,
            output_per_million: config.output_cost_per_million,
        }
    }

    /// Cost of `input` and `output` tokens, when both prices are configured.
    pub fn cost(&self, input: u64, output: u64) -> Option<f64> {
        let (input_price, output_price) = (self.input_per_million?, self.output_per_million?);
        Some((input as f64 * input_price + output as f64 * output_price) / 1_000_000.0)
    }
}

/// Explanation settings.
#[derive(Debug, Clone, Copy)]
pub struct ExplainOptions<'a> {
    /// Token budget for the evidence.
    pub max_context_tokens: u32,
    /// Output limit.
    pub max_output_tokens: u32,
    /// Checkout to read redacted excerpts from, when excerpts are enabled.
    pub excerpt_root: Option<&'a Path>,
    /// Configured prices for cost estimates.
    pub prices: Prices,
}

/// Everything that would be sent, before anything is sent.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// Task identifier.
    pub task: String,
    /// Task subject, if any.
    pub subject: Option<String>,
    /// Selected evidence.
    pub context: Context,
    /// The exact prompt.
    pub prompt: Prompt,
    /// Estimated input tokens of the whole prompt.
    pub estimated_input_tokens: u32,
    /// Output limit.
    pub max_output_tokens: u32,
    /// Highest possible cost from configured prices, if configured.
    pub estimated_max_cost: Option<f64>,
}

/// Selects the evidence and builds the prompt for `task`.
pub fn plan(
    dna: &RepositoryDna,
    task: &AiTask,
    options: &ExplainOptions<'_>,
) -> Result<Plan, AiError> {
    let context = select_context(
        dna,
        task,
        &ContextOptions {
            max_tokens: options.max_context_tokens,
            excerpt_root: options.excerpt_root,
        },
    )?;
    let prompt = build_prompt(&dna.identity.name, task, &context);
    let estimated_input_tokens = estimate_tokens(&prompt.system) + estimate_tokens(&prompt.user);
    Ok(Plan {
        task: task.id().to_owned(),
        subject: task.subject().map(str::to_owned),
        estimated_max_cost: options.prices.cost(
            u64::from(estimated_input_tokens),
            u64::from(options.max_output_tokens),
        ),
        context,
        prompt,
        estimated_input_tokens,
        max_output_tokens: options.max_output_tokens,
    })
}

/// A cache key for an explanation: identical prompts to the same model share it.
pub fn cache_key(plan: &Plan, provider: &dyn AiProvider) -> String {
    let mut hasher = StableHasher::new();
    hasher
        .str_field(EXPLANATION_FORMAT)
        .str_field(PROMPT_VERSION)
        .str_field(provider.id())
        .str_field(provider.model())
        .str_field(&plan.max_output_tokens.to_string())
        .str_field(&plan.prompt.system)
        .str_field(&plan.prompt.user);
    hasher.finish_hex()[..32].to_owned()
}

/// Token use and cost of one explanation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplanationUsage {
    /// Provider, model, locality, and request count.
    #[serde(flatten)]
    pub ai: AiUsage,
    /// Input tokens (reported by the provider, or estimated).
    pub input_tokens: u64,
    /// Output tokens, when reported.
    pub output_tokens: Option<u64>,
    /// `true` when the token counts are estimates.
    pub tokens_estimated: bool,
    /// Cost from the user's configured prices, when both are configured.
    pub estimated_cost: Option<f64>,
}

/// A generated explanation with its provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Explanation {
    /// Always [`EXPLANATION_FORMAT`].
    pub format: String,
    /// Human-readable title.
    pub title: String,
    /// Task identifier.
    pub task: String,
    /// Task subject, if any.
    pub subject: Option<String>,
    /// Repository name.
    pub repository: String,
    /// Commit analyzed, when known.
    pub revision: Option<String>,
    /// DNA hash of the analyzed snapshot.
    pub dna_hash: String,
    /// When the analysis ran.
    pub analyzed_at: Timestamp,
    /// When the explanation was generated.
    pub generated_at: Timestamp,
    /// The checked answer.
    pub answer: Answer,
    /// The evidence items the answer cites.
    pub evidence: Vec<EvidenceItem>,
    /// Evidence items that were sent.
    pub evidence_sent: usize,
    /// Evidence items left out to fit the budget.
    pub evidence_omitted: usize,
    /// `true` when redacted source excerpts were sent.
    pub excerpts_sent: bool,
    /// Token use and cost.
    pub usage: ExplanationUsage,
    /// How to read the explanation.
    pub notice: String,
    /// RepoDNA version.
    pub tool_version: String,
    /// Prompt format version.
    pub prompt_version: String,
}

/// Sends a planned request and checks the answer.
pub fn run(
    dna: &RepositoryDna,
    task: &AiTask,
    plan: &Plan,
    provider: &dyn AiProvider,
    prices: Prices,
    cancel: Option<&CancellationToken>,
) -> Result<Explanation, AiError> {
    if cancel.is_some_and(CancellationToken::is_cancelled) {
        return Err(AiError::Cancelled);
    }
    let completion = provider.complete(&CompletionRequest {
        system: &plan.prompt.system,
        user: &plan.prompt.user,
        max_output_tokens: plan.max_output_tokens,
        cancel,
    })?;
    let answer = check_answer(&completion.text, &plan.context);
    let evidence = answer
        .cited()
        .into_iter()
        .filter_map(|id| plan.context.item(id).cloned())
        .collect();
    let tokens_estimated = completion.input_tokens.is_none();
    let input_tokens = completion
        .input_tokens
        .unwrap_or_else(|| u64::from(plan.estimated_input_tokens));
    let output_tokens = completion
        .output_tokens
        .or_else(|| tokens_estimated.then(|| u64::from(estimate_tokens(&completion.text))));
    let dna_hash = if dna.fingerprint.dna_hash.is_empty() {
        compute_dna_hash(&dna.structure)
    } else {
        dna.fingerprint.dna_hash.clone()
    };
    Ok(Explanation {
        format: EXPLANATION_FORMAT.to_owned(),
        title: task.title(),
        task: plan.task.clone(),
        subject: plan.subject.clone(),
        repository: dna.identity.name.clone(),
        revision: dna.analysis_metadata.revision.clone(),
        dna_hash,
        analyzed_at: dna.analysis_metadata.generated_at,
        generated_at: Timestamp::now_or_source_date_epoch(),
        evidence,
        evidence_sent: plan.context.items.len(),
        evidence_omitted: plan.context.omitted,
        excerpts_sent: plan.context.excerpts,
        usage: ExplanationUsage {
            ai: AiUsage {
                provider: provider.id().to_owned(),
                model: provider.model().to_owned(),
                remote: provider.remote(),
                requests: 1,
            },
            input_tokens,
            output_tokens,
            tokens_estimated,
            estimated_cost: prices.cost(input_tokens, output_tokens.unwrap_or(0)),
        },
        notice: format!(
            "Generated by an AI model ({} / {}) from RepoDNA evidence; it is not part of the deterministic analysis. Statements marked as inference or not supported are not established by the analysis.",
            provider.id(),
            provider.model()
        ),
        tool_version: env!("CARGO_PKG_VERSION").to_owned(),
        prompt_version: PROMPT_VERSION.to_owned(),
        answer,
    })
}

/// Plans, sends, and checks an explanation of `task`.
pub fn explain(
    dna: &RepositoryDna,
    task: &AiTask,
    provider: &dyn AiProvider,
    options: &ExplainOptions<'_>,
    cancel: Option<&CancellationToken>,
) -> Result<Explanation, AiError> {
    let plan = plan(dna, task, options)?;
    run(dna, task, &plan, provider, options.prices, cancel)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::provider::Completion;
    use crate::response::Support;
    use crate::testing::sample;
    use repodna_core::Confidence;
    use std::sync::Mutex;

    /// Replies with fixed text and records the prompts it receives.
    pub(crate) struct Scripted {
        pub reply: String,
        pub seen: Mutex<Vec<String>>,
    }

    impl AiProvider for Scripted {
        fn id(&self) -> &'static str {
            "command"
        }
        fn model(&self) -> &str {
            "scripted"
        }
        fn remote(&self) -> bool {
            false
        }
        fn destination(&self) -> String {
            "test".into()
        }
        fn complete(&self, request: &CompletionRequest<'_>) -> Result<Completion, AiError> {
            self.seen.lock().unwrap().push(request.user.to_owned());
            Ok(Completion {
                text: self.reply.clone(),
                input_tokens: None,
                output_tokens: None,
            })
        }
    }

    pub(crate) fn options() -> ExplainOptions<'static> {
        ExplainOptions {
            max_context_tokens: 6_000,
            max_output_tokens: 2_000,
            excerpt_root: None,
            prices: Prices {
                input_per_million: Some(1.0),
                output_per_million: Some(2.0),
            },
        }
    }

    pub(crate) fn explained() -> Explanation {
        let provider = Scripted {
            reply: r#"{"summary": "A layered Rust library.", "points": [{"text": "Networking code lives in src/net.", "evidence": ["E1", "E3"]}, {"text": "It is probably an HTTP client.", "evidence": ["E3"], "inference": true}], "confidence": "medium", "limitations": ["No tests were described."]}"#.into(),
            seen: Mutex::new(Vec::new()),
        };
        explain(&sample(), &AiTask::Repository, &provider, &options(), None).unwrap()
    }

    #[test]
    fn explains_with_provenance_and_cited_evidence() {
        let explanation = explained();
        assert_eq!(explanation.format, EXPLANATION_FORMAT);
        assert_eq!(explanation.title, "Repository explanation");
        assert_eq!(explanation.repository, "widget");
        assert!(explanation.dna_hash.starts_with("rdna1-"));
        assert_eq!(explanation.answer.confidence, Confidence::Medium);
        assert_eq!(explanation.answer.points[1].support, Support::Inference);
        let cited: Vec<&str> = explanation.evidence.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(cited, vec!["E1", "E3"]);
        assert!(explanation.usage.tokens_estimated);
        assert!(explanation.usage.input_tokens > 100);
        assert!(explanation.usage.estimated_cost.unwrap() > 0.0);
        assert_eq!(explanation.usage.ai.requests, 1);
        assert!(!explanation.usage.ai.remote);
        let json = serde_json::to_string(&explanation).unwrap();
        assert!(json.contains("\"tokensEstimated\":true"));
        assert!(json.contains("\"provider\":\"command\""));
        let back: Explanation = serde_json::from_str(&json).unwrap();
        assert_eq!(back, explanation);
    }

    #[test]
    fn plans_without_sending_and_keys_the_cache_by_prompt() {
        let dna = sample();
        let first = plan(&dna, &AiTask::Architecture, &options()).unwrap();
        let provider = Scripted {
            reply: String::new(),
            seen: Mutex::new(Vec::new()),
        };
        assert!(provider.seen.lock().unwrap().is_empty());
        assert!(first.estimated_input_tokens > 0);
        assert!(first.estimated_max_cost.is_some());
        let again = plan(&dna, &AiTask::Architecture, &options()).unwrap();
        assert_eq!(cache_key(&first, &provider), cache_key(&again, &provider));
        let other = plan(&dna, &AiTask::History, &options()).unwrap();
        assert_ne!(cache_key(&first, &provider), cache_key(&other, &provider));
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            run(
                &dna,
                &AiTask::Architecture,
                &first,
                &provider,
                Prices::default(),
                Some(&cancel)
            ),
            Err(AiError::Cancelled)
        ));
        assert_eq!(Prices::default().cost(10, 10), None);
    }
}
