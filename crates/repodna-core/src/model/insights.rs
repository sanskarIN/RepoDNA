//! Insights derived from the analysis: first-look answers, onboarding steps, important
//! files, and recent changes. Every answer links to evidence and is labeled as fact or
//! interpretation.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::evolution::StatementKind;
use crate::confidence::Confidence;
use crate::evidence::Evidence;
use crate::time::Timestamp;

/// An answer to one "first look" question such as "Where does it start?".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuestionAnswer {
    /// Stable identifier, e.g. `entrypoints`.
    pub id: String,
    /// The question.
    pub question: String,
    /// The answer, written from the evidence.
    pub answer: String,
    /// Fact or interpretation.
    pub kind: StatementKind,
    /// Confidence of the answer.
    pub confidence: Confidence,
    /// Supporting evidence.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// One step of an onboarding or learning path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GuideStep {
    /// Step title, e.g. `Read the entrypoint`.
    pub title: String,
    /// What to do and why.
    pub description: String,
    /// Repository files referenced by the step.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Commands referenced by the step (detected, not verified).
    #[serde(default)]
    pub commands: Vec<String>,
}

/// A file worth reading early, with the reasons it ranks highly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportantFile {
    /// Repository-relative path.
    pub path: String,
    /// Ranking score (0–1).
    pub score: f64,
    /// Evidence-backed reasons.
    #[serde(default)]
    pub reasons: Vec<String>,
}

/// Recent change activity in one directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryChange {
    /// Directory path.
    pub path: String,
    /// Commits touching it in the window.
    pub commits: u64,
    /// Lines added plus deleted in the window.
    pub churn: u64,
}

/// How a dependency changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeKind {
    /// Newly declared.
    Added,
    /// No longer declared.
    Removed,
    /// Requirement changed.
    Changed,
}

/// A dependency declaration change between two revisions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DependencyChange {
    /// Ecosystem identifier.
    pub ecosystem: String,
    /// Package name.
    pub name: String,
    /// Kind of change.
    pub change: ChangeKind,
    /// Previous requirement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// New requirement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Manifest where the change happened.
    pub manifest: String,
}

/// What changed during the recent window before the analyzed revision.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecentChanges {
    /// Window size in days, ending at the latest commit.
    pub window_days: u32,
    /// Start of the window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<Timestamp>,
    /// End of the window (the latest commit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<Timestamp>,
    /// Commits in the window.
    pub commits: u64,
    /// Distinct authors in the window.
    pub contributors: u32,
    /// Distinct files changed.
    pub files_changed: u64,
    /// Lines added.
    pub insertions: u64,
    /// Lines deleted.
    pub deletions: u64,
    /// Directories with the most churn.
    #[serde(default)]
    pub directories: Vec<DirectoryChange>,
    /// Files added in the window that still exist.
    #[serde(default)]
    pub added_files: Vec<String>,
    /// Files deleted in the window.
    #[serde(default)]
    pub removed_files: Vec<String>,
    /// Dependency declaration changes in the window.
    #[serde(default)]
    pub dependency_changes: Vec<DependencyChange>,
    /// Manifests changed in the window.
    #[serde(default)]
    pub manifests_changed: Vec<String>,
    /// Test files changed in the window.
    pub test_files_changed: u64,
    /// Documentation files changed in the window.
    pub doc_files_changed: u64,
}

/// A term used in the repository, with a definition derived from the analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GlossaryTerm {
    /// The term, e.g. a module or tool name.
    pub term: String,
    /// What the term refers to in this repository.
    pub definition: String,
}

/// Human-oriented insights computed from the analysis.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Insights {
    /// Answers to the "first look" questions.
    #[serde(default)]
    pub first_look: Vec<QuestionAnswer>,
    /// Suggested onboarding path.
    #[serde(default)]
    pub onboarding: Vec<GuideStep>,
    /// Files worth reading first.
    #[serde(default)]
    pub important_files: Vec<ImportantFile>,
    /// What changed recently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recent_changes: Option<RecentChanges>,
    /// Repository-specific terminology.
    #[serde(default)]
    pub glossary: Vec<GlossaryTerm>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insights_default_to_empty() {
        let insights = Insights::default();
        let json = serde_json::to_value(&insights).unwrap();
        assert!(json.get("recentChanges").is_none());
        assert_eq!(json["firstLook"].as_array().unwrap().len(), 0);
        assert_eq!(
            serde_json::to_string(&ChangeKind::Added).unwrap(),
            "\"added\""
        );
    }
}
