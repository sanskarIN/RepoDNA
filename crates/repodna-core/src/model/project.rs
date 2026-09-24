//! Project conventions: tests, builds, environment requirements, CI, and documentation.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{SectionStatus, is_false};
use crate::confidence::Confidence;
use crate::evidence::Evidence;
use crate::time::Timestamp;

/// A tool, framework, or build system detected in the repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DetectedTool {
    /// Tool name, e.g. `cargo test` or `vitest`.
    pub name: String,
    /// Ecosystem, when meaningful.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ecosystem: Option<String>,
    /// Confidence of the detection.
    pub confidence: Confidence,
    /// Supporting evidence.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// What a command is for, in the order a newcomer would run them.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum CommandPurpose {
    /// Install dependencies.
    Install,
    /// Build the project.
    Build,
    /// Run the tests.
    Test,
    /// Run the program.
    Run,
    /// Start a development server or watcher.
    Dev,
    /// Run linters.
    Lint,
    /// Check or apply formatting.
    Format,
    /// Run benchmarks.
    Bench,
}

impl CommandPurpose {
    /// Human-readable label.
    pub const fn label(self) -> &'static str {
        match self {
            CommandPurpose::Install => "Install",
            CommandPurpose::Build => "Build",
            CommandPurpose::Test => "Test",
            CommandPurpose::Run => "Run",
            CommandPurpose::Dev => "Development",
            CommandPurpose::Lint => "Lint",
            CommandPurpose::Format => "Format",
            CommandPurpose::Bench => "Benchmark",
        }
    }
}

/// A command RepoDNA derived from repository metadata.
///
/// Candidates are *detected*, not *verified*: RepoDNA never runs them unless the user
/// explicitly enables command execution, and `verified` is only set by a successful run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommandCandidate {
    /// The command line.
    pub command: String,
    /// What the command is for.
    pub purpose: CommandPurpose,
    /// Repository-relative working directory (empty for the root).
    pub working_directory: String,
    /// Where the command was found, e.g. `package.json scripts.test`.
    pub source: String,
    /// `true` only after the command ran successfully under RepoDNA's supervision.
    #[serde(default, skip_serializing_if = "is_false")]
    pub verified: bool,
    /// Supporting evidence.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// Outcome of an explicitly authorized command execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionResult {
    /// The command line that was executed.
    pub command: String,
    /// What the command was for.
    pub purpose: CommandPurpose,
    /// Repository-relative working directory.
    pub working_directory: String,
    /// When execution started.
    pub started_at: Timestamp,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Process exit code, absent when the process was killed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// `true` when the timeout was reached and the process was stopped.
    #[serde(default, skip_serializing_if = "is_false")]
    pub timed_out: bool,
    /// `true` when the user cancelled the run.
    #[serde(default, skip_serializing_if = "is_false")]
    pub cancelled: bool,
    /// `true` when the process exited with code 0.
    pub success: bool,
    /// Last lines of combined output, with secret-like values redacted.
    #[serde(default)]
    pub output_tail: Vec<String>,
}

/// Kind of automated tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TestKind {
    /// Unit tests.
    Unit,
    /// Integration tests.
    Integration,
    /// End-to-end or browser tests.
    EndToEnd,
    /// Snapshot or golden-file tests.
    Snapshot,
    /// Benchmarks.
    Benchmark,
}

/// Evidence for one kind of tests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TestKindSignal {
    /// Test kind.
    pub kind: TestKind,
    /// Files suggesting this kind.
    pub files: u64,
    /// Sample evidence.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// Test detection.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TestReport {
    /// Whether this section was analyzed.
    pub status: SectionStatus,
    /// Notes about limitations or partial results.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Detected test frameworks.
    #[serde(default)]
    pub frameworks: Vec<DetectedTool>,
    /// Files classified as tests.
    pub test_files: u64,
    /// Code lines in test files.
    pub test_lines: u64,
    /// Files classified as first-party source code.
    pub source_files: u64,
    /// `testLines / (testLines + sourceCodeLines)` (0–1).
    pub test_ratio: f64,
    /// Directories that contain tests.
    #[serde(default)]
    pub test_directories: Vec<String>,
    /// Kinds of tests detected.
    #[serde(default)]
    pub kinds: Vec<TestKindSignal>,
    /// Test commands derived from metadata.
    #[serde(default)]
    pub commands: Vec<CommandCandidate>,
    /// Test commands found in CI configuration.
    #[serde(default)]
    pub ci_commands: Vec<CommandCandidate>,
    /// Coverage reports present in the repository.
    #[serde(default)]
    pub coverage_artifacts: Vec<String>,
    /// Results of explicitly authorized test runs.
    #[serde(default)]
    pub executions: Vec<ExecutionResult>,
}

/// Kind of environment requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RequirementKind {
    /// A language runtime such as Node.js or Python.
    Runtime,
    /// A compiler toolchain such as Rust or Go.
    Toolchain,
    /// A package manager such as npm or pnpm.
    PackageManager,
    /// An SDK such as .NET or Flutter.
    Sdk,
    /// A container engine.
    Container,
    /// A platform or operating-system hint.
    Platform,
    /// An external service such as a database.
    Service,
}

/// A detected project requirement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentRequirement {
    /// Requirement kind.
    pub kind: RequirementKind,
    /// Name, e.g. `Node.js`.
    pub name: String,
    /// Declared version or constraint, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// File declaring the requirement.
    pub source: String,
}

/// A CI/CD configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CiConfig {
    /// Provider, e.g. `GitHub Actions`.
    pub provider: String,
    /// Configuration file.
    pub path: String,
    /// Job or stage names found in the file.
    #[serde(default)]
    pub jobs: Vec<String>,
}

/// A configuration file and its purpose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFileInfo {
    /// Repository-relative path.
    pub path: String,
    /// What the file configures.
    pub purpose: String,
}

/// Build systems, commands, environment, and CI.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BuildReport {
    /// Whether this section was analyzed.
    pub status: SectionStatus,
    /// Notes about limitations or partial results.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Detected build systems.
    #[serde(default)]
    pub systems: Vec<DetectedTool>,
    /// Build, run, and development commands derived from metadata.
    #[serde(default)]
    pub commands: Vec<CommandCandidate>,
    /// Results of explicitly authorized build runs.
    #[serde(default)]
    pub executions: Vec<ExecutionResult>,
    /// Detected environment requirements.
    #[serde(default)]
    pub requirements: Vec<EnvironmentRequirement>,
    /// CI/CD configurations.
    #[serde(default)]
    pub ci: Vec<CiConfig>,
    /// Container definitions.
    #[serde(default)]
    pub containers: Vec<String>,
    /// Configuration files and their purpose.
    #[serde(default)]
    pub configuration_files: Vec<ConfigFileInfo>,
}

/// Result of a documentation check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DocCheckStatus {
    /// Clearly present.
    Present,
    /// Present in a limited form (e.g. a README section instead of a dedicated file).
    Partial,
    /// Not detected. This does not prove absence.
    NotDetected,
}

impl DocCheckStatus {
    /// Symbol used in terminal and Markdown checklists (never color alone).
    pub const fn symbol(self) -> &'static str {
        match self {
            DocCheckStatus::Present => "✓",
            DocCheckStatus::Partial => "△",
            DocCheckStatus::NotDetected => "✕",
        }
    }
}

/// One documentation check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocCheck {
    /// Identifier, e.g. `contributing`.
    pub id: String,
    /// Label, e.g. `Contributing guide`.
    pub label: String,
    /// Outcome.
    pub status: DocCheckStatus,
    /// `true` for documents many projects legitimately omit.
    pub optional: bool,
    /// Supporting evidence.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// Structure of the README.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadmeInfo {
    /// Repository-relative path.
    pub path: String,
    /// Lines.
    pub lines: u64,
    /// Words.
    pub words: u64,
    /// Headings in order (capped).
    #[serde(default)]
    pub headings: Vec<String>,
    /// Fenced code blocks.
    pub code_blocks: u32,
    /// Links.
    pub links: u32,
    /// Images.
    pub images: u32,
    /// A heading mentions installation or setup.
    pub has_installation: bool,
    /// A heading mentions usage, examples, or getting started.
    pub has_usage: bool,
}

/// A detected license.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LicenseInfo {
    /// File the license was detected in.
    pub path: String,
    /// SPDX identifier, when recognized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spdx: Option<String>,
    /// Confidence of the identification.
    pub confidence: Confidence,
}

/// Documentation analysis.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocsReport {
    /// Whether this section was analyzed.
    pub status: SectionStatus,
    /// Notes about limitations or partial results.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// README structure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme: Option<ReadmeInfo>,
    /// Detected license.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<LicenseInfo>,
    /// Documentation checklist.
    #[serde(default)]
    pub checks: Vec<DocCheck>,
    /// Documentation files.
    pub doc_files: u64,
    /// Lines in documentation files.
    pub doc_lines: u64,
    /// Directories dedicated to documentation.
    #[serde(default)]
    pub doc_directories: Vec<String>,
    /// Example files or directories.
    #[serde(default)]
    pub examples: Vec<String>,
    /// Purpose statement quoted from the README or a manifest, when found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Where `description` was taken from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_source: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_checks_use_symbols_not_just_color() {
        assert_eq!(DocCheckStatus::Present.symbol(), "✓");
        assert_eq!(DocCheckStatus::Partial.symbol(), "△");
        assert_eq!(DocCheckStatus::NotDetected.symbol(), "✕");
        assert_eq!(
            serde_json::to_string(&DocCheckStatus::NotDetected).unwrap(),
            "\"not-detected\""
        );
    }

    #[test]
    fn unverified_commands_omit_the_flag() {
        let candidate = CommandCandidate {
            command: "cargo test".into(),
            purpose: CommandPurpose::Test,
            working_directory: String::new(),
            source: "Cargo.toml".into(),
            verified: false,
            evidence: vec![],
        };
        let json = serde_json::to_value(&candidate).unwrap();
        assert!(json.get("verified").is_none());
        assert_eq!(json["purpose"], "test");
    }
}
