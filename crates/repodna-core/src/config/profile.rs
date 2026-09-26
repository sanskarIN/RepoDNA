//! Analysis profiles and the pipeline stages they enable.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A stage of the analysis pipeline.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// Walk the repository, classify files, and detect languages.
    Discovery,
    /// Lexical analysis of file contents.
    Parsing,
    /// Commit history, contributors, and file history.
    Git,
    /// Manifests and lockfiles.
    Dependencies,
    /// Modules, dependency graph, cycles, and layers.
    Architecture,
    /// Complexity, size, markers, and dead-code candidates.
    Quality,
    /// Token-based duplicate detection.
    Duplication,
    /// MinHash file similarity.
    Similarity,
    /// Tests, builds, environment, CI, and documentation.
    Project,
    /// Secret candidates and risky patterns.
    Security,
    /// Epochs, events, snapshots, and the Project Story.
    Evolution,
    /// Architecture snapshots of historical revisions (reads historical file contents).
    HistoricalArchitecture,
    /// Explicitly enabled plugins.
    Plugins,
    /// First-look answers, onboarding, and important files.
    Insights,
}

impl Stage {
    /// Every stage in pipeline order.
    pub const ALL: [Stage; 14] = [
        Stage::Discovery,
        Stage::Parsing,
        Stage::Git,
        Stage::Dependencies,
        Stage::Architecture,
        Stage::Quality,
        Stage::Duplication,
        Stage::Similarity,
        Stage::Project,
        Stage::Security,
        Stage::Evolution,
        Stage::HistoricalArchitecture,
        Stage::Plugins,
        Stage::Insights,
    ];

    /// Identifier used in configuration, logs, and timings.
    pub const fn id(self) -> &'static str {
        match self {
            Stage::Discovery => "discovery",
            Stage::Parsing => "parsing",
            Stage::Git => "git",
            Stage::Dependencies => "dependencies",
            Stage::Architecture => "architecture",
            Stage::Quality => "quality",
            Stage::Duplication => "duplication",
            Stage::Similarity => "similarity",
            Stage::Project => "project",
            Stage::Security => "security",
            Stage::Evolution => "evolution",
            Stage::HistoricalArchitecture => "historical-architecture",
            Stage::Plugins => "plugins",
            Stage::Insights => "insights",
        }
    }

    /// Human-readable label.
    pub const fn label(self) -> &'static str {
        match self {
            Stage::Discovery => "Files discovered",
            Stage::Parsing => "Source files parsed",
            Stage::Git => "Git history analyzed",
            Stage::Dependencies => "Dependencies analyzed",
            Stage::Architecture => "Architecture inferred",
            Stage::Quality => "Quality signals computed",
            Stage::Duplication => "Duplication detected",
            Stage::Similarity => "File similarity estimated",
            Stage::Project => "Tests, build, and docs detected",
            Stage::Security => "Security signals scanned",
            Stage::Evolution => "Evolution reconstructed",
            Stage::HistoricalArchitecture => "Historical architecture sampled",
            Stage::Plugins => "Plugins executed",
            Stage::Insights => "Insights generated",
        }
    }

    const fn bit(self) -> u32 {
        1 << (self as u32)
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// A set of pipeline stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct StageSet(u32);

impl StageSet {
    /// The empty set.
    pub const EMPTY: StageSet = StageSet(0);

    /// Creates a set from a list of stages.
    pub fn of(stages: &[Stage]) -> Self {
        stages
            .iter()
            .fold(Self::EMPTY, |set, &stage| set.with(stage))
    }

    /// Returns `true` if the set contains `stage`.
    pub const fn contains(self, stage: Stage) -> bool {
        self.0 & stage.bit() != 0
    }

    /// Returns a copy of the set with `stage` added.
    #[must_use]
    pub const fn with(self, stage: Stage) -> Self {
        Self(self.0 | stage.bit())
    }

    /// Returns a copy of the set with `stage` removed.
    #[must_use]
    pub const fn without(self, stage: Stage) -> Self {
        Self(self.0 & !stage.bit())
    }

    /// Iterates the stages in pipeline order.
    pub fn iter(self) -> impl Iterator<Item = Stage> {
        Stage::ALL
            .into_iter()
            .filter(move |stage| self.contains(*stage))
    }
}

/// A preset trading analysis depth for speed.
///
/// All profiles are deterministic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AnalysisProfile {
    /// Structure, languages, basic metrics, and project conventions. No Git.
    Quick,
    /// Adds Git history, dependencies, architecture, quality, security, evolution, and
    /// enabled plugins.
    #[default]
    Standard,
    /// Adds duplication, similarity, and historical architecture.
    Deep,
    /// Git history and evolution only.
    HistoryOnly,
    /// Parsing, dependencies, and architecture only.
    ArchitectureOnly,
    /// Manifests and lockfiles only.
    DependenciesOnly,
    /// Security signals only.
    SecurityOnly,
}

impl AnalysisProfile {
    /// Every profile.
    pub const ALL: [AnalysisProfile; 7] = [
        AnalysisProfile::Quick,
        AnalysisProfile::Standard,
        AnalysisProfile::Deep,
        AnalysisProfile::HistoryOnly,
        AnalysisProfile::ArchitectureOnly,
        AnalysisProfile::DependenciesOnly,
        AnalysisProfile::SecurityOnly,
    ];

    /// Identifier used on the command line and in configuration.
    pub const fn id(self) -> &'static str {
        match self {
            AnalysisProfile::Quick => "quick",
            AnalysisProfile::Standard => "standard",
            AnalysisProfile::Deep => "deep",
            AnalysisProfile::HistoryOnly => "history-only",
            AnalysisProfile::ArchitectureOnly => "architecture-only",
            AnalysisProfile::DependenciesOnly => "dependencies-only",
            AnalysisProfile::SecurityOnly => "security-only",
        }
    }

    /// One-line description.
    pub const fn description(self) -> &'static str {
        match self {
            AnalysisProfile::Quick => {
                "Current structure, languages, basic metrics, and project conventions; no Git history."
            }
            AnalysisProfile::Standard => {
                "Adds Git history, dependencies, architecture, quality, security, evolution, and enabled plugins."
            }
            AnalysisProfile::Deep => {
                "Adds duplication, file similarity, and historical architecture snapshots."
            }
            AnalysisProfile::HistoryOnly => "Git history and evolution only.",
            AnalysisProfile::ArchitectureOnly => "Parsing, dependencies, and architecture only.",
            AnalysisProfile::DependenciesOnly => "Manifests and lockfiles only.",
            AnalysisProfile::SecurityOnly => "Secret candidates and risky patterns only.",
        }
    }

    /// Stages enabled by the profile (before configuration switches are applied).
    pub fn stages(self) -> StageSet {
        use Stage::*;
        match self {
            AnalysisProfile::Quick => StageSet::of(&[Discovery, Parsing, Project, Insights]),
            AnalysisProfile::Standard => StageSet::of(&[
                Discovery,
                Parsing,
                Git,
                Dependencies,
                Architecture,
                Quality,
                Project,
                Security,
                Evolution,
                Plugins,
                Insights,
            ]),
            AnalysisProfile::Deep => AnalysisProfile::Standard
                .stages()
                .with(Duplication)
                .with(Similarity)
                .with(HistoricalArchitecture),
            AnalysisProfile::HistoryOnly => StageSet::of(&[Discovery, Git, Evolution]),
            AnalysisProfile::ArchitectureOnly => {
                StageSet::of(&[Discovery, Parsing, Dependencies, Architecture])
            }
            AnalysisProfile::DependenciesOnly => StageSet::of(&[Discovery, Dependencies]),
            AnalysisProfile::SecurityOnly => StageSet::of(&[Discovery, Security]),
        }
    }
}

impl fmt::Display for AnalysisProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

impl FromStr for AnalysisProfile {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
        AnalysisProfile::ALL
            .into_iter()
            .find(|profile| profile.id() == normalized)
            .ok_or_else(|| {
                let valid: Vec<_> = AnalysisProfile::ALL.iter().map(|p| p.id()).collect();
                format!(
                    "unknown profile {value:?}; expected one of {}",
                    valid.join(", ")
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_is_a_superset_of_standard() {
        let standard = AnalysisProfile::Standard.stages();
        let deep = AnalysisProfile::Deep.stages();
        for stage in standard.iter() {
            assert!(deep.contains(stage), "{stage}");
        }
        assert!(deep.contains(Stage::Duplication));
        assert!(!standard.contains(Stage::Duplication));
    }

    #[test]
    fn quick_skips_git() {
        assert!(!AnalysisProfile::Quick.stages().contains(Stage::Git));
        assert!(AnalysisProfile::Quick.stages().contains(Stage::Discovery));
    }

    #[test]
    fn every_profile_discovers_files() {
        for profile in AnalysisProfile::ALL {
            assert!(profile.stages().contains(Stage::Discovery), "{profile}");
        }
    }

    #[test]
    fn parses_profile_names() {
        assert_eq!("deep".parse::<AnalysisProfile>(), Ok(AnalysisProfile::Deep));
        assert_eq!(
            "History_Only".parse::<AnalysisProfile>(),
            Ok(AnalysisProfile::HistoryOnly)
        );
        let error = "fast".parse::<AnalysisProfile>().unwrap_err();
        assert!(error.contains("quick"));
    }

    #[test]
    fn stage_sets_add_and_remove() {
        let set = StageSet::EMPTY.with(Stage::Git).with(Stage::Security);
        assert!(set.contains(Stage::Git));
        let set = set.without(Stage::Git);
        assert!(!set.contains(Stage::Git));
        assert_eq!(set.iter().collect::<Vec<_>>(), vec![Stage::Security]);
    }
}
