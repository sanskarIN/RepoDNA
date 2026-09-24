//! Evolution: epochs, architectural events, Time Machine snapshots, and the Project Story.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::SectionStatus;
use crate::confidence::Confidence;
use crate::evidence::Evidence;
use crate::time::Timestamp;

/// Kind of a development epoch. Labels are interpretations of commit activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EpochKind {
    /// The first period of the history.
    Initial,
    /// A period with activity above the repository's median.
    Active,
    /// A period with activity at or below the repository's median.
    Maintenance,
    /// The most recent period.
    Current,
}

/// A contiguous period of development separated from others by long gaps or activity shifts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Epoch {
    /// Position in the history (0 = first).
    pub index: u32,
    /// Label, e.g. `Initial development`.
    pub label: String,
    /// Epoch kind.
    pub kind: EpochKind,
    /// First commit timestamp in the epoch.
    pub start: Timestamp,
    /// Latest commit timestamp in the epoch.
    pub end: Timestamp,
    /// Commits in the epoch.
    pub commits: u64,
    /// Distinct authors in the epoch.
    pub contributors: u32,
    /// Lines added.
    pub insertions: u64,
    /// Lines deleted.
    pub deletions: u64,
    /// Directories that changed most during the epoch.
    #[serde(default)]
    pub focus_areas: Vec<String>,
}

/// Kind of an evolution event.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum EvolutionEventKind {
    /// The first commit.
    RepositoryCreated,
    /// A new top-level area or module appeared.
    ModuleIntroduced,
    /// A top-level area or module disappeared.
    ModuleRemoved,
    /// Many files were renamed or moved in one commit.
    Restructuring,
    /// A language appeared with a meaningful share.
    LanguageIntroduced,
    /// The share of a language changed substantially.
    LanguageShift,
    /// A notable framework dependency was added.
    FrameworkAdopted,
    /// A notable framework dependency was removed.
    FrameworkRemoved,
    /// Test files or test configuration first appeared.
    TestsIntroduced,
    /// CI/CD configuration first appeared.
    CiAdopted,
    /// Container definitions first appeared.
    ContainersAdopted,
    /// The repository grew substantially between snapshots.
    SignificantGrowth,
    /// The repository shrank substantially between snapshots.
    SignificantReduction,
    /// The repository started declaring multiple packages.
    MultiPackageStructure,
    /// A release tag was created.
    Release,
}

/// An evidence-backed historical event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EvolutionEvent {
    /// Stable identifier.
    pub id: String,
    /// Event kind.
    pub kind: EvolutionEventKind,
    /// When the event happened (commit or snapshot date).
    pub date: Timestamp,
    /// Short title, e.g. "A new `network/` area appeared".
    pub title: String,
    /// Description stating only what the evidence shows.
    pub description: String,
    /// Commit most directly associated with the event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Confidence.
    pub confidence: Confidence,
    /// Supporting evidence.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// Why a revision was chosen as a Time Machine snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotKind {
    /// The first commit.
    Initial,
    /// A release tag.
    Release,
    /// An evenly spaced sample of the history.
    Sample,
    /// The analyzed revision.
    Current,
}

/// Language statistics at a snapshot, measured in bytes because historical line counts
/// would require reading every historical file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotLanguage {
    /// Language identifier.
    pub id: String,
    /// Files.
    pub files: u64,
    /// Bytes.
    pub bytes: u64,
    /// Share of bytes among languages that count toward composition (0–1).
    pub share: f64,
}

/// Size of a top-level directory at a snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDirectory {
    /// Directory path (empty for files at the root).
    pub path: String,
    /// Files.
    pub files: u64,
    /// Bytes.
    pub bytes: u64,
}

/// A module edge in a historical architecture snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotEdge {
    /// Dependent module.
    pub from: String,
    /// Dependency module.
    pub to: String,
    /// Number of supporting file edges.
    pub weight: u32,
}

/// Architecture at a snapshot (deep profile only).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotArchitecture {
    /// Modules present, with file counts.
    #[serde(default)]
    pub modules: Vec<SnapshotDirectory>,
    /// Module edges.
    #[serde(default)]
    pub edges: Vec<SnapshotEdge>,
}

/// The repository as it existed at one revision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Commit hash.
    pub revision: String,
    /// Label, e.g. `Initial commit`, `v1.0.0`, `2023-05`, or `Current`.
    pub label: String,
    /// Why the revision was chosen.
    pub kind: SnapshotKind,
    /// Commit timestamp.
    pub date: Timestamp,
    /// Commits up to and including this revision.
    pub commit_index: u64,
    /// Files.
    pub files: u64,
    /// Bytes.
    pub bytes: u64,
    /// Test files.
    pub test_files: u64,
    /// Documentation files.
    pub doc_files: u64,
    /// Language statistics.
    #[serde(default)]
    pub languages: Vec<SnapshotLanguage>,
    /// Top-level directories.
    #[serde(default)]
    pub directories: Vec<SnapshotDirectory>,
    /// Direct dependencies declared by manifests at this revision, when parsed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<u32>,
    /// Module-level architecture, when historical parsing was enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture: Option<SnapshotArchitecture>,
}

/// One point of the growth curve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GrowthPoint {
    /// Date of the snapshot.
    pub date: Timestamp,
    /// Files.
    pub files: u64,
    /// Bytes.
    pub bytes: u64,
    /// Commits so far.
    pub commits: u64,
}

/// A directory that stopped changing while the rest of the repository continued.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AbandonedArea {
    /// Directory path.
    pub path: String,
    /// Latest change inside the directory.
    pub last_changed: Timestamp,
    /// Days between that change and the latest commit in the repository.
    pub days_before_latest: i64,
    /// Files currently in the directory.
    pub files: u64,
    /// Neutral description, e.g. "No changes detected since 2021-03-04."
    pub description: String,
}

/// Approximate age class of a module relative to the repository's lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AgeClass {
    /// Created in the first fifth of the history and older than two years.
    Ancient,
    /// Older than one year and not growing.
    Established,
    /// Older than 90 days with substantial recent churn.
    Growing,
    /// Created within the last year.
    Recent,
    /// Created within the last 30 days.
    New,
}

impl AgeClass {
    /// Human-readable label.
    pub const fn label(self) -> &'static str {
        match self {
            AgeClass::Ancient => "Ancient",
            AgeClass::Established => "Established",
            AgeClass::Growing => "Growing",
            AgeClass::Recent => "Recent",
            AgeClass::New => "New",
        }
    }
}

/// Age of a module, with actual dates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModuleAge {
    /// Module identifier.
    pub module: String,
    /// Earliest commit touching a file in the module.
    pub first_commit: Timestamp,
    /// Latest commit touching a file in the module.
    pub last_change: Timestamp,
    /// Days between the first commit and the latest repository commit.
    pub age_days: i64,
    /// Age class.
    pub class: AgeClass,
    /// Why the class was assigned.
    pub reason: String,
}

/// Whether a statement is a verifiable fact or an interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum StatementKind {
    /// Directly supported by the linked evidence.
    Fact,
    /// A reading of the evidence that may be wrong.
    Interpretation,
}

/// A sentence of a narrative, linked to its evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StoryStatement {
    /// The sentence.
    pub text: String,
    /// Fact or interpretation.
    pub kind: StatementKind,
    /// Date the statement refers to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<Timestamp>,
    /// Supporting evidence.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// Software-archaeology view of the history.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchaeologyReport {
    /// How the project started.
    #[serde(default)]
    pub origin: Vec<StoryStatement>,
    /// How it grew.
    #[serde(default)]
    pub growth: Vec<StoryStatement>,
    /// Major expansions.
    #[serde(default)]
    pub expansions: Vec<StoryStatement>,
    /// Major rewrites and restructurings.
    #[serde(default)]
    pub rewrites: Vec<StoryStatement>,
    /// Periods of inactivity.
    #[serde(default)]
    pub inactivity: Vec<StoryStatement>,
    /// The final (most recent) active phase.
    #[serde(default)]
    pub final_phase: Vec<StoryStatement>,
    /// The current state.
    #[serde(default)]
    pub current_state: Vec<StoryStatement>,
}

/// Evolution analysis.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EvolutionReport {
    /// Whether this section was analyzed.
    pub status: SectionStatus,
    /// Notes about limitations or partial results.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Development epochs, oldest first.
    #[serde(default)]
    pub epochs: Vec<Epoch>,
    /// Historical events, oldest first.
    #[serde(default)]
    pub events: Vec<EvolutionEvent>,
    /// Time Machine snapshots, oldest first.
    #[serde(default)]
    pub snapshots: Vec<Snapshot>,
    /// Growth curve derived from snapshots.
    #[serde(default)]
    pub growth: Vec<GrowthPoint>,
    /// Directories that stopped changing.
    #[serde(default)]
    pub abandoned_areas: Vec<AbandonedArea>,
    /// Module ages.
    #[serde(default)]
    pub module_ages: Vec<ModuleAge>,
    /// Chronological Project Story.
    #[serde(default)]
    pub story: Vec<StoryStatement>,
    /// Software-archaeology view.
    #[serde(default)]
    pub archaeology: ArchaeologyReport,
    /// How snapshots were sampled.
    pub sampling: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_distinguish_fact_from_interpretation() {
        let statement = StoryStatement {
            text: "The project started with 12 files.".into(),
            kind: StatementKind::Fact,
            date: Timestamp::from_ymd(2020, 1, 1),
            evidence: vec![Evidence::commit("abc", None)],
        };
        let json = serde_json::to_value(&statement).unwrap();
        assert_eq!(json["kind"], "fact");
        assert_eq!(json["date"], "2020-01-01T00:00:00Z");
    }

    #[test]
    fn event_kinds_serialize_in_kebab_case() {
        assert_eq!(
            serde_json::to_string(&EvolutionEventKind::CiAdopted).unwrap(),
            "\"ci-adopted\""
        );
        assert_eq!(AgeClass::Growing.label(), "Growing");
    }
}
