//! Git history: commits, contributors, file history, hotspots, and activity.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{SectionStatus, is_false};
use crate::evidence::Evidence;
use crate::time::Timestamp;

/// A reference to a commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommitRef {
    /// Full commit hash.
    pub hash: String,
    /// Abbreviated hash (first 7 characters).
    pub short: String,
    /// Author timestamp.
    pub timestamp: Timestamp,
}

/// A commit with its change statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommitRecord {
    /// Full commit hash.
    pub hash: String,
    /// Abbreviated hash (first 7 characters).
    pub short: String,
    /// Identifier of the author (see [`ContributorRecord::id`]).
    pub author: String,
    /// Author timestamp.
    pub timestamp: Timestamp,
    /// Subject line (may be removed by privacy settings).
    pub subject: String,
    /// Files changed (zero for merge commits, which are not diffed).
    pub files_changed: u32,
    /// Lines added.
    pub insertions: u64,
    /// Lines deleted.
    pub deletions: u64,
    /// `true` for merge commits.
    #[serde(default, skip_serializing_if = "is_false")]
    pub merge: bool,
}

/// A branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    /// Branch name, e.g. `main` or `origin/main`.
    pub name: String,
    /// `true` for remote-tracking branches.
    pub remote: bool,
    /// Commit the branch points to.
    pub head: String,
    /// Committer timestamp of the branch head.
    pub updated: Timestamp,
}

/// A tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    /// Tag name.
    pub name: String,
    /// Commit the tag points to (peeled).
    pub commit: String,
    /// Tag or commit timestamp.
    pub date: Timestamp,
    /// `true` for annotated tags.
    pub annotated: bool,
}

/// A tag whose name looks like a release version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    /// Tag name, e.g. `v1.2.0`.
    pub tag: String,
    /// Parsed version string without prefix, e.g. `1.2.0`.
    pub version: String,
    /// Tag date.
    pub date: Timestamp,
    /// Commit the release points to.
    pub commit: String,
    /// Commits between the previous release and this one.
    pub commits_since_previous: u64,
}

/// Share of a contributor's commits within one directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AreaShare {
    /// Directory path (top two levels at most).
    pub path: String,
    /// Commits touching the directory.
    pub commits: u64,
}

/// A contributor as recorded in public Git history.
///
/// Contribution statistics describe the history; they are not judgments about people.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContributorRecord {
    /// Stable pseudonymous identifier derived from the normalized e-mail address.
    /// E-mail addresses themselves are never stored.
    pub id: String,
    /// Name as recorded in commits (after `.mailmap`), or a pseudonym when anonymized.
    pub name: String,
    /// Number of commits.
    pub commits: u64,
    /// Lines added.
    pub insertions: u64,
    /// Lines deleted.
    pub deletions: u64,
    /// First commit timestamp.
    pub first_commit: Timestamp,
    /// Latest commit timestamp.
    pub last_commit: Timestamp,
    /// Distinct days with at least one commit.
    pub active_days: u32,
    /// Directories most often touched.
    #[serde(default)]
    pub areas: Vec<AreaShare>,
}

/// History statistics for one file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileHistoryRecord {
    /// Current repository-relative path.
    pub path: String,
    /// Commits that changed the file.
    pub commits: u32,
    /// Distinct authors.
    pub authors: u32,
    /// Lines added across history.
    pub insertions: u64,
    /// Lines deleted across history.
    pub deletions: u64,
    /// First commit that touched the file (after following renames).
    pub first_seen: Timestamp,
    /// Latest commit that touched the file.
    pub last_changed: Timestamp,
    /// Commits in the recent window (see `GitReport::recentWindowDays`).
    pub recent_commits: u32,
    /// Earlier paths of the file, when renames were detected.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub previous_paths: Vec<String>,
}

/// History statistics for a directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryActivity {
    /// Directory path (top two levels at most).
    pub path: String,
    /// Commits touching the directory.
    pub commits: u64,
    /// Lines added plus deleted.
    pub churn: u64,
    /// Distinct authors.
    pub authors: u32,
    /// Latest change.
    pub last_changed: Timestamp,
    /// Share of the repository's churn in the recent window (0–1).
    pub recent_churn_share: f64,
}

/// A file that changes frequently and carries other risk-relevant signals.
///
/// A hotspot is a prioritization aid, not a defect claim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Hotspot {
    /// Repository-relative path.
    pub path: String,
    /// Rank (1 = strongest hotspot).
    pub rank: u32,
    /// Combined score (0–1); see `method` in the documentation.
    pub score: f64,
    /// Commits touching the file.
    pub commits: u32,
    /// Distinct authors.
    pub authors: u32,
    /// Lines added plus deleted.
    pub churn: u64,
    /// Commits in the recent window.
    pub recent_commits: u32,
    /// Code lines.
    pub lines: u64,
    /// Highest function complexity in the file.
    pub complexity: u32,
    /// Number of files that import this file.
    pub dependents: u32,
    /// Why the file is a hotspot, one reason per signal.
    #[serde(default)]
    pub reasons: Vec<String>,
    /// Neutral interpretation of what the signals mean.
    pub interpretation: String,
    /// Supporting evidence.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// Commit activity for one calendar month.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimelineBucket {
    /// Month key, `YYYY-MM`.
    pub period: String,
    /// First instant of the month.
    pub start: Timestamp,
    /// Commits in the month.
    pub commits: u64,
    /// Distinct authors in the month.
    pub authors: u32,
    /// Lines added.
    pub insertions: u64,
    /// Lines deleted.
    pub deletions: u64,
}

/// Commit activity for one day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DailyActivity {
    /// Date, `YYYY-MM-DD` (UTC).
    pub date: String,
    /// Commits on that day.
    pub commits: u32,
    /// Lines added plus deleted.
    pub churn: u64,
}

/// Overall activity level at the time of analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ActivityLevel {
    /// At least 30 commits in the last 30 days.
    VeryActive,
    /// At least 5 commits in the last 30 days.
    Active,
    /// At least one commit in the last 90 days.
    Moderate,
    /// At least one commit in the last 365 days.
    Low,
    /// No commits in the last 365 days.
    Dormant,
    /// No history available.
    #[default]
    None,
}

impl ActivityLevel {
    /// Human-readable label.
    pub const fn label(self) -> &'static str {
        match self {
            ActivityLevel::VeryActive => "Very active",
            ActivityLevel::Active => "Active",
            ActivityLevel::Moderate => "Moderate activity",
            ActivityLevel::Low => "Low activity",
            ActivityLevel::Dormant => "No recent commits",
            ActivityLevel::None => "No history",
        }
    }
}

/// Activity summary relative to a reference time.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySummary {
    /// Activity level according to the documented thresholds.
    pub level: ActivityLevel,
    /// Time the activity was measured against (normally the analysis time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_time: Option<Timestamp>,
    /// Commits in the 30 days before the reference time.
    pub commits_last_30_days: u64,
    /// Commits in the 90 days before the reference time.
    pub commits_last_90_days: u64,
    /// Commits in the 365 days before the reference time.
    pub commits_last_365_days: u64,
    /// Days between the latest commit and the reference time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days_since_last_commit: Option<i64>,
    /// Neutral description, e.g. "No activity detected after 2021-03-04."
    pub description: String,
}

/// A period without commits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DormantPeriod {
    /// Timestamp of the last commit before the gap.
    pub start: Timestamp,
    /// Timestamp of the first commit after the gap.
    pub end: Timestamp,
    /// Length of the gap in days.
    pub days: i64,
    /// Commit before the gap.
    pub before_commit: String,
    /// Commit after the gap.
    pub after_commit: String,
}

/// How concentrated commits are among contributors.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipSummary {
    /// Number of contributors.
    pub contributors: u32,
    /// Fewest contributors who together authored at least half of all commits.
    pub contributors_for_half_of_commits: u32,
    /// Share of commits by the most active contributor (0–1).
    pub top_contributor_share: f64,
    /// Neutral explanation of the numbers.
    pub note: String,
}

/// Git history analysis.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GitReport {
    /// Whether this section was analyzed.
    pub status: SectionStatus,
    /// Notes about limitations or partial results.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Git version used for the analysis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_version: Option<String>,
    /// Commit analyzed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<CommitRef>,
    /// Branch checked out at analysis time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_branch: Option<String>,
    /// Default branch, when it can be determined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_branch: Option<String>,
    /// `true` when the clone is shallow and history is incomplete.
    #[serde(default, skip_serializing_if = "is_false")]
    pub shallow: bool,
    /// `true` when the configured commit limit was reached.
    #[serde(default, skip_serializing_if = "is_false")]
    pub history_truncated: bool,
    /// Number of commits analyzed (reachable from HEAD).
    pub commit_count: u64,
    /// Earliest analyzed commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_commit: Option<CommitRef>,
    /// Latest analyzed commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_commit: Option<CommitRef>,
    /// Size of the "recent" window used for recency signals, in days before the latest commit.
    pub recent_window_days: u32,
    /// Most recent commits (capped by configuration).
    #[serde(default)]
    pub commits: Vec<CommitRecord>,
    /// Branches.
    #[serde(default)]
    pub branches: Vec<BranchInfo>,
    /// Tags.
    #[serde(default)]
    pub tags: Vec<TagInfo>,
    /// Tags that look like release versions, oldest first.
    #[serde(default)]
    pub releases: Vec<ReleaseInfo>,
    /// Contributors sorted by commits, descending.
    #[serde(default)]
    pub contributors: Vec<ContributorRecord>,
    /// History per current file, sorted by path.
    #[serde(default)]
    pub file_history: Vec<FileHistoryRecord>,
    /// History per directory.
    #[serde(default)]
    pub directory_activity: Vec<DirectoryActivity>,
    /// Ranked hotspots.
    #[serde(default)]
    pub hot_spots: Vec<Hotspot>,
    /// Monthly commit activity.
    #[serde(default)]
    pub timeline: Vec<TimelineBucket>,
    /// Daily commit activity (days with at least one commit).
    #[serde(default)]
    pub daily_activity: Vec<DailyActivity>,
    /// Commits per weekday (0 = Monday) and hour (0–23), in the author's local time.
    #[serde(default)]
    pub weekday_hour: Vec<Vec<u32>>,
    /// Activity summary.
    #[serde(default)]
    pub activity: ActivitySummary,
    /// Gaps without commits longer than the configured threshold.
    #[serde(default)]
    pub dormant_periods: Vec<DormantPeriod>,
    /// Commit concentration among contributors.
    #[serde(default)]
    pub ownership: OwnershipSummary,
}

impl GitReport {
    /// Looks up the history of a file by path.
    pub fn file(&self, path: &str) -> Option<&FileHistoryRecord> {
        self.file_history
            .binary_search_by(|record| record.path.as_str().cmp(path))
            .ok()
            .map(|index| &self.file_history[index])
    }

    /// Looks up a contributor by identifier.
    pub fn contributor(&self, id: &str) -> Option<&ContributorRecord> {
        self.contributors
            .iter()
            .find(|contributor| contributor.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(path: &str) -> FileHistoryRecord {
        FileHistoryRecord {
            path: path.into(),
            commits: 1,
            authors: 1,
            insertions: 1,
            deletions: 0,
            first_seen: Timestamp::UNIX_EPOCH,
            last_changed: Timestamp::UNIX_EPOCH,
            recent_commits: 0,
            previous_paths: Vec::new(),
        }
    }

    #[test]
    fn finds_file_history_by_binary_search() {
        let report = GitReport {
            file_history: vec![history("a.rs"), history("b.rs"), history("c.rs")],
            ..GitReport::default()
        };
        assert_eq!(report.file("b.rs").map(|r| r.path.as_str()), Some("b.rs"));
        assert!(report.file("z.rs").is_none());
    }

    #[test]
    fn activity_levels_have_neutral_labels() {
        assert_eq!(ActivityLevel::Dormant.label(), "No recent commits");
        assert_eq!(
            serde_json::to_string(&ActivityLevel::VeryActive).unwrap(),
            "\"very-active\""
        );
    }
}
