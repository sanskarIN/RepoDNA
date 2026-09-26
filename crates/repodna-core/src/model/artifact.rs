//! The root `RepositoryDNA` document.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::architecture::ArchitectureReport;
use super::dependencies::DependencyReport;
use super::evolution::EvolutionReport;
use super::fingerprint::DnaFingerprint;
use super::git::GitReport;
use super::identity::RepositoryIdentity;
use super::insights::Insights;
use super::languages::LanguageReport;
use super::metadata::{
    AnalysisMetadata, GeneratedReport, PluginRunRecord, SCHEMA_MAJOR, SCHEMA_VERSION, ToolInfo,
};
use super::metrics::MetricsReport;
use super::project::{BuildReport, DocsReport, TestReport};
use super::quality::{QualityReport, SimilarityReport};
use super::security::SecurityReport;
use super::structure::StructureReport;
use crate::finding::Finding;
use crate::hash::StableHasher;
use crate::severity::Severity;

/// A complete, versioned analysis of one repository snapshot.
///
/// Every section is present; a section's `status` says whether it was analyzed. Readers must
/// ignore unknown fields so newer minor schema versions remain readable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(title = "RepositoryDna")]
pub struct RepositoryDna {
    /// Artifact schema version, e.g. `1.0`.
    pub schema_version: String,
    /// The tool that produced the artifact.
    pub tool: ToolInfo,
    /// Repository identity.
    pub identity: RepositoryIdentity,
    /// Files, directories, symbols, and entrypoints.
    #[serde(default)]
    pub structure: StructureReport,
    /// Language composition.
    #[serde(default)]
    pub languages: LanguageReport,
    /// Inferred architecture.
    #[serde(default)]
    pub architecture: ArchitectureReport,
    /// Git history.
    #[serde(default)]
    pub git: GitReport,
    /// Code-quality signals.
    #[serde(default)]
    pub code_quality: QualityReport,
    /// Declared dependencies.
    #[serde(default)]
    pub dependencies: DependencyReport,
    /// Test detection.
    #[serde(default)]
    pub tests: TestReport,
    /// Build systems, environment, and CI.
    #[serde(default)]
    pub builds: BuildReport,
    /// Documentation.
    #[serde(default)]
    pub docs: DocsReport,
    /// Security signals.
    #[serde(default)]
    pub security: SecurityReport,
    /// Evolution and Time Machine.
    #[serde(default)]
    pub evolution: EvolutionReport,
    /// File similarity.
    #[serde(default)]
    pub similarity: SimilarityReport,
    /// Metrics, normalized signals, and confidence.
    #[serde(default)]
    pub metrics: MetricsReport,
    /// Evidence-backed findings, most severe first.
    #[serde(default)]
    pub findings: Vec<Finding>,
    /// Project DNA fingerprint.
    #[serde(default)]
    pub fingerprint: DnaFingerprint,
    /// First-look answers, onboarding, and recent changes.
    #[serde(default)]
    pub insights: Insights,
    /// Plugins that ran.
    #[serde(default)]
    pub plugins: Vec<PluginRunRecord>,
    /// Reports generated alongside this artifact.
    #[serde(default)]
    pub generated_reports: Vec<GeneratedReport>,
    /// How the artifact was produced.
    pub analysis_metadata: AnalysisMetadata,
}

/// Number of active (unsuppressed) findings per severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FindingCounts {
    /// Critical findings.
    pub critical: usize,
    /// Warnings.
    pub warning: usize,
    /// Attention signals.
    pub attention: usize,
    /// Informational findings.
    pub info: usize,
    /// Suppressed findings of any severity.
    pub suppressed: usize,
}

impl RepositoryDna {
    /// Creates an artifact with empty (skipped) sections.
    pub fn new(identity: RepositoryIdentity, analysis_metadata: AnalysisMetadata) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_owned(),
            tool: ToolInfo::current(),
            identity,
            structure: StructureReport::default(),
            languages: LanguageReport::default(),
            architecture: ArchitectureReport::default(),
            git: GitReport::default(),
            code_quality: QualityReport::default(),
            dependencies: DependencyReport::default(),
            tests: TestReport::default(),
            builds: BuildReport::default(),
            docs: DocsReport::default(),
            security: SecurityReport::default(),
            evolution: EvolutionReport::default(),
            similarity: SimilarityReport::default(),
            metrics: MetricsReport::default(),
            findings: Vec::new(),
            fingerprint: DnaFingerprint::default(),
            insights: Insights::default(),
            plugins: Vec::new(),
            generated_reports: Vec::new(),
            analysis_metadata,
        }
    }

    /// Counts active findings per severity.
    pub fn finding_counts(&self) -> FindingCounts {
        let mut counts = FindingCounts::default();
        for finding in &self.findings {
            if finding.is_suppressed() {
                counts.suppressed += 1;
                continue;
            }
            match finding.severity {
                Severity::Critical => counts.critical += 1,
                Severity::Warning => counts.warning += 1,
                Severity::Attention => counts.attention += 1,
                Severity::Info => counts.info += 1,
            }
        }
        counts
    }

    /// Identifier of the primary language, if any.
    pub fn primary_language(&self) -> Option<&str> {
        self.languages.primary.first().map(String::as_str)
    }

    /// Returns the schema major version declared by the artifact.
    pub fn schema_major(&self) -> Option<u32> {
        self.schema_version.split('.').next()?.parse().ok()
    }
}

/// Computes the deterministic DNA hash of a repository snapshot.
///
/// The hash covers the schema major version and every discovered file's path and content
/// hash (or size, for files whose content was not read). It is therefore identical for
/// identical content regardless of when, where, or with which profile the analysis ran.
/// It identifies a snapshot; it is not a security primitive.
pub fn compute_dna_hash(structure: &StructureReport) -> String {
    let mut files: Vec<(&str, String)> = structure
        .files
        .iter()
        .map(|file| {
            let content = file
                .hash
                .clone()
                .unwrap_or_else(|| format!("size:{}", file.bytes));
            (file.path.as_str(), content)
        })
        .collect();
    files.sort();
    let mut hasher = StableHasher::new();
    hasher.str_field(&format!("repodna-dna-v{SCHEMA_MAJOR}"));
    for (path, content) in files {
        hasher.str_field(path).str_field(&content);
    }
    format!("rdna1-{}", &hasher.finish_hex()[..32])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::confidence::Confidence;
    use crate::finding::{FindingCategory, Suppression};
    use crate::model::structure::{FileCategory, FileRecord};

    fn artifact() -> RepositoryDna {
        RepositoryDna::new(
            RepositoryIdentity {
                name: "demo".into(),
                ..RepositoryIdentity::default()
            },
            AnalysisMetadata::default(),
        )
    }

    #[test]
    fn new_artifacts_are_versioned() {
        let dna = artifact();
        assert_eq!(dna.schema_version, SCHEMA_VERSION);
        assert_eq!(dna.schema_major(), Some(SCHEMA_MAJOR));
        let json = serde_json::to_value(&dna).unwrap();
        assert_eq!(json["schemaVersion"], SCHEMA_VERSION);
        assert_eq!(json["codeQuality"]["status"], "skipped");
        assert!(json.get("analysisMetadata").is_some());
    }

    #[test]
    fn counts_findings_by_severity() {
        let mut dna = artifact();
        let finding = |severity| {
            Finding::new(
                "x.rule",
                &format!("{severity:?}"),
                FindingCategory::Structure,
                severity,
                Confidence::High,
                "t",
            )
        };
        dna.findings = vec![
            finding(Severity::Critical),
            finding(Severity::Info),
            finding(Severity::Info),
        ];
        dna.findings[2].suppressed = Some(Suppression {
            reason: "r".into(),
            source: "s".into(),
        });
        let counts = dna.finding_counts();
        assert_eq!(counts.critical, 1);
        assert_eq!(counts.info, 1);
        assert_eq!(counts.suppressed, 1);
    }

    #[test]
    fn dna_hash_depends_only_on_content() {
        let mut a = FileRecord::new("a.rs", FileCategory::Source, 10);
        a.hash = Some("1111".into());
        let b = FileRecord::new("b.bin", FileCategory::Binary, 99);
        let first = StructureReport {
            files: vec![a.clone(), b.clone()],
            ..StructureReport::default()
        };
        let reordered = StructureReport {
            files: vec![b, a],
            total_files: 2,
            ..StructureReport::default()
        };
        assert_eq!(compute_dna_hash(&first), compute_dna_hash(&reordered));
        assert!(compute_dna_hash(&first).starts_with("rdna1-"));
        assert_eq!(compute_dna_hash(&first).len(), 38);

        let mut changed = first.clone();
        changed.files[0].hash = Some("2222".into());
        assert_ne!(compute_dna_hash(&first), compute_dna_hash(&changed));
    }
}
