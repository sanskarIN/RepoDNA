//! A concise summary for continuous integration, with an optional failure policy.
//!
//! RepoDNA findings are analysis signals, not formal guarantees; the policy only turns the
//! signals a team chooses into a failing exit status.

use std::collections::{BTreeMap, HashSet};

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
}

const COMPLEXITY_RULES: [&str; 4] = [
    "quality.complex-function",
    "quality.long-function",
    "quality.deep-nesting",
    "quality.large-file",
];

/// Changed files between two snapshots: `(added, removed, modified)`.
fn file_changes(baseline: &RepositoryDna, current: &RepositoryDna) -> (usize, usize, usize) {
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
    let added = after.keys().filter(|p| !before.contains_key(*p)).count();
    let removed = before.keys().filter(|p| !after.contains_key(*p)).count();
    let modified = after
        .iter()
        .filter(|(path, hash)| before.get(*path).is_some_and(|old| old != *hash))
        .count();
    (added, removed, modified)
}

/// Evaluates an artifact, optionally against a baseline artifact (for example the base
/// branch of a pull request).
pub fn evaluate(
    dna: &RepositoryDna,
    baseline: Option<&RepositoryDna>,
    policy: CiPolicy,
) -> CiOutcome {
    let active: Vec<_> = dna.findings.iter().filter(|f| !f.is_suppressed()).collect();
    let known: HashSet<&str> = baseline
        .map(|b| b.findings.iter().map(|f| f.id.as_str()).collect())
        .unwrap_or_default();
    let new: Vec<_> = active
        .iter()
        .filter(|f| baseline.is_some() && !known.contains(f.id.as_str()))
        .copied()
        .collect();
    let count = |severity: Severity| active.iter().filter(|f| f.severity == severity).count();
    let mut values = vec![("Repository".to_owned(), dna.identity.name.clone())];
    if let Some(revision) = &dna.analysis_metadata.revision {
        values.push(("Revision".to_owned(), revision.chars().take(12).collect()));
    }
    match baseline {
        Some(baseline) => {
            let (added, removed, modified) = file_changes(baseline, dna);
            values.push((
                "Changed files".to_owned(),
                format!(
                    "{} ({added} added, {removed} removed, {modified} modified)",
                    added + removed + modified
                ),
            ));
        }
        None => {
            if let Some(recent) = &dna.insights.recent_changes {
                values.push((
                    format!("Files changed in the last {} days", recent.window_days),
                    thousands(recent.files_changed),
                ));
            }
        }
    }
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
    if let Some(baseline) = baseline {
        let current: HashSet<&str> = active.iter().map(|f| f.id.as_str()).collect();
        let resolved = baseline
            .findings
            .iter()
            .filter(|f| !f.is_suppressed() && !current.contains(f.id.as_str()))
            .count();
        values.push(("New findings".to_owned(), new.len().to_string()));
        values.push(("Resolved findings".to_owned(), resolved.to_string()));
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

/// Markdown summary, for example for `$GITHUB_STEP_SUMMARY`.
pub fn markdown(outcome: &CiOutcome) -> String {
    let mut out = String::from("### RepoDNA analysis\n\n| Signal | Value |\n| --- | --- |\n");
    for (label, value) in &outcome.values {
        out.push_str(&format!(
            "| {} | {} |\n",
            crate::text::escape_markdown(label),
            crate::text::escape_markdown(value)
        ));
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
    serde_json::json!({
        "summary": values,
        "failed": outcome.failed,
        "failing": outcome.failing.iter().map(|(severity, title)| serde_json::json!({
            "severity": severity.label(),
            "title": title,
        })).collect::<Vec<_>>(),
    })
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
    dna.findings
        .iter()
        .filter(|f| !f.is_suppressed() && f.severity >= minimum)
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
}
