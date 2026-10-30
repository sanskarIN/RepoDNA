//! A concise summary for continuous integration, with an optional failure policy.
//!
//! RepoDNA findings are analysis signals, not formal guarantees; the policy only turns the
//! signals a team chooses into a failing exit status.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use repodna_core::finding::Finding;
use repodna_core::model::artifact::RepositoryDna;
use repodna_core::severity::Severity;

use crate::text::{counted, thousands};

/// When a CI run should fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CiPolicy {
    /// Fail when an active finding has at least this severity.
    pub fail_on: Option<Severity>,
    /// With a baseline, consider only findings that are new since the baseline.
    pub new_only: bool,
}

/// The result of evaluating an artifact for CI.
#[derive(Debug, Clone, PartialEq)]
pub struct CiOutcome {
    /// Labeled values in display order.
    pub values: Vec<(String, String)>,
    /// Findings that count toward the policy, most severe first, as `(severity, title)`.
    pub failing: Vec<(Severity, String)>,
    /// `true` when the policy failed.
    pub failed: bool,
    /// Typed, stable impact details when a baseline artifact was supplied.
    pub changes: Option<CiChanges>,
}

/// Paths changed relative to the baseline, sorted for reproducible CI output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiFileChanges {
    /// Paths present only in the current snapshot.
    pub added: Vec<String>,
    /// Paths present only in the baseline snapshot.
    pub removed: Vec<String>,
    /// Paths whose recorded content hashes changed.
    pub modified: Vec<String>,
}

/// Compact evidence references for a newly introduced or resolved finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiFindingChange {
    /// Stable finding identifier, suitable for matching across runs.
    pub id: String,
    /// Analyzer rule that produced this finding.
    pub rule: String,
    /// Finding urgency according to RepoDNA's severity criteria.
    pub severity: Severity,
    /// Short human-readable description; may contain repository-controlled text.
    pub title: String,
    /// Repository-relative paths associated with the finding.
    pub paths: Vec<String>,
}

/// Structured CI impact, available only when --baseline is provided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiChanges {
    /// Sorted file-path additions, removals, and modifications.
    pub files: CiFileChanges,
    /// Active finding IDs absent among active baseline findings.
    pub introduced: Vec<CiFindingChange>,
    /// Active baseline finding IDs absent among active current findings.
    pub resolved: Vec<CiFindingChange>,
    /// Module impact, absent unless both snapshots have architecture results.
    pub architecture: Option<CiArchitectureImpact>,
}

/// A directed relationship between two modules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiModuleEdge {
    /// Module that depends on another module.
    pub from: String,
    /// Module being depended upon.
    pub to: String,
}

/// Changed modules and one-hop dependency signals for CI review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiArchitectureImpact {
    /// Modules that own added, removed or modified files.
    pub touched_modules: Vec<String>,
    /// Immediate dependents of any touched module from either snapshot.
    pub direct_dependents: Vec<String>,
    /// Newly observed module dependency relationships.
    pub added_edges: Vec<CiModuleEdge>,
    /// Module dependency relationships no longer observed.
    pub removed_edges: Vec<CiModuleEdge>,
}

fn changed_findings<'a>(findings: impl IntoIterator<Item = &'a Finding>) -> Vec<CiFindingChange> {
    let mut seen = HashSet::new();
    let mut changes: Vec<_> = findings
        .into_iter()
        .filter(|finding| seen.insert(finding.id.as_str()))
        .map(|finding| CiFindingChange {
            id: finding.id.clone(),
            rule: finding.rule.clone(),
            severity: finding.severity,
            title: finding.title.clone(),
            paths: finding.paths.clone(),
        })
        .collect();
    changes.sort_by(|a, b| b.severity.cmp(&a.severity).then_with(|| a.id.cmp(&b.id)));
    changes
}

const COMPLEXITY_RULES: [&str; 4] = [
    "quality.complex-function",
    "quality.long-function",
    "quality.deep-nesting",
    "quality.large-file",
];

/// Changed paths between two snapshots. A missing hash is not evidence of a modification.
fn file_changes(baseline: &RepositoryDna, current: &RepositoryDna) -> CiFileChanges {
    let before: BTreeMap<&str, Option<&String>> = baseline
        .structure
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.hash.as_ref()))
        .collect();
    let after: BTreeMap<&str, Option<&String>> = current
        .structure
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.hash.as_ref()))
        .collect();
    let added = after
        .keys()
        .filter(|p| !before.contains_key(*p))
        .map(|path| (*path).to_owned())
        .collect();
    let removed = before
        .keys()
        .filter(|p| !after.contains_key(*p))
        .map(|path| (*path).to_owned())
        .collect();
    let modified = after
        .iter()
        .filter(|(path, hash)| {
            before.get(*path).is_some_and(|old| {
                old.as_ref()
                    .zip(hash.as_ref())
                    .is_some_and(|(before, after)| before != after)
            })
        })
        .map(|(path, _)| (*path).to_owned())
        .collect();
    CiFileChanges {
        added,
        removed,
        modified,
    }
}

/// Match the deepest module path on directory boundaries, not string prefixes.
fn module_for_path<'a>(dna: &'a RepositoryDna, path: &str) -> Option<&'a str> {
    dna.architecture
        .modules
        .iter()
        .filter(|module| {
            let prefix = module.path.trim_end_matches('/');
            prefix.is_empty()
                || path == prefix
                || path
                    .strip_prefix(prefix)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        })
        .max_by_key(|module| module.path.trim_end_matches('/').len())
        .map(|module| module.id.as_str())
}

/// One-hop structural impact only, not a transitive or causal prediction.
fn architecture_impact(
    baseline: &RepositoryDna,
    current: &RepositoryDna,
    files: &CiFileChanges,
) -> Option<CiArchitectureImpact> {
    if !baseline.architecture.status.has_results() || !current.architecture.status.has_results() {
        return None;
    }

    let mut touched = BTreeSet::new();
    // Removed paths exist only in the baseline; added paths only in the
    // current snapshot. For modified paths compare both boundary assignments.
    for path in files.added.iter().chain(&files.modified) {
        if let Some(id) = module_for_path(current, path) {
            touched.insert(id.to_owned());
        }
    }
    for path in files.removed.iter().chain(&files.modified) {
        if let Some(id) = module_for_path(baseline, path) {
            touched.insert(id.to_owned());
        }
    }

    let before: BTreeSet<_> = baseline
        .architecture
        .module_edges
        .iter()
        .map(|edge| (edge.from.as_str(), edge.to.as_str()))
        .collect();
    let after: BTreeSet<_> = current
        .architecture
        .module_edges
        .iter()
        .map(|edge| (edge.from.as_str(), edge.to.as_str()))
        .collect();
    let edge_change = |(from, to): &(&str, &str)| CiModuleEdge {
        from: (*from).to_owned(),
        to: (*to).to_owned(),
    };
    let added_edges = after.difference(&before).map(edge_change).collect();
    let removed_edges = before.difference(&after).map(edge_change).collect();
    let direct_dependents = baseline
        .architecture
        .module_edges
        .iter()
        .chain(&current.architecture.module_edges)
        .filter(|edge| touched.contains(&edge.to) && !touched.contains(&edge.from))
        .map(|edge| edge.from.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    Some(CiArchitectureImpact {
        touched_modules: touched.into_iter().collect(),
        direct_dependents,
        added_edges,
        removed_edges,
    })
}

/// Evaluates an artifact, optionally against a baseline artifact (for example the base
/// branch of a pull request).
pub fn evaluate(
    dna: &RepositoryDna,
    baseline: Option<&RepositoryDna>,
    policy: CiPolicy,
) -> CiOutcome {
    let active: Vec<_> = dna.findings.iter().filter(|f| !f.is_suppressed()).collect();
    // Suppressed baseline findings are not active; reactivating one is an
    // introduced finding and must be eligible for --new-only policy checks.
    let known: HashSet<&str> = baseline
        .map(|b| {
            b.findings
                .iter()
                .filter(|f| !f.is_suppressed())
                .map(|f| f.id.as_str())
                .collect()
        })
        .unwrap_or_default();
    let mut seen_new = HashSet::new();
    let new: Vec<_> = active
        .iter()
        .filter(|f| {
            baseline.is_some() && !known.contains(f.id.as_str()) && seen_new.insert(f.id.as_str())
        })
        .copied()
        .collect();
    let count = |severity: Severity| active.iter().filter(|f| f.severity == severity).count();
    let mut values = vec![("Repository".to_owned(), dna.identity.name.clone())];
    if let Some(revision) = &dna.analysis_metadata.revision {
        values.push(("Revision".to_owned(), revision.chars().take(12).collect()));
    }

    let changes = if let Some(baseline) = baseline {
        let files = file_changes(baseline, dna);
        values.push((
            "Changed files".to_owned(),
            format!(
                "{} ({} added, {} removed, {} modified)",
                files.added.len() + files.removed.len() + files.modified.len(),
                files.added.len(),
                files.removed.len(),
                files.modified.len()
            ),
        ));
        let current: HashSet<&str> = active.iter().map(|f| f.id.as_str()).collect();
        let resolved = baseline
            .findings
            .iter()
            .filter(|f| !f.is_suppressed() && !current.contains(f.id.as_str()));
        let architecture = architecture_impact(baseline, dna, &files);
        Some(CiChanges {
            files,
            introduced: changed_findings(new.iter().copied()),
            resolved: changed_findings(resolved),
            architecture,
        })
    } else {
        if let Some(recent) = &dna.insights.recent_changes {
            values.push((
                // The window ends at the latest commit, not today.
                format!(
                    "Files changed in the last {} days of history",
                    recent.window_days
                ),
                thousands(recent.files_changed),
            ));
        }
        None
    };

    values.push((
        "Complexity signals".to_owned(),
        active
            .iter()
            .filter(|f| COMPLEXITY_RULES.contains(&f.rule.as_str()))
            .count()
            .to_string(),
    ));
    values.push((
        "Dependency cycles".to_owned(),
        if dna.architecture.status.has_results() {
            dna.architecture.cycles.len().to_string()
        } else {
            "not analyzed".to_owned()
        },
    ));
    values.push((
        "Potential secret candidates".to_owned(),
        if dna.security.status.has_results() {
            dna.security.secrets.len().to_string()
        } else {
            "not analyzed".to_owned()
        },
    ));
    values.push((
        "Tests detected".to_owned(),
        if dna.tests.status.has_results() {
            let with_tests = dna.tests.test_files + dna.tests.inline_test_files;
            if with_tests > 0 {
                format!("yes ({})", counted(with_tests, "file", "files"))
            } else {
                "no".to_owned()
            }
        } else {
            "not analyzed".to_owned()
        },
    ));
    values.push((
        "Findings".to_owned(),
        format!(
            "{} critical, {}, {} attention, {} informational",
            count(Severity::Critical),
            counted(count(Severity::Warning) as u64, "warning", "warnings"),
            count(Severity::Attention),
            count(Severity::Info)
        ),
    ));
    if let Some(changes) = &changes {
        values.push((
            "New findings".to_owned(),
            changes.introduced.len().to_string(),
        ));
        values.push((
            "Resolved findings".to_owned(),
            changes.resolved.len().to_string(),
        ));
    }

    let considered = if policy.new_only && baseline.is_some() {
        new
    } else {
        active
    };
    let mut failing: Vec<(Severity, String)> = match policy.fail_on {
        Some(threshold) => considered
            .iter()
            .filter(|f| f.severity >= threshold)
            .map(|f| (f.severity, f.title.clone()))
            .collect(),
        None => Vec::new(),
    };
    failing.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    CiOutcome {
        values,
        failed: !failing.is_empty(),
        failing,
        changes,
    }
}

/// Plain-text summary for terminals and logs.
pub fn text(outcome: &CiOutcome) -> String {
    let width = outcome
        .values
        .iter()
        .map(|(label, _)| label.chars().count())
        .max()
        .unwrap_or(0);
    let mut out = String::from("RepoDNA analysis\n----------------\n");
    for (label, value) in &outcome.values {
        out.push_str(&format!("{label:<width$}  {value}\n"));
    }
    if outcome.failed {
        out.push_str(&format!(
            "\nFailing: {} finding(s) at or above the configured severity:\n",
            outcome.failing.len()
        ));
        for (severity, title) in outcome.failing.iter().take(20) {
            out.push_str(&format!("  [{}] {title}\n", severity.label()));
        }
    }
    out.push_str("\nFindings are analysis signals backed by evidence, not formal guarantees.\n");
    out
}

// Each list has a hard limit: CI summaries must remain reviewable even when
// analyzing large repositories. Escape repository-controlled text in all cases.
fn markdown_entries(
    out: &mut String,
    title: &str,
    entries: impl ExactSizeIterator<Item = String>,
) {
    const LIMIT: usize = 10;
    const MAX_CHARS: usize = 180;
    let total = entries.len();
    if total == 0 {
        return;
    }
    out.push_str(&format!("\n**{title} ({total})**\n\n"));
    for value in entries.take(LIMIT) {
        let excerpt: String = value.chars().take(MAX_CHARS).collect();
        out.push_str("- ");
        out.push_str(&crate::text::escape_markdown(&excerpt));
        if value.chars().count() > MAX_CHARS {
            out.push('…');
        }
        out.push('\n');
    }
    if total > LIMIT {
        out.push_str(&format!("- … and {} more\n", total - LIMIT));
    }
}

/// Markdown summary, for example for $GITHUB_STEP_SUMMARY.
pub fn markdown(outcome: &CiOutcome) -> String {
    let mut out = String::from("### RepoDNA analysis\n\n| Signal | Value |\n| --- | --- |\n");
    for (label, value) in &outcome.values {
        out.push_str(&format!(
            "| {} | {} |\n",
            crate::text::escape_markdown(label),
            crate::text::escape_markdown(value)
        ));
    }
    if let Some(changes) = &outcome.changes {
        out.push_str("\n#### Baseline impact\n\n");
        out.push_str(&format!(
            "- Files: {} added, {} removed, {} modified\n",
            changes.files.added.len(),
            changes.files.removed.len(),
            changes.files.modified.len()
        ));
        out.push_str(&format!(
            "- Findings: {} introduced, {} resolved\n",
            changes.introduced.len(),
            changes.resolved.len()
        ));
        if let Some(architecture) = &changes.architecture {
            out.push_str(&format!(
                "- Modules: {} touched, {} immediate dependents\n",
                architecture.touched_modules.len(),
                architecture.direct_dependents.len()
            ));
        } else {
            out.push_str("- Module impact: not analyzed in both snapshots\n");
        }
        markdown_entries(
            &mut out,
            "Added files",
            changes.files.added.iter().cloned(),
        );
        markdown_entries(
            &mut out,
            "Removed files",
            changes.files.removed.iter().cloned(),
        );
        markdown_entries(
            &mut out,
            "Modified files",
            changes.files.modified.iter().cloned(),
        );
        markdown_entries(
            &mut out,
            "Introduced findings",
            changes.introduced.iter().map(|f| {
                format!("{} [{}; {}] ({})", f.title, f.severity.label(), f.rule, f.id)
            }),
        );
        markdown_entries(
            &mut out,
            "Resolved findings",
            changes.resolved.iter().map(|f| {
                format!("{} [{}; {}] ({})", f.title, f.severity.label(), f.rule, f.id)
            }),
        );
        if let Some(architecture) = &changes.architecture {
            markdown_entries(
                &mut out,
                "Touched modules",
                architecture.touched_modules.iter().cloned(),
            );
            markdown_entries(
                &mut out,
                "Immediate dependents",
                architecture.direct_dependents.iter().cloned(),
            );
            markdown_entries(
                &mut out,
                "Added module edges",
                architecture
                    .added_edges
                    .iter()
                    .map(|edge| format!("{} → {}", edge.from, edge.to)),
            );
            markdown_entries(
                &mut out,
                "Removed module edges",
                architecture
                    .removed_edges
                    .iter()
                    .map(|edge| format!("{} → {}", edge.from, edge.to)),
            );
        }
        out.push_str("\n_Impact reflects observed snapshots, not proven causation. Paths and finding titles may contain sensitive repository content._\n");
    }
    if outcome.failed {
        out.push_str(&format!(
            "\n**Failing:** {} finding(s) at or above the configured severity.\n\n",
            outcome.failing.len()
        ));
        for (severity, title) in outcome.failing.iter().take(20) {
            out.push_str(&format!(
                "- {}: {}\n",
                severity.label(),
                crate::text::escape_markdown(title)
            ));
        }
    }
    out.push_str("\n_Findings are analysis signals backed by evidence, not formal guarantees._\n");
    out
}

/// Machine-readable summary.
pub fn json(outcome: &CiOutcome) -> serde_json::Value {
    let values: serde_json::Map<String, serde_json::Value> = outcome
        .values
        .iter()
        .map(|(label, value)| (label.clone(), serde_json::Value::String(value.clone())))
        .collect();
    let mut result = serde_json::json!({
        "summary": values,
        "failed": outcome.failed,
        "failing": outcome.failing.iter().map(|(severity, title)| serde_json::json!({
            "severity": severity.label(),
            "title": title,
        })).collect::<Vec<_>>(),
    });
    if let Some(changes) = &outcome.changes {
        let finding_json = |f: &CiFindingChange| {
            serde_json::json!({
                "id": f.id,
                "rule": f.rule,
                "severity": f.severity,
                "title": f.title,
                "paths": f.paths,
            })
        };
        result["changes"] = serde_json::json!({
            "files": {
                "added": changes.files.added,
                "removed": changes.files.removed,
                "modified": changes.files.modified,
            },
            "findings": {
                "introduced": changes.introduced.iter().map(finding_json).collect::<Vec<_>>(),
                "resolved": changes.resolved.iter().map(finding_json).collect::<Vec<_>>(),
            }
        });
        if let Some(architecture) = &changes.architecture {
            let edge_json = |edge: &CiModuleEdge| {
                serde_json::json!({
                    "from": edge.from,
                    "to": edge.to,
                })
            };
            result["changes"]["architecture"] = serde_json::json!({
                "touchedModules": architecture.touched_modules,
                "directDependents": architecture.direct_dependents,
                "moduleEdges": {
                    "added": architecture.added_edges.iter().map(edge_json).collect::<Vec<_>>(),
                    "removed": architecture.removed_edges.iter().map(edge_json).collect::<Vec<_>>(),
                }
            });
        }
    }
    result
}

fn escape_data(text: &str) -> String {
    text.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn escape_property(text: &str) -> String {
    escape_data(text).replace(':', "%3A").replace(',', "%2C")
}

/// GitHub Actions workflow commands that annotate findings of at least `minimum` severity
/// on the files they concern.
pub fn github_annotations(dna: &RepositoryDna, minimum: Severity) -> Vec<String> {
    github_annotations_for_changes(dna, minimum, None)
}

/// GitHub annotations optionally limited to introduced findings from a baseline comparison.
///
/// `None` preserves the full annotation set; `Some(&[])` deliberately emits no
/// annotations. The limit applies after filtering so unchanged findings cannot
/// consume the annotation budget. Suppressed findings remain excluded.
pub fn github_annotations_for_changes(
    dna: &RepositoryDna,
    minimum: Severity,
    introduced: Option<&[CiFindingChange]>,
) -> Vec<String> {
    let allowed = introduced.map(|changes| {
        changes
            .iter()
            .map(|finding| finding.id.as_str())
            .collect::<HashSet<_>>()
    });
    let mut seen = HashSet::new();
    dna.findings
        .iter()
        .filter(|finding| {
            !finding.is_suppressed()
                && finding.severity >= minimum
                && allowed
                    .as_ref()
                    .is_none_or(|ids| ids.contains(finding.id.as_str()))
                && seen.insert(finding.id.as_str())
        })
        .take(50)
        .map(|finding| {
            let command = match finding.severity {
                Severity::Critical | Severity::Warning => "warning",
                Severity::Attention | Severity::Info => "notice",
            };
            let location = finding.evidence.iter().find_map(|evidence| {
                let path = evidence.path()?;
                let line = match evidence {
                    repodna_core::evidence::Evidence::File { line, .. } => *line,
                    repodna_core::evidence::Evidence::Symbol { line, .. } => *line,
                    _ => None,
                };
                Some((path.to_owned(), line))
            });
            let mut properties = vec![format!(
                "title={}",
                escape_property(&format!("RepoDNA: {}", finding.rule))
            )];
            if let Some((path, line)) = location {
                properties.push(format!("file={}", escape_property(&path)));
                if let Some(line) = line {
                    properties.push(format!("line={line}"));
                }
            }
            format!(
                "::{command} {}::{}",
                properties.join(","),
                escape_data(&format!("{}. {}", finding.title, finding.summary))
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::confidence::Confidence;
    use repodna_core::evidence::Evidence;
    use repodna_core::finding::{Finding, FindingCategory};
    use repodna_core::model::SectionStatus;
    use repodna_core::model::architecture::{ModuleEdge, ModuleKind, ModuleRecord};
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::metadata::AnalysisMetadata;
    use repodna_core::model::structure::{FileCategory, FileRecord};

    fn finding(subject: &str, severity: Severity) -> Finding {
        Finding::new(
            "architecture.cycle",
            subject,
            FindingCategory::Architecture,
            severity,
            Confidence::High,
            format!("Cycle {subject}"),
        )
        .summary("a, b\ncycle 100%")
        .evidence(Evidence::line("src/a.rs", 7))
    }

    fn artifact(findings: Vec<Finding>, files: &[(&str, &str)]) -> RepositoryDna {
        let mut dna = RepositoryDna::new(
            RepositoryIdentity {
                name: "widget".into(),
                ..RepositoryIdentity::default()
            },
            AnalysisMetadata::default(),
        );
        dna.findings = findings;
        dna.tests.status = SectionStatus::Analyzed;
        dna.tests.test_files = 3;
        dna.structure.files = files
            .iter()
            .map(|(path, hash)| {
                let mut record = FileRecord::new(*path, FileCategory::Source, 1);
                record.hash = Some((*hash).to_owned());
                record
            })
            .collect();
        dna
    }

    #[test]
    fn summarizes_and_applies_the_policy() {
        let dna = artifact(
            vec![
                finding("a", Severity::Warning),
                finding("b", Severity::Info),
            ],
            &[],
        );
        let passing = evaluate(&dna, None, CiPolicy::default());
        assert!(!passing.failed);
        let summary = text(&passing);
        assert!(summary.contains("Tests detected"));
        assert!(summary.contains("yes (3 files)"));
        assert!(summary.contains("1 warning"));
        let failing = evaluate(
            &dna,
            None,
            CiPolicy {
                fail_on: Some(Severity::Warning),
                new_only: false,
            },
        );
        assert!(failing.failed);
        assert_eq!(failing.failing.len(), 1);
        assert!(markdown(&failing).contains("**Failing:** 1 finding(s)"));
        assert_eq!(json(&failing)["failed"], true);
    }

    #[test]
    fn counts_recent_files_up_to_the_latest_commit() {
        let mut dna = artifact(Vec::new(), &[]);
        dna.insights.recent_changes = Some(repodna_core::model::insights::RecentChanges {
            window_days: 90,
            files_changed: 12,
            ..Default::default()
        });
        let summary = text(&evaluate(&dna, None, CiPolicy::default()));
        // The window ends at the latest commit, which can be long ago.
        assert!(
            summary.contains("Files changed in the last 90 days of history"),
            "{summary}"
        );
    }

    #[test]
    fn unknown_file_hashes_never_create_false_modified_signals() {
        let mut baseline = artifact(
            Vec::new(),
            &[
                ("src/changed.rs", "old"),
                ("src/known_to_unknown.rs", "old"),
                ("src/unknown_to_known.rs", "missing"),
                ("src/both_unknown.rs", "missing"),
            ],
        );
        let mut current = artifact(
            Vec::new(),
            &[
                ("src/changed.rs", "new"),
                ("src/known_to_unknown.rs", "missing"),
                ("src/unknown_to_known.rs", "new"),
                ("src/both_unknown.rs", "missing"),
            ],
        );
        for path in ["src/unknown_to_known.rs", "src/both_unknown.rs"] {
            baseline
                .structure
                .files
                .iter_mut()
                .find(|file| file.path == path)
                .unwrap()
                .hash = None;
        }
        for path in ["src/known_to_unknown.rs", "src/both_unknown.rs"] {
            current
                .structure
                .files
                .iter_mut()
                .find(|file| file.path == path)
                .unwrap()
                .hash = None;
        }
        let outcome = evaluate(&current, Some(&baseline), CiPolicy::default());
        assert_eq!(
            json(&outcome)["changes"]["files"]["modified"],
            serde_json::json!(["src/changed.rs"])
        );
        assert_eq!(
            json(&outcome)["summary"]["Changed files"],
            "1 (0 added, 0 removed, 1 modified)"
        );
    }

    #[test]
    fn compares_with_a_baseline() {
        let baseline = artifact(
            vec![
                finding("a", Severity::Warning),
                finding("old", Severity::Warning),
            ],
            &[("a.rs", "1"), ("gone.rs", "2")],
        );
        let current = artifact(
            vec![
                finding("a", Severity::Warning),
                finding("new", Severity::Warning),
            ],
            &[("a.rs", "9"), ("added.rs", "3")],
        );
        let outcome = evaluate(
            &current,
            Some(&baseline),
            CiPolicy {
                fail_on: Some(Severity::Warning),
                new_only: true,
            },
        );
        let values: BTreeMap<&str, &str> = outcome
            .values
            .iter()
            .map(|(l, v)| (l.as_str(), v.as_str()))
            .collect();
        assert_eq!(
            values["Changed files"],
            "3 (1 added, 1 removed, 1 modified)"
        );
        assert_eq!(values["New findings"], "1");
        assert_eq!(values["Resolved findings"], "1");
        assert_eq!(
            outcome.failing,
            vec![(Severity::Warning, "Cycle new".to_owned())]
        );
    }

    #[test]
    fn new_only_annotations_skip_known_findings_and_preserve_reactivated_findings() {
        let mut previously_suppressed = finding("reactivated", Severity::Warning);
        previously_suppressed.suppressed = Some(repodna_core::finding::Suppression {
            reason: "Reviewed in baseline".to_owned(),
            source: "repodna.toml".to_owned(),
        });
        let baseline = artifact(
            vec![finding("known", Severity::Warning), previously_suppressed],
            &[],
        );
        let current = artifact(
            vec![
                finding("known", Severity::Warning),
                finding("reactivated", Severity::Warning),
                finding("new", Severity::Critical),
                finding("low", Severity::Attention),
            ],
            &[],
        );
        let impact = evaluate(
            &current,
            Some(&baseline),
            CiPolicy {
                fail_on: Some(Severity::Warning),
                new_only: true,
            },
        );
        let introduced = &impact.changes.as_ref().unwrap().introduced;
        let messages =
            github_annotations_for_changes(&current, Severity::Warning, Some(introduced));
        assert_eq!(messages.len(), 2);
        assert!(
            messages
                .iter()
                .any(|text| text.contains("Cycle reactivated"))
        );
        assert!(messages.iter().any(|text| text.contains("Cycle new")));
        assert!(!messages.iter().any(|text| text.contains("Cycle known")));
        assert!(!messages.iter().any(|text| text.contains("Cycle low")));
        assert_eq!(
            github_annotations_for_changes(&current, Severity::Warning, Some(&[])).len(),
            0
        );
        assert_eq!(github_annotations(&current, Severity::Warning).len(), 3);
    }

    #[test]
    fn newly_introduced_findings_get_the_entire_annotation_budget() {
        let existing: Vec<_> = (0..60)
            .map(|number| finding(&format!("known-{number}"), Severity::Warning))
            .collect();
        let baseline = artifact(existing.clone(), &[]);
        let mut current_findings = existing;
        current_findings
            .extend((0..55).map(|number| finding(&format!("new-{number}"), Severity::Warning)));
        let current = artifact(current_findings, &[]);
        let impact = evaluate(&current, Some(&baseline), CiPolicy::default());
        let introduced = &impact.changes.as_ref().unwrap().introduced;
        let annotations =
            github_annotations_for_changes(&current, Severity::Warning, Some(introduced));
        assert_eq!(annotations.len(), 50);
        assert!(annotations.iter().all(|text| text.contains("Cycle new-")));
    }

    #[test]
    fn escapes_workflow_commands() {
        let dna = artifact(vec![finding("a,b", Severity::Warning)], &[]);
        let annotations = github_annotations(&dna, Severity::Attention);
        assert_eq!(annotations.len(), 1);
        let line = &annotations[0];
        assert!(
            line.starts_with(
                "::warning title=RepoDNA%3A architecture.cycle,file=src/a.rs,line=7::"
            )
        );
        assert!(line.contains("a, b%0Acycle 100%25"));
        assert!(!line.contains('\n'));
    }

    #[test]
    fn machine_readable_baseline_impact_is_stable_and_identifiable() {
        let baseline = artifact(
            vec![
                finding("old", Severity::Attention).path("src/old.rs"),
                finding("same", Severity::Info),
            ],
            &[("z.rs", "1"), ("gone.rs", "2"), ("same.rs", "1")],
        );
        let current = artifact(
            vec![
                finding("same", Severity::Info),
                finding("new-low", Severity::Attention).path("src/new.rs"),
                finding("new-high", Severity::Critical).path("src/important.rs"),
            ],
            &[
                ("new.rs", "1"),
                ("a.rs", "1"),
                ("z.rs", "2"),
                ("same.rs", "1"),
            ],
        );
        let outcome = evaluate(
            &current,
            Some(&baseline),
            CiPolicy {
                fail_on: Some(Severity::Warning),
                new_only: true,
            },
        );
        let report = json(&outcome);
        assert_eq!(
            report["changes"]["files"]["added"],
            serde_json::json!(["a.rs", "new.rs"])
        );
        assert_eq!(
            report["changes"]["files"]["removed"],
            serde_json::json!(["gone.rs"])
        );
        assert_eq!(
            report["changes"]["files"]["modified"],
            serde_json::json!(["z.rs"])
        );
        assert_eq!(report["summary"]["New findings"], "2");
        assert_eq!(report["summary"]["Resolved findings"], "1");
        assert_eq!(
            report["changes"]["findings"]["introduced"][0]["severity"],
            "critical"
        );
        assert_eq!(
            report["changes"]["findings"]["introduced"][0]["paths"][0],
            "src/important.rs"
        );
        assert_eq!(
            report["changes"]["findings"]["introduced"][1]["id"],
            finding("new-low", Severity::Attention).id
        );
        assert_eq!(
            report["changes"]["findings"]["resolved"][0]["id"],
            finding("old", Severity::Attention).id
        );
        assert!(outcome.failed);
        assert_eq!(outcome.failing.len(), 1);
    }

    #[test]
    fn suppressed_baseline_finding_is_introduced_when_reactivated() {
        let mut suppressed = finding("reactivated", Severity::Warning);
        suppressed.suppressed = Some(repodna_core::finding::Suppression {
            reason: "Previously reviewed".to_owned(),
            source: "repodna.toml".to_owned(),
        });
        let baseline = artifact(vec![suppressed], &[]);
        let current = artifact(vec![finding("reactivated", Severity::Warning)], &[]);
        let outcome = evaluate(
            &current,
            Some(&baseline),
            CiPolicy {
                fail_on: Some(Severity::Warning),
                new_only: true,
            },
        );
        assert_eq!(outcome.changes.as_ref().unwrap().introduced.len(), 1);
        assert!(outcome.changes.as_ref().unwrap().resolved.is_empty());
        assert!(outcome.failed);
    }

    #[test]
    fn no_baseline_keeps_existing_json_contract_without_changes() {
        let outcome = evaluate(
            &artifact(vec![finding("single", Severity::Info)], &[]),
            None,
            CiPolicy::default(),
        );
        assert!(outcome.changes.is_none());
        assert!(json(&outcome).get("changes").is_none());
        assert!(json(&outcome).get("summary").is_some());
    }

    #[test]
    fn repeated_imported_finding_ids_do_not_duplicate_delta_counts() {
        let duplicate = finding("repeat", Severity::Warning);
        let current = artifact(vec![duplicate.clone(), duplicate], &[]);
        let baseline = artifact(Vec::new(), &[]);
        let outcome = evaluate(&current, Some(&baseline), CiPolicy::default());
        assert_eq!(outcome.changes.as_ref().unwrap().introduced.len(), 1);
        assert!(text(&outcome).contains("New findings"));
    }

    fn module(id: &str, path: &str) -> ModuleRecord {
        ModuleRecord {
            id: id.to_owned(),
            name: id.to_owned(),
            path: path.to_owned(),
            kind: ModuleKind::Directory,
            language: None,
            files: 1,
            code_lines: 1,
            fan_in: 0,
            fan_out: 0,
            instability: 0.0,
            centrality: 0.0,
            layer: None,
            inferred: true,
            confidence: Confidence::High,
            importance: Vec::new(),
            external_dependencies: Vec::new(),
            evidence: Vec::new(),
        }
    }

    fn module_edge(from: &str, to: &str) -> ModuleEdge {
        ModuleEdge {
            from: from.to_owned(),
            to: to.to_owned(),
            weight: 1,
            confidence: Confidence::High,
            samples: Vec::new(),
        }
    }

    #[test]
    fn baseline_impact_links_changed_paths_to_closest_modules_and_adjacent_edges() {
        let mut baseline = artifact(
            Vec::new(),
            &[
                ("src/core/a.rs", "a"),
                ("removed/old.rs", "old"),
                ("src2/keep.rs", "same"),
            ],
        );
        let mut current = artifact(
            Vec::new(),
            &[
                ("src/core/a.rs", "b"),
                ("src/core/new.rs", "new"),
                ("src2/keep.rs", "same"),
            ],
        );
        let modules = vec![
            module("src", "src"),
            module("src/core", "src/core"),
            module("src2", "src2"),
            module("removed", "removed"),
            module("consumer", "consumer"),
        ];
        baseline.architecture.status = SectionStatus::Analyzed;
        baseline.architecture.modules.clone_from(&modules);
        baseline.architecture.module_edges = vec![
            module_edge("consumer", "src/core"),
            module_edge("src2", "removed"),
        ];
        current.architecture.status = SectionStatus::Analyzed;
        current.architecture.modules = modules;
        current.architecture.module_edges = vec![
            module_edge("consumer", "src/core"),
            module_edge("src2", "src/core"),
        ];
        let outcome = evaluate(&current, Some(&baseline), CiPolicy::default());
        let report = json(&outcome);
        let architecture = &report["changes"]["architecture"];
        assert_eq!(
            architecture["touchedModules"],
            serde_json::json!(["removed", "src/core"])
        );
        assert_eq!(
            architecture["directDependents"],
            serde_json::json!(["consumer", "src2"])
        );
        assert_eq!(
            architecture["moduleEdges"]["added"],
            serde_json::json!([{"from": "src2", "to": "src/core"}])
        );
        assert_eq!(
            architecture["moduleEdges"]["removed"],
            serde_json::json!([{"from": "src2", "to": "removed"}])
        );
        assert_eq!(
            report["summary"]["Changed files"],
            "3 (1 added, 1 removed, 1 modified)"
        );
    }

    #[test]
    fn architecture_impact_is_omitted_when_either_snapshot_is_not_analyzed() {
        let mut baseline = artifact(Vec::new(), &[("src/a.rs", "1")]);
        let mut current = artifact(Vec::new(), &[("src/a.rs", "2")]);
        current.architecture.status = SectionStatus::Analyzed;
        current.architecture.modules = vec![module("src", "src")];
        let missing_baseline = json(&evaluate(&current, Some(&baseline), CiPolicy::default()));
        assert!(missing_baseline["changes"].get("architecture").is_none());
        baseline.architecture.status = SectionStatus::Analyzed;
        let fully_analyzed = json(&evaluate(&current, Some(&baseline), CiPolicy::default()));
        assert_eq!(
            fully_analyzed["changes"]["architecture"]["touchedModules"],
            serde_json::json!(["src"])
        );
    }

    #[test]
    fn module_ownership_uses_directory_boundary_and_deepest_match() {
        let mut current = artifact(Vec::new(), &[]);
        current.architecture.modules = vec![
            module("src", "src"),
            module("src2", "src2"),
            module("deep", "src/deep"),
            module("root", ""),
        ];
        assert_eq!(module_for_path(&current, "src2/lib.rs"), Some("src2"));
        assert_eq!(module_for_path(&current, "src/deep/lib.rs"), Some("deep"));
        assert_eq!(module_for_path(&current, "src/other.rs"), Some("src"));
        assert_eq!(module_for_path(&current, "notes.md"), Some("root"));
    }
}
