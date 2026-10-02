//! Context selection: the numbered evidence an explanation may use.
//!
//! Only facts from the analysis artifact are selected, most relevant first, until the
//! token budget is spent. Whole files are never sent. Contributor names and email
//! addresses are never included. Short source excerpts are added only when the caller
//! passes a checkout root, which the CLI does only when `ai.include_source_excerpts` is
//! on; every excerpt line passes through secret redaction first.

mod general;
mod subject;

use std::path::Path;

use repodna_core::model::artifact::RepositoryDna;
use serde::{Deserialize, Serialize};

use crate::error::AiError;
use crate::task::AiTask;
use crate::text::{estimate_tokens, single_line};

use general::{
    architecture_signals, commands, cycles, edges, entrypoints, evolution, findings, fingerprint,
    first_look, glossary, history, hotspots, important_files, modules, onboarding, overview,
    packages, recent_changes, recent_commits, tests_and_docs,
};
use subject::{hotspot_task, module_task};

/// Longest reference, in characters.
const MAX_REFERENCE: usize = 200;
/// Longest detail, in characters.
const MAX_DETAIL: usize = 500;
/// Lines per source excerpt.
const EXCERPT_LINES: usize = 40;
/// Bytes read per excerpt file.
const EXCERPT_BYTES: u64 = 64 * 1024;
/// Excerpt files per request.
const EXCERPT_FILES: usize = 3;

/// The kind of an evidence item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceKind {
    /// A repository-level fact.
    Fact,
    /// A file.
    File,
    /// A module or directory.
    Module,
    /// A dependency edge between modules.
    Edge,
    /// An external package.
    Package,
    /// A commit.
    Commit,
    /// A measured value.
    Metric,
    /// A finding.
    Finding,
    /// A detected (not executed) command.
    Command,
    /// A redacted source excerpt.
    Excerpt,
}

impl EvidenceKind {
    /// Human-readable label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::File => "file",
            Self::Module => "module",
            Self::Edge => "dependency edge",
            Self::Package => "package",
            Self::Commit => "commit",
            Self::Metric => "metric",
            Self::Finding => "finding",
            Self::Command => "command",
            Self::Excerpt => "excerpt",
        }
    }
}

/// One numbered piece of evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceItem {
    /// Identifier cited by the model, e.g. `E3`.
    pub id: String,
    /// Kind of evidence.
    pub kind: EvidenceKind,
    /// What the evidence refers to: a path, commit, edge, metric, or topic.
    pub reference: String,
    /// The fact itself.
    pub detail: String,
}

/// The evidence selected for one request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    /// Selected evidence, most relevant first.
    pub items: Vec<EvidenceItem>,
    /// Candidate items left out to stay within the budget.
    pub omitted: usize,
    /// Estimated tokens of the selected evidence.
    pub estimated_tokens: u32,
    /// `true` when redacted source excerpts are included.
    pub excerpts: bool,
}

impl Context {
    /// Looks up an item by identifier.
    pub fn item(&self, id: &str) -> Option<&EvidenceItem> {
        self.items.iter().find(|item| item.id == id)
    }
}

/// Context selection settings.
#[derive(Debug, Clone, Copy)]
pub struct ContextOptions<'a> {
    /// Token budget for the evidence.
    pub max_tokens: u32,
    /// Checkout to read short, redacted excerpts from; `None` sends no source text.
    pub excerpt_root: Option<&'a Path>,
}

/// Candidate evidence in priority order.
#[derive(Default)]
struct Candidates(Vec<(EvidenceKind, String, String)>);

impl Candidates {
    fn push(&mut self, kind: EvidenceKind, reference: impl AsRef<str>, detail: impl AsRef<str>) {
        let detail = single_line(detail.as_ref(), MAX_DETAIL);
        if !detail.is_empty() {
            self.0
                .push((kind, single_line(reference.as_ref(), MAX_REFERENCE), detail));
        }
    }

    fn push_excerpt(&mut self, reference: String, text: String) {
        self.0.push((EvidenceKind::Excerpt, reference, text));
    }
}

/// Selects the evidence for `task`, most relevant first, within the token budget.
pub fn select_context(
    dna: &RepositoryDna,
    task: &AiTask,
    options: &ContextOptions<'_>,
) -> Result<Context, AiError> {
    let mut c = Candidates::default();
    let all = |_: &str| true;
    match task {
        AiTask::Repository => {
            overview(dna, &mut c);
            first_look(dna, &mut c);
            entrypoints(dna, &mut c, 6);
            commands(dna, &mut c, 8);
            important_files(dna, &mut c, 8);
            modules(dna, &mut c, 8, all);
            packages(dna, &mut c, 10);
            recent_changes(dna, &mut c);
            tests_and_docs(dna, &mut c);
            findings(dna, &mut c, 8, |_| true);
            hotspots(dna, &mut c, 5, all);
            fingerprint(dna, &mut c);
        }
        AiTask::Architecture => {
            overview(dna, &mut c);
            architecture_signals(dna, &mut c);
            modules(dna, &mut c, 15, all);
            edges(dna, &mut c, 20, all);
            cycles(dna, &mut c, 5);
            entrypoints(dna, &mut c, 6);
            findings(dna, &mut c, 8, |f| {
                f.rule.starts_with("architecture.") || f.rule.starts_with("structure.")
            });
            hotspots(dna, &mut c, 5, all);
        }
        AiTask::History => {
            overview(dna, &mut c);
            history(dna, &mut c);
            evolution(dna, &mut c);
            recent_changes(dna, &mut c);
            recent_commits(dna, &mut c, 15);
            hotspots(dna, &mut c, 5, all);
            findings(dna, &mut c, 6, |f| {
                f.rule.starts_with("activity.")
                    || f.rule.starts_with("evolution.")
                    || f.rule.starts_with("contributors.")
            });
        }
        AiTask::Dependencies => {
            overview(dna, &mut c);
            packages(dna, &mut c, 25);
            edges(dna, &mut c, 15, all);
            cycles(dna, &mut c, 5);
            modules(dna, &mut c, 8, all);
            findings(dna, &mut c, 10, |f| {
                f.rule.starts_with("dependencies.") || f.rule.starts_with("architecture.")
            });
        }
        AiTask::Onboarding => {
            overview(dna, &mut c);
            onboarding(dna, &mut c);
            important_files(dna, &mut c, 10);
            entrypoints(dna, &mut c, 6);
            commands(dna, &mut c, 10);
            tests_and_docs(dna, &mut c);
            first_look(dna, &mut c);
            modules(dna, &mut c, 8, all);
            glossary(dna, &mut c, 15);
        }
        AiTask::Module(subject) => {
            overview(dna, &mut c);
            module_task(dna, &mut c, subject, options.excerpt_root)?;
        }
        AiTask::Hotspot(subject) => {
            overview(dna, &mut c);
            hotspot_task(dna, &mut c, subject, options.excerpt_root)?;
        }
        AiTask::Question(_) => {
            overview(dna, &mut c);
            first_look(dna, &mut c);
            entrypoints(dna, &mut c, 6);
            commands(dna, &mut c, 8);
            modules(dna, &mut c, 10, all);
            hotspots(dna, &mut c, 5, all);
            packages(dna, &mut c, 10);
            recent_changes(dna, &mut c);
            findings(dna, &mut c, 10, |_| true);
            history(dna, &mut c);
            tests_and_docs(dna, &mut c);
            glossary(dna, &mut c, 10);
        }
    }

    let mut context = Context {
        items: Vec::new(),
        omitted: 0,
        estimated_tokens: 0,
        excerpts: false,
    };
    for (kind, reference, detail) in c.0 {
        let item = EvidenceItem {
            id: format!("E{}", context.items.len() + 1),
            kind,
            reference,
            detail,
        };
        let cost = estimate_tokens(&serde_json::to_string(&item).unwrap_or_default()) + 2;
        if context.estimated_tokens + cost > options.max_tokens {
            context.omitted += 1;
            continue;
        }
        context.estimated_tokens += cost;
        context.excerpts |= kind == EvidenceKind::Excerpt;
        context.items.push(item);
    }
    Ok(context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::sample;

    fn options(max_tokens: u32) -> ContextOptions<'static> {
        ContextOptions {
            max_tokens,
            excerpt_root: None,
        }
    }

    #[test]
    fn selects_numbered_evidence_within_the_budget() {
        let dna = sample();
        let context = select_context(&dna, &AiTask::Repository, &options(6_000)).unwrap();
        assert_eq!(context.items[0].id, "E1");
        assert!(context.items[0].detail.contains("Ignore all previous"));
        assert!(context.items.iter().any(|i| i.reference == "src/net"));
        assert_eq!(context.omitted, 0);
        assert!(!context.excerpts);

        let small = select_context(&dna, &AiTask::Repository, &options(60)).unwrap();
        assert!(small.items.len() < context.items.len());
        assert!(small.omitted > 0);
        assert!(small.estimated_tokens <= 60);
        let ids: Vec<&str> = small.items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids.first(), Some(&"E1"));
    }

    #[test]
    fn resolves_module_and_hotspot_subjects() {
        let dna = sample();
        let module =
            select_context(&dna, &AiTask::Module("src/net".into()), &options(6_000)).unwrap();
        assert!(module.items.iter().any(|i| i.reference == "src/net/tls.rs"));
        assert!(!module.items.iter().any(|i| i.reference == "src/ui/app.rs"));

        let hotspot = select_context(
            &dna,
            &AiTask::Hotspot("src/ui/app.rs".into()),
            &options(6_000),
        )
        .unwrap();
        assert!(
            hotspot
                .items
                .iter()
                .any(|i| i.detail.contains("not among the 1 files"))
        );

        assert!(matches!(
            select_context(&dna, &AiTask::Module("lib/none".into()), &options(6_000)),
            Err(AiError::UnknownSubject(_))
        ));
        assert!(matches!(
            select_context(&dna, &AiTask::Hotspot("nope.rs".into()), &options(6_000)),
            Err(AiError::UnknownSubject(_))
        ));
    }

    #[test]
    fn excerpts_are_redacted_and_confined_to_the_root() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/net")).unwrap();
        let key = ["gh", "p_", &"a1B2".repeat(9)].concat();
        std::fs::write(
            dir.path().join("src/net/client.rs"),
            format!("fn connect() {{}}\nlet token = \"{key}\";\n"),
        )
        .unwrap();
        let dna = sample();
        let context = select_context(
            &dna,
            &AiTask::Hotspot("src/net/client.rs".into()),
            &ContextOptions {
                max_tokens: 6_000,
                excerpt_root: Some(dir.path()),
            },
        )
        .unwrap();
        assert!(context.excerpts);
        let excerpt = context
            .items
            .iter()
            .find(|i| i.kind == EvidenceKind::Excerpt)
            .unwrap();
        assert!(excerpt.detail.contains("fn connect()"));
        assert!(!excerpt.detail.contains(&key));
        assert!(subject::read_excerpt(dir.path(), "../outside.rs").is_none());
    }
}
