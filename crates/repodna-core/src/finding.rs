//! Findings: evidence-backed observations produced by analyzers.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::confidence::Confidence;
use crate::evidence::Evidence;
use crate::hash::stable_id;
use crate::severity::Severity;

/// The dashboard area a finding belongs to.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum FindingCategory {
    /// Repository layout, file classification, and size.
    Structure,
    /// Modules, dependency edges, cycles, and layering.
    Architecture,
    /// Declared packages, manifests, and lockfiles.
    Dependencies,
    /// Commit activity, churn, and hotspots.
    Activity,
    /// Contribution patterns (never judgments about individuals).
    Contributors,
    /// Complexity signals for functions and files.
    Complexity,
    /// Duplicated or highly similar code.
    Duplication,
    /// Maintainability signals such as markers and dead-code candidates.
    Maintainability,
    /// Test detection and test commands.
    Tests,
    /// Build systems, commands, and environment requirements.
    Build,
    /// Documentation presence and structure.
    Documentation,
    /// Defensive security signals such as secret candidates.
    Security,
    /// Historical changes and architectural transitions.
    Evolution,
    /// Findings contributed by a plugin.
    Plugin,
}

impl FindingCategory {
    /// Every category in dashboard order.
    pub const ALL: [FindingCategory; 14] = [
        FindingCategory::Structure,
        FindingCategory::Architecture,
        FindingCategory::Dependencies,
        FindingCategory::Activity,
        FindingCategory::Contributors,
        FindingCategory::Complexity,
        FindingCategory::Duplication,
        FindingCategory::Maintainability,
        FindingCategory::Tests,
        FindingCategory::Build,
        FindingCategory::Documentation,
        FindingCategory::Security,
        FindingCategory::Evolution,
        FindingCategory::Plugin,
    ];

    /// Human-readable label.
    pub const fn label(self) -> &'static str {
        match self {
            FindingCategory::Structure => "Structure",
            FindingCategory::Architecture => "Architecture",
            FindingCategory::Dependencies => "Dependencies",
            FindingCategory::Activity => "Activity",
            FindingCategory::Contributors => "Contributors",
            FindingCategory::Complexity => "Complexity",
            FindingCategory::Duplication => "Duplication",
            FindingCategory::Maintainability => "Maintainability",
            FindingCategory::Tests => "Tests",
            FindingCategory::Build => "Build",
            FindingCategory::Documentation => "Documentation",
            FindingCategory::Security => "Security",
            FindingCategory::Evolution => "Evolution",
            FindingCategory::Plugin => "Plugin",
        }
    }
}

impl fmt::Display for FindingCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Records that a finding matched a user-defined suppression rule.
///
/// Suppressed findings stay in the artifact so suppression is always visible and auditable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Suppression {
    /// The reason recorded in the suppression rule.
    pub reason: String,
    /// Where the rule was defined, e.g. `repodna.toml` or `user configuration`.
    pub source: String,
}

/// An evidence-backed observation about the repository.
///
/// Findings answer: *what is this* (`title`, `summary`), *why does it matter* (`rationale`),
/// *how was it calculated* (`method`), *what supports it* (`evidence`), *how sure are we*
/// (`confidence`, `limitations`), and *what to look at next* (`next_steps`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// Stable identifier: `<rule>:<hash of rule and subject>`. Stays the same across scans
    /// while the underlying subject (file, cycle, package…) is unchanged.
    pub id: String,
    /// Rule identifier, e.g. `architecture.cycle`.
    pub rule: String,
    /// Dashboard category.
    pub category: FindingCategory,
    /// Severity assigned according to the documented criteria.
    pub severity: Severity,
    /// Confidence of the underlying method.
    pub confidence: Confidence,
    /// Short title, e.g. `Dependency cycle`.
    pub title: String,
    /// What was observed.
    pub summary: String,
    /// Why the observation may matter.
    pub rationale: String,
    /// How the observation was calculated.
    pub method: String,
    /// Facts that support the finding.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    /// Known limitations of the method for this finding.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
    /// Suggested next steps for investigation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub next_steps: Vec<String>,
    /// Repository paths the finding is about; used for linking and suppression matching.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    /// Present when a suppression rule matched this finding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppressed: Option<Suppression>,
}

impl Finding {
    /// Starts a finding for `rule` about `subject`.
    ///
    /// `subject` identifies what the finding is about (a path, a sorted list of cycle members,
    /// a package name…) and determines the stable identifier.
    pub fn new(
        rule: impl Into<String>,
        subject: &str,
        category: FindingCategory,
        severity: Severity,
        confidence: Confidence,
        title: impl Into<String>,
    ) -> Self {
        let rule = rule.into();
        let id = format!("{rule}:{}", stable_id(&[rule.as_str(), subject]));
        Self {
            id,
            rule,
            category,
            severity,
            confidence,
            title: title.into(),
            summary: String::new(),
            rationale: String::new(),
            method: String::new(),
            evidence: Vec::new(),
            limitations: Vec::new(),
            next_steps: Vec::new(),
            paths: Vec::new(),
            suppressed: None,
        }
    }

    /// Sets the summary.
    #[must_use]
    pub fn summary(mut self, text: impl Into<String>) -> Self {
        self.summary = text.into();
        self
    }

    /// Sets the rationale ("why it matters").
    #[must_use]
    pub fn rationale(mut self, text: impl Into<String>) -> Self {
        self.rationale = text.into();
        self
    }

    /// Sets the method description.
    #[must_use]
    pub fn method(mut self, text: impl Into<String>) -> Self {
        self.method = text.into();
        self
    }

    /// Appends one evidence item.
    #[must_use]
    pub fn evidence(mut self, item: Evidence) -> Self {
        self.evidence.push(item);
        self
    }

    /// Appends several evidence items.
    #[must_use]
    pub fn with_evidence(mut self, items: impl IntoIterator<Item = Evidence>) -> Self {
        self.evidence.extend(items);
        self
    }

    /// Appends a limitation.
    #[must_use]
    pub fn limitation(mut self, text: impl Into<String>) -> Self {
        self.limitations.push(text.into());
        self
    }

    /// Appends a suggested next step.
    #[must_use]
    pub fn next_step(mut self, text: impl Into<String>) -> Self {
        self.next_steps.push(text.into());
        self
    }

    /// Appends a path the finding is about.
    #[must_use]
    pub fn path(mut self, path: impl Into<String>) -> Self {
        let path = path.into();
        if !self.paths.contains(&path) {
            self.paths.push(path);
        }
        self
    }

    /// Returns `true` if a suppression rule matched this finding.
    pub fn is_suppressed(&self) -> bool {
        self.suppressed.is_some()
    }

    /// Display ordering: most severe first, then category and rule. Use it with a stable
    /// sort, so findings of one rule keep the order their analyzer gave them (hotspots by
    /// rank, functions by complexity); analyzers emit findings in a deterministic order.
    pub fn display_order(a: &Finding, b: &Finding) -> Ordering {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.category.cmp(&b.category))
            .then_with(|| a.rule.cmp(&b.rule))
    }
}

/// Sorts findings for display (a stable sort by [`Finding::display_order`]) and keeps only
/// the first finding with each identifier.
pub fn normalize_findings(findings: &mut Vec<Finding>) {
    findings.sort_by(Finding::display_order);
    let mut seen = HashSet::new();
    findings.retain(|finding| seen.insert(finding.id.clone()));
}

/// Up to `limit` findings worth looking at first, from findings in display order: the
/// first of each rule, so one kind of finding does not crowd out the others, then the
/// remaining ones in order. Suppressed findings are skipped.
pub fn highlights(findings: &[Finding], limit: usize) -> Vec<&Finding> {
    let active: Vec<&Finding> = findings.iter().filter(|f| !f.is_suppressed()).collect();
    let mut rules = HashSet::new();
    let (mut picked, rest): (Vec<&Finding>, Vec<&Finding>) = active
        .iter()
        .partition(|finding| rules.insert(finding.rule.as_str()));
    picked.truncate(limit);
    let room = limit - picked.len();
    picked.extend(rest.into_iter().take(room));
    picked.sort_by(|a, b| Finding::display_order(a, b));
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(rule: &str, subject: &str, severity: Severity) -> Finding {
        Finding::new(
            rule,
            subject,
            FindingCategory::Architecture,
            severity,
            Confidence::High,
            "Sample",
        )
    }

    /// A finding whose title is its subject, to check orderings.
    fn titled(rule: &str, subject: &str, severity: Severity) -> Finding {
        Finding::new(
            rule,
            subject,
            FindingCategory::Architecture,
            severity,
            Confidence::High,
            subject,
        )
    }

    #[test]
    fn identifiers_are_stable_and_subject_specific() {
        let a = sample("architecture.cycle", "a|b", Severity::Warning);
        let b = sample("architecture.cycle", "a|b", Severity::Warning);
        let c = sample("architecture.cycle", "a|c", Severity::Warning);
        assert_eq!(a.id, b.id);
        assert_ne!(a.id, c.id);
        assert!(a.id.starts_with("architecture.cycle:"));
    }

    #[test]
    fn builder_collects_details() {
        let finding = sample("quality.large-file", "src/big.rs", Severity::Attention)
            .summary("The file has 2,400 lines.")
            .rationale("Large files are harder to review.")
            .method("Counted non-blank lines.")
            .evidence(Evidence::file("src/big.rs"))
            .limitation("Generated files are excluded.")
            .next_step("Look for cohesive sections that could be extracted.")
            .path("src/big.rs")
            .path("src/big.rs");
        assert_eq!(finding.paths, vec!["src/big.rs"]);
        assert_eq!(finding.evidence.len(), 1);
        let json = serde_json::to_value(&finding).unwrap();
        assert_eq!(json["category"], "architecture");
        assert_eq!(
            json["nextSteps"][0],
            "Look for cohesive sections that could be extracted."
        );
        assert!(json.get("suppressed").is_none());
    }

    #[test]
    fn normalization_orders_by_severity_and_dedups() {
        let mut findings = vec![
            sample("b.rule", "1", Severity::Info),
            sample("a.rule", "1", Severity::Critical),
            sample("a.rule", "1", Severity::Critical),
            sample("c.rule", "1", Severity::Warning),
        ];
        normalize_findings(&mut findings);
        let rules: Vec<_> = findings.iter().map(|f| f.rule.as_str()).collect();
        assert_eq!(rules, vec!["a.rule", "c.rule", "b.rule"]);
    }

    #[test]
    fn normalization_keeps_the_analyzer_order_within_a_rule() {
        let mut findings = vec![
            titled("a.rule", "rank 1", Severity::Attention),
            titled("a.rule", "rank 2", Severity::Attention),
            titled("b.rule", "x", Severity::Warning),
            titled("a.rule", "rank 3", Severity::Attention),
        ];
        normalize_findings(&mut findings);
        let subjects: Vec<_> = findings.iter().map(|f| f.title.as_str()).collect();
        assert_eq!(subjects, vec!["x", "rank 1", "rank 2", "rank 3"]);
    }

    #[test]
    fn highlights_take_one_finding_per_rule_first() {
        let findings = vec![
            titled("a.rule", "1", Severity::Warning),
            titled("a.rule", "2", Severity::Warning),
            titled("a.rule", "3", Severity::Warning),
            titled("b.rule", "1", Severity::Attention),
            titled("c.rule", "1", Severity::Info),
        ];
        let subjects = |limit| {
            highlights(&findings, limit)
                .iter()
                .map(|f| format!("{} {}", f.rule, f.title))
                .collect::<Vec<_>>()
        };
        assert_eq!(subjects(2), vec!["a.rule 1", "b.rule 1"]);
        assert_eq!(
            subjects(4),
            vec!["a.rule 1", "a.rule 2", "b.rule 1", "c.rule 1"]
        );
        assert_eq!(subjects(9).len(), 5);
    }
}
