//! Analysis metadata recorded for reproducibility.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::{AnalysisProfile, PrivacyPreset, SuppressionRule, Thresholds};
use crate::time::Timestamp;

/// Version of the RepoDNA artifact schema produced by this build.
pub const SCHEMA_VERSION: &str = "1.0";
/// Major component of [`SCHEMA_VERSION`]; artifacts with a different major version are
/// not readable without migration.
pub const SCHEMA_MAJOR: u32 = 1;
/// Minor component of [`SCHEMA_VERSION`]; minor versions only add optional fields.
pub const SCHEMA_MINOR: u32 = 0;

/// The tool that produced an artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    /// Always `RepoDNA`.
    pub name: String,
    /// Tool version, e.g. `1.0.0`.
    pub version: String,
    /// Artifact schema version, e.g. `1.0`.
    pub schema_version: String,
}

impl ToolInfo {
    /// Information about this build.
    pub fn current() -> Self {
        Self {
            name: "RepoDNA".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
        }
    }
}

/// How the repository was provided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum InputKind {
    /// A local directory without Git metadata.
    #[default]
    LocalDirectory,
    /// A local Git working tree.
    GitRepository,
    /// A remote Git URL cloned for analysis.
    GitUrl,
    /// A ZIP or TAR archive extracted for analysis.
    Archive,
    /// An existing RepoDNA artifact.
    Artifact,
}

/// The analyzed input, described without revealing local absolute paths.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InputInfo {
    /// Input kind.
    pub kind: InputKind,
    /// Safe display string: a directory name, a credential-free URL, or an archive file name.
    pub display: String,
}

/// Outcome of one pipeline stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AnalyzerStatus {
    /// Completed successfully.
    Completed,
    /// Completed with some inputs skipped.
    Partial,
    /// Not enabled.
    Skipped,
    /// Failed; the rest of the analysis continued.
    Failed,
    /// Stopped because the user cancelled the analysis.
    Cancelled,
}

/// Timing and outcome of one pipeline stage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerRun {
    /// Stage identifier, e.g. `git`.
    pub stage: String,
    /// Human-readable label.
    pub label: String,
    /// Outcome.
    pub status: AnalyzerStatus,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Explanation for partial, failed, or skipped stages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// The platform the analysis ran on.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    /// Operating system, e.g. `linux`.
    pub os: String,
    /// CPU architecture, e.g. `x86_64`.
    pub arch: String,
    /// OS family, e.g. `unix`.
    pub family: String,
}

impl PlatformInfo {
    /// The current platform.
    pub fn current() -> Self {
        Self {
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            family: std::env::consts::FAMILY.to_owned(),
        }
    }
}

/// Kind of data source used by the analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DataSourceKind {
    /// Files read from the local file system.
    LocalFiles,
    /// Local Git history.
    GitHistory,
    /// A repository cloned from a remote URL.
    RemoteClone,
    /// A plugin.
    Plugin,
    /// An AI provider.
    Ai,
    /// A vulnerability advisory provider.
    Advisory,
}

/// A data source used by the analysis, with the time it was accessed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DataSource {
    /// Source kind.
    pub kind: DataSourceKind,
    /// Name, e.g. `git 2.43.0`.
    pub name: String,
    /// Additional detail.
    pub detail: String,
    /// When the source was accessed.
    pub accessed_at: Timestamp,
}

/// Privacy facts about the analysis and the document.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyInfo {
    /// Always `false`: RepoDNA collects no telemetry.
    pub telemetry: bool,
    /// `true` when the analysis used the network (for example to clone a URL).
    pub network_used: bool,
    /// `true` when a remote AI provider received data.
    pub remote_ai: bool,
    /// Privacy preset applied to this document.
    pub preset: PrivacyPreset,
    /// Redactions applied to this document, e.g. `contributor names anonymized`.
    #[serde(default)]
    pub redactions: Vec<String>,
}

/// AI usage during the analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AiUsage {
    /// Provider identifier.
    pub provider: String,
    /// Model identifier.
    pub model: String,
    /// `true` when data left the machine.
    pub remote: bool,
    /// Requests made.
    pub requests: u32,
}

/// Result of running one plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginRunRecord {
    /// Plugin name.
    pub name: String,
    /// Plugin version.
    pub version: String,
    /// Outcome.
    pub status: AnalyzerStatus,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Findings contributed.
    pub findings: u32,
    /// Metrics contributed.
    pub metrics: u32,
    /// Explanation for failures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// A report generated from this artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedReport {
    /// Format, e.g. `html`.
    pub format: String,
    /// Path relative to the report output directory.
    pub path: String,
}

/// Everything needed to understand how an artifact was produced.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisMetadata {
    /// Identifier of this analysis run.
    pub id: String,
    /// When the analysis finished (honors `SOURCE_DATE_EPOCH`).
    pub generated_at: Timestamp,
    /// Profile used.
    pub profile: AnalysisProfile,
    /// Input description.
    pub input: InputInfo,
    /// Commit analyzed, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// `true` when the working tree had uncommitted changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty: Option<bool>,
    /// Per-stage outcomes and timings.
    #[serde(default)]
    pub analyzers: Vec<AnalyzerRun>,
    /// Total wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Platform.
    pub platform: PlatformInfo,
    /// Hash of the effective configuration.
    pub config_hash: String,
    /// Configuration layers that contributed.
    #[serde(default)]
    pub config_sources: Vec<String>,
    /// Thresholds in effect.
    #[serde(default)]
    pub thresholds: Thresholds,
    /// Suppression rules in effect.
    #[serde(default)]
    pub suppressions: Vec<SuppressionRule>,
    /// Findings suppressed by those rules.
    pub suppressed_findings: u32,
    /// `true` when the analysis was cancelled or a stage failed.
    pub partial: bool,
    /// Warnings raised during the analysis.
    #[serde(default)]
    pub warnings: Vec<String>,
    /// Data sources used.
    #[serde(default)]
    pub data_sources: Vec<DataSource>,
    /// Privacy facts.
    #[serde(default)]
    pub privacy: PrivacyInfo,
    /// AI usage, when an AI provider was used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai: Option<AiUsage>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_info_reports_this_build() {
        let tool = ToolInfo::current();
        assert_eq!(tool.name, "RepoDNA");
        assert_eq!(tool.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(tool.schema_version, SCHEMA_VERSION);
        assert_eq!(SCHEMA_VERSION, format!("{SCHEMA_MAJOR}.{SCHEMA_MINOR}"));
    }

    #[test]
    fn platform_is_detected() {
        let platform = PlatformInfo::current();
        assert!(!platform.os.is_empty());
        assert!(!platform.arch.is_empty());
    }

    #[test]
    fn metadata_defaults_report_no_telemetry() {
        let metadata = AnalysisMetadata::default();
        assert!(!metadata.privacy.telemetry);
        let json = serde_json::to_value(&metadata).unwrap();
        assert_eq!(json["profile"], "standard");
        assert_eq!(json["input"]["kind"], "local-directory");
    }
}
