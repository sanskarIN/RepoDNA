//! Optional AI explanations, with a local cache so an identical question to the same model
//! is answered once.

use std::path::{Path, PathBuf};

use repodna_ai::explain::run;
use repodna_ai::provider::{AiProvider, build_provider, max_output_tokens};
use repodna_ai::{AiTask, ExplainOptions, Explanation, Plan, Prices, cache_key, plan};
use repodna_core::CancellationToken;
use repodna_core::config::Config;
use repodna_core::io::write_atomic;
use repodna_core::model::artifact::RepositoryDna;

use crate::error::AppError;
use crate::paths::AppPaths;

/// What to explain and how.
#[derive(Debug, Clone)]
pub struct ExplainRequest<'a> {
    /// The task.
    pub task: AiTask,
    /// Allow providers that send data off this machine (in addition to
    /// `privacy.remote_ai`).
    pub allow_remote: bool,
    /// Reuse and store cached explanations.
    pub use_cache: bool,
    /// Checkout to read redacted excerpts from; used only when
    /// `ai.include_source_excerpts` is on.
    pub checkout: Option<&'a Path>,
}

/// A generated or cached explanation.
#[derive(Debug, Clone)]
pub struct ExplainOutcome {
    /// The explanation.
    pub explanation: Explanation,
    /// `true` when it came from the cache.
    pub cached: bool,
    /// Where it is cached.
    pub cache_path: Option<PathBuf>,
}

fn options<'a>(config: &Config, checkout: Option<&'a Path>) -> ExplainOptions<'a> {
    ExplainOptions {
        max_context_tokens: config.ai.max_context_tokens,
        max_output_tokens: max_output_tokens(&config.ai),
        excerpt_root: checkout.filter(|_| config.ai.include_source_excerpts),
        prices: Prices::from_config(&config.ai),
    }
}

/// Shows what would be sent without contacting any provider.
pub fn plan_explanation(
    config: &Config,
    dna: &RepositoryDna,
    request: &ExplainRequest<'_>,
) -> Result<Plan, AppError> {
    Ok(plan(
        dna,
        &request.task,
        &options(config, request.checkout),
    )?)
}

/// The configured provider, if it can be built (used to describe where data would go).
pub fn provider(config: &Config, allow_remote: bool) -> Result<Box<dyn AiProvider>, AppError> {
    Ok(build_provider(
        &config.ai,
        allow_remote || config.privacy.remote_ai,
    )?)
}

/// Explains `request.task` for `dna`.
pub fn explain(
    paths: &AppPaths,
    config: &Config,
    dna: &RepositoryDna,
    request: &ExplainRequest<'_>,
    cancel: &CancellationToken,
) -> Result<ExplainOutcome, AppError> {
    let provider = provider(config, request.allow_remote)?;
    let options = options(config, request.checkout);
    let plan = plan(dna, &request.task, &options)?;
    let cache_path = request.use_cache.then(|| {
        paths
            .explanation_dir()
            .join(format!("{}.json", cache_key(&plan, provider.as_ref())))
    });
    if let Some(path) = &cache_path
        && let Ok(text) = std::fs::read_to_string(path)
        && let Ok(explanation) = serde_json::from_str::<Explanation>(&text)
    {
        return Ok(ExplainOutcome {
            explanation,
            cached: true,
            cache_path,
        });
    }
    let explanation = run(
        dna,
        &request.task,
        &plan,
        provider.as_ref(),
        options.prices,
        Some(cancel),
    )?;
    if let Some(path) = &cache_path
        && let Ok(json) = serde_json::to_string_pretty(&explanation)
    {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // The cache is an optimization; failing to write it never fails the command.
        let _ = write_atomic(path, json.as_bytes());
    }
    Ok(ExplainOutcome {
        explanation,
        cached: false,
        cache_path,
    })
}

/// Removes every cached explanation; returns how many were removed.
pub fn clear_explanations(paths: &AppPaths) -> Result<usize, AppError> {
    let dir = paths.explanation_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(0);
    };
    let mut removed = 0;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            std::fs::remove_file(&path).map_err(|error| {
                AppError::storage(format!("cannot remove {}: {error}", path.display()))
            })?;
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use repodna_core::config::AiProviderKind;
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::metadata::AnalysisMetadata;

    fn dna() -> RepositoryDna {
        RepositoryDna::new(
            RepositoryIdentity {
                name: "widget".into(),
                ..RepositoryIdentity::default()
            },
            AnalysisMetadata::default(),
        )
    }

    #[test]
    fn explains_with_a_local_command_and_caches_the_answer() {
        let home = tempfile::tempdir().unwrap();
        let paths = AppPaths::in_directory(home.path());
        let mut config = Config::default();
        config.ai.provider = AiProviderKind::Command;
        config.ai.command = vec![
            "sh".into(),
            "-c".into(),
            r#"cat >/dev/null; printf '%s' '{"summary":"A widget.","points":[{"text":"Named widget.","evidence":["E1"]}],"confidence":"high","limitations":[]}'"#.into(),
        ];
        let request = ExplainRequest {
            task: AiTask::Repository,
            allow_remote: false,
            use_cache: true,
            checkout: None,
        };
        let cancel = CancellationToken::new();
        let first = explain(&paths, &config, &dna(), &request, &cancel).unwrap();
        assert!(!first.cached);
        assert_eq!(first.explanation.answer.summary, "A widget.");
        let second = explain(&paths, &config, &dna(), &request, &cancel).unwrap();
        assert!(second.cached);
        assert_eq!(second.explanation, first.explanation);
        assert_eq!(clear_explanations(&paths).unwrap(), 1);

        let planned = plan_explanation(&config, &dna(), &request).unwrap();
        assert!(planned.prompt.user.contains("widget"));

        config.ai.provider = AiProviderKind::None;
        let error = explain(&paths, &config, &dna(), &request, &cancel).unwrap_err();
        assert_eq!(error.exit_code(), 4);
    }
}
