//! RepoDNA configuration.
//!
//! Configuration is layered: built-in defaults, then the user configuration file, then an
//! explicit `--config` file, then the repository's own `repodna.toml`, and finally
//! command-line flags. Repository configuration is *untrusted* — it comes from the code being
//! analyzed — so it may tune analysis scope and thresholds but can never enable AI
//! providers, plugins, command execution, or network access. See [`load`].

use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::hash::sha256_hex;

pub mod profile;
pub mod suppression;
pub mod thresholds;

pub use profile::{AnalysisProfile, Stage, StageSet};
pub use suppression::{SuppressionRule, apply_suppressions};
pub use thresholds::Thresholds;

/// Directories ignored by default in addition to `.gitignore` rules, because they hold
/// installed dependencies or caches rather than repository content.
pub const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    "node_modules/",
    "bower_components/",
    ".venv/",
    "__pycache__/",
    ".tox/",
    ".mypy_cache/",
    ".pytest_cache/",
    ".gradle/",
    ".next/",
    ".nuxt/",
    ".turbo/",
    ".parcel-cache/",
];

/// The complete configuration.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// What to analyze.
    pub analysis: AnalysisConfig,
    /// Privacy switches.
    pub privacy: PrivacyConfig,
    /// Performance tuning.
    pub performance: PerformanceConfig,
    /// Paths to exclude from discovery.
    pub ignore: IgnoreConfig,
    /// Classification overrides.
    pub classification: ClassificationConfig,
    /// Signal thresholds.
    pub thresholds: Thresholds,
    /// Suppression rules (`[[suppress]]` tables).
    #[serde(rename = "suppress", skip_serializing_if = "Vec::is_empty")]
    pub suppressions: Vec<SuppressionRule>,
    /// Report defaults.
    pub report: ReportConfig,
    /// Optional AI provider (user configuration only).
    pub ai: AiConfig,
    /// Plugins (user configuration only).
    pub plugins: PluginConfig,
    /// Build and test execution (user configuration only).
    pub execution: ExecutionConfig,
}

/// What to analyze.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AnalysisConfig {
    /// Analysis profile.
    pub profile: AnalysisProfile,
    /// Analyze Git history.
    pub include_git: bool,
    /// Reconstruct evolution (epochs, events, Time Machine snapshots).
    pub include_history: bool,
    /// Analyze manifests and lockfiles.
    pub include_dependencies: bool,
    /// Infer architecture.
    pub include_architecture: bool,
    /// Compute quality signals.
    pub include_quality: bool,
    /// Scan for security signals.
    pub include_security: bool,
    /// Detect tests.
    pub include_tests: bool,
    /// Detect build systems and environment requirements.
    pub include_build: bool,
    /// Analyze documentation.
    pub include_docs: bool,
    /// Respect `.gitignore`, `.ignore`, and Git exclude files.
    pub respect_gitignore: bool,
    /// Files larger than this many bytes are classified but not read.
    pub max_file_bytes: u64,
    /// Maximum commits to analyze (0 = unlimited).
    pub max_commits: u64,
    /// Maximum commits stored in the artifact.
    pub artifact_commits: u64,
    /// Maximum symbols stored in the artifact.
    pub max_symbols: u64,
    /// Maximum file-level edges stored in the artifact.
    pub max_file_edges: u64,
    /// Number of Time Machine snapshots to sample.
    pub snapshots: u32,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            profile: AnalysisProfile::Standard,
            include_git: true,
            include_history: true,
            include_dependencies: true,
            include_architecture: true,
            include_quality: true,
            include_security: true,
            include_tests: true,
            include_build: true,
            include_docs: true,
            respect_gitignore: true,
            max_file_bytes: 2 * 1024 * 1024,
            max_commits: 50_000,
            artifact_commits: 1_000,
            max_symbols: 25_000,
            max_file_edges: 25_000,
            snapshots: 12,
        }
    }
}

/// Privacy switches. RepoDNA has no telemetry; `telemetry` exists so the setting is explicit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PrivacyConfig {
    /// Always `false`: RepoDNA does not collect telemetry.
    pub telemetry: bool,
    /// Allow AI providers that send data to remote endpoints.
    pub remote_ai: bool,
    /// Replace contributor names with pseudonyms in artifacts.
    pub anonymize_contributors: bool,
    /// Store commit subject lines in artifacts.
    pub include_commit_messages: bool,
    /// Replace repository paths in log output with hashes.
    pub redact_paths_in_logs: bool,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            telemetry: false,
            remote_ai: false,
            anonymize_contributors: false,
            include_commit_messages: true,
            redact_paths_in_logs: false,
        }
    }
}

/// Number of worker threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Parallelism {
    /// One thread per available CPU.
    #[default]
    Auto,
    /// A fixed number of threads (at least one).
    Threads(usize),
}

impl Parallelism {
    /// Resolves the setting to a thread count, capped at twice the available parallelism.
    pub fn threads(self) -> usize {
        let available = std::thread::available_parallelism().map_or(1, usize::from);
        match self {
            Parallelism::Auto => available,
            Parallelism::Threads(count) => count.clamp(1, available * 2),
        }
    }
}

impl Serialize for Parallelism {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Parallelism::Auto => serializer.serialize_str("auto"),
            Parallelism::Threads(count) => serializer.serialize_u64(*count as u64),
        }
    }
}

impl<'de> Deserialize<'de> for Parallelism {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ParallelismVisitor;

        impl Visitor<'_> for ParallelismVisitor {
            type Value = Parallelism;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("\"auto\" or a positive number of threads")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Parallelism, E> {
                if value.eq_ignore_ascii_case("auto") {
                    Ok(Parallelism::Auto)
                } else {
                    value
                        .parse::<u64>()
                        .map_err(|_| E::custom("expected \"auto\" or a positive integer"))
                        .and_then(|count| self.visit_u64(count))
                }
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Parallelism, E> {
                if value == 0 {
                    return Err(E::custom("parallelism must be at least 1"));
                }
                usize::try_from(value)
                    .map(Parallelism::Threads)
                    .map_err(|_| E::custom("parallelism is too large"))
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Parallelism, E> {
                u64::try_from(value)
                    .map_err(|_| E::custom("parallelism must be positive"))
                    .and_then(|count| self.visit_u64(count))
            }
        }

        deserializer.deserialize_any(ParallelismVisitor)
    }
}

impl JsonSchema for Parallelism {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Parallelism".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "\"auto\" or a positive number of worker threads.",
            "anyOf": [
                { "type": "string", "enum": ["auto"] },
                { "type": "integer", "minimum": 1 }
            ]
        })
    }
}

/// Performance tuning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PerformanceConfig {
    /// Worker threads.
    pub parallelism: Parallelism,
    /// Reuse cached per-file and history results.
    pub cache: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            parallelism: Parallelism::Auto,
            cache: true,
        }
    }
}

/// Paths to exclude from discovery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct IgnoreConfig {
    /// Apply [`DEFAULT_IGNORE_PATTERNS`].
    pub use_default_patterns: bool,
    /// Additional gitignore-style patterns.
    pub patterns: Vec<String>,
}

impl Default for IgnoreConfig {
    fn default() -> Self {
        Self {
            use_default_patterns: true,
            patterns: Vec::new(),
        }
    }
}

impl IgnoreConfig {
    /// Default patterns (when enabled) followed by configured patterns.
    pub fn effective_patterns(&self) -> Vec<String> {
        let mut patterns: Vec<String> = if self.use_default_patterns {
            DEFAULT_IGNORE_PATTERNS
                .iter()
                .map(|p| (*p).to_owned())
                .collect()
        } else {
            Vec::new()
        };
        for pattern in &self.patterns {
            if !patterns.contains(pattern) {
                patterns.push(pattern.clone());
            }
        }
        patterns
    }
}

/// Classification overrides as glob patterns. Later categories win over earlier ones.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ClassificationConfig {
    /// Paths to treat as generated code.
    pub generated: Vec<String>,
    /// Paths to treat as vendored third-party code.
    pub vendor: Vec<String>,
    /// Paths to treat as tests.
    pub tests: Vec<String>,
    /// Paths to treat as documentation.
    pub docs: Vec<String>,
    /// Paths to treat as first-party source even if they look generated or vendored.
    pub source: Vec<String>,
}

/// Visual theme of HTML reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ReportTheme {
    /// Balanced light theme.
    #[default]
    Professional,
    /// Reduced ornamentation.
    Minimal,
    /// Dense, monospace-leaning layout.
    Technical,
    /// Dark theme.
    Dark,
}

impl ReportTheme {
    /// Every theme.
    pub const ALL: [ReportTheme; 4] = [
        ReportTheme::Professional,
        ReportTheme::Minimal,
        ReportTheme::Technical,
        ReportTheme::Dark,
    ];

    /// Identifier.
    pub const fn id(self) -> &'static str {
        match self {
            ReportTheme::Professional => "professional",
            ReportTheme::Minimal => "minimal",
            ReportTheme::Technical => "technical",
            ReportTheme::Dark => "dark",
        }
    }
}

/// How much identifying data exported reports contain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PrivacyPreset {
    /// Everything in the artifact (secret values and absolute paths are never included).
    #[default]
    Local,
    /// Removes commit messages and marker text.
    Share,
    /// Additionally anonymizes contributors and removes remote URLs and symbol names.
    Public,
}

impl PrivacyPreset {
    /// Identifier.
    pub const fn id(self) -> &'static str {
        match self {
            PrivacyPreset::Local => "local",
            PrivacyPreset::Share => "share",
            PrivacyPreset::Public => "public",
        }
    }
}

/// Report defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ReportConfig {
    /// HTML theme.
    pub theme: ReportTheme,
    /// Show "RepoDNA · Made by the Sanskar" branding in generated visuals.
    pub branding: bool,
    /// Report sections to include (empty = all).
    pub sections: Vec<String>,
    /// Privacy preset applied to exported reports.
    pub privacy: PrivacyPreset,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            theme: ReportTheme::Professional,
            branding: true,
            sections: Vec::new(),
            privacy: PrivacyPreset::Local,
        }
    }
}

/// AI provider kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AiProviderKind {
    /// AI features are disabled.
    #[default]
    None,
    /// A local command that reads a prompt on standard input (e.g. `ollama run <model>`).
    Command,
    /// An OpenAI-compatible HTTP API (local servers or cloud providers).
    OpenaiCompatible,
    /// An Anthropic-compatible Messages API.
    Anthropic,
}

impl AiProviderKind {
    /// Identifier.
    pub const fn id(self) -> &'static str {
        match self {
            AiProviderKind::None => "none",
            AiProviderKind::Command => "command",
            AiProviderKind::OpenaiCompatible => "openai-compatible",
            AiProviderKind::Anthropic => "anthropic",
        }
    }
}

/// Optional AI provider. Only honored from user configuration or command-line flags.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AiConfig {
    /// Provider kind.
    pub provider: AiProviderKind,
    /// Model identifier passed to the provider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Base URL for HTTP providers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Name of the environment variable holding the API key (keys are never stored in files).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// Command line for the `command` provider.
    pub command: Vec<String>,
    /// Maximum estimated tokens of repository context sent per request.
    pub max_context_tokens: u32,
    /// Maximum tokens requested in the response.
    pub max_output_tokens: u32,
    /// Request timeout in seconds.
    pub timeout_seconds: u64,
    /// Price per million input tokens, used only for cost estimates you configure yourself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_cost_per_million: Option<f64>,
    /// Price per million output tokens, used only for cost estimates you configure yourself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_cost_per_million: Option<f64>,
    /// Allow short source excerpts in prompts (off by default; evidence metadata only).
    pub include_source_excerpts: bool,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: AiProviderKind::None,
            model: None,
            endpoint: None,
            api_key_env: None,
            command: Vec::new(),
            max_context_tokens: 6_000,
            max_output_tokens: 1_200,
            timeout_seconds: 120,
            input_cost_per_million: None,
            output_cost_per_million: None,
            include_source_excerpts: false,
        }
    }
}

/// Plugins. Only honored from user configuration or command-line flags.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PluginConfig {
    /// Names of plugins to run.
    pub enabled: Vec<String>,
    /// Additional directories to search for plugins.
    pub directories: Vec<String>,
    /// Per-plugin timeout in seconds.
    pub timeout_seconds: u64,
    /// Maximum bytes a plugin may write to standard output.
    pub max_output_bytes: u64,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enabled: Vec::new(),
            directories: Vec::new(),
            timeout_seconds: 60,
            max_output_bytes: 8 * 1024 * 1024,
        }
    }
}

/// Build and test execution. Off by default; only honored from user configuration or flags.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ExecutionConfig {
    /// Allow running detected build commands.
    pub allow_build_commands: bool,
    /// Allow running detected test commands.
    pub allow_test_commands: bool,
    /// Timeout per command in seconds.
    pub timeout_seconds: u64,
    /// Lines of output kept per command.
    pub max_output_lines: u64,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            allow_build_commands: false,
            allow_test_commands: false,
            timeout_seconds: 600,
            max_output_lines: 200,
        }
    }
}

impl Config {
    /// Stages that will run: the profile's stages minus those disabled by `include_*` switches.
    pub fn effective_stages(&self) -> StageSet {
        let a = &self.analysis;
        let mut stages = a.profile.stages();
        let switches = [
            (a.include_git, Stage::Git),
            (a.include_history, Stage::Evolution),
            (a.include_dependencies, Stage::Dependencies),
            (a.include_architecture, Stage::Architecture),
            (a.include_quality, Stage::Quality),
            (a.include_security, Stage::Security),
        ];
        for (enabled, stage) in switches {
            if !enabled {
                stages = stages.without(stage);
            }
        }
        if !a.include_git {
            stages = stages
                .without(Stage::Evolution)
                .without(Stage::HistoricalArchitecture);
        }
        if !a.include_history {
            stages = stages.without(Stage::HistoricalArchitecture);
        }
        if !a.include_quality {
            stages = stages
                .without(Stage::Duplication)
                .without(Stage::Similarity);
        }
        if !(a.include_tests || a.include_build || a.include_docs) {
            stages = stages.without(Stage::Project);
        }
        if self.plugins.enabled.is_empty() {
            stages = stages.without(Stage::Plugins);
        }
        stages
    }

    /// Returns a description of every invalid setting.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = self.thresholds.validate();
        for rule in &self.suppressions {
            if let Err(problem) = rule.validate() {
                problems.push(problem);
            }
        }
        for pattern in self
            .classification
            .generated
            .iter()
            .chain(&self.classification.vendor)
            .chain(&self.classification.tests)
            .chain(&self.classification.docs)
            .chain(&self.classification.source)
        {
            if let Err(error) = crate::glob::Glob::new(pattern) {
                problems.push(format!("classification: {error}"));
            }
        }
        if self.privacy.telemetry {
            problems.push(
                "privacy.telemetry cannot be enabled: RepoDNA does not collect telemetry"
                    .to_owned(),
            );
        }
        match self.ai.provider {
            AiProviderKind::None => {}
            AiProviderKind::Command => {
                if self.ai.command.is_empty() {
                    problems
                        .push("ai.command must be set when ai.provider = \"command\"".to_owned());
                }
            }
            AiProviderKind::OpenaiCompatible | AiProviderKind::Anthropic => {
                if self.ai.model.as_deref().is_none_or(str::is_empty) {
                    problems.push(format!(
                        "ai.model must be set when ai.provider = \"{}\"",
                        self.ai.provider.id()
                    ));
                }
            }
        }
        if self.ai.max_context_tokens < 500 {
            problems.push("ai.max_context_tokens must be at least 500".to_owned());
        }
        if self.analysis.max_file_bytes == 0 {
            problems.push("analysis.max_file_bytes must be greater than 0".to_owned());
        }
        if self.analysis.snapshots > 200 {
            problems.push("analysis.snapshots must be at most 200".to_owned());
        }
        problems
    }

    /// Stable hash of the effective configuration, recorded in artifact metadata.
    pub fn hash(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        sha256_hex(json.as_bytes())[..16].to_owned()
    }

    /// Serializes the configuration as TOML.
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }
}

/// A commented configuration template written by `repodna init`.
pub const CONFIG_TEMPLATE: &str = r#"# RepoDNA project configuration.
# Documentation: https://github.com/sanskarIN/RepoDNA/blob/main/docs/configuration.md
#
# This file is read from the repository being analyzed, so it can only tune what is
# analyzed. AI providers, plugins, and command execution can only be enabled in your
# user configuration (see `repodna config path`).

[analysis]
# quick | standard | deep | history-only | architecture-only | dependencies-only | security-only
profile = "standard"
include_git = true
include_history = true
include_dependencies = true
include_security = true
include_tests = true
# Files larger than this (in bytes) are classified but not read.
max_file_bytes = 2097152

[ignore]
# Gitignore-style patterns excluded in addition to .gitignore.
patterns = []

[classification]
# Override automatic classification with glob patterns.
generated = []
vendor = []

[thresholds]
large_file_lines = 1000
large_function_lines = 80
high_complexity = 15
high_churn_share = 0.3
dependency_concentration = 0.3
duplicate_min_tokens = 70

# Suppress known false positives. Suppressed findings stay visible in reports.
# [[suppress]]
# rule = "security.secret-candidate"
# path = "tests/fixtures/**"
# reason = "Intentional fake credentials used by tests"

[report]
# professional | minimal | technical | dark
theme = "professional"
branding = true
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_parses_to_defaults() {
        let parsed: Config = toml::from_str(CONFIG_TEMPLATE).unwrap();
        assert_eq!(parsed, Config::default());
    }

    #[test]
    fn defaults_are_valid_and_privacy_friendly() {
        let config = Config::default();
        assert!(config.validate().is_empty());
        assert!(!config.privacy.telemetry);
        assert!(!config.privacy.remote_ai);
        assert_eq!(config.ai.provider, AiProviderKind::None);
        assert!(!config.execution.allow_build_commands);
        assert!(!config.execution.allow_test_commands);
    }

    #[test]
    fn switches_remove_stages() {
        let mut config = Config::default();
        assert!(config.effective_stages().contains(Stage::Git));
        assert!(!config.effective_stages().contains(Stage::Plugins));
        config.analysis.include_git = false;
        let stages = config.effective_stages();
        assert!(!stages.contains(Stage::Git));
        assert!(!stages.contains(Stage::Evolution));
        config.plugins.enabled = vec!["example".into()];
        assert!(config.effective_stages().contains(Stage::Plugins));
    }

    #[test]
    fn parallelism_accepts_auto_and_numbers() {
        #[derive(Deserialize)]
        struct Wrapper {
            parallelism: Parallelism,
        }
        let auto: Wrapper = toml::from_str("parallelism = \"auto\"").unwrap();
        assert_eq!(auto.parallelism, Parallelism::Auto);
        let fixed: Wrapper = toml::from_str("parallelism = 4").unwrap();
        assert_eq!(fixed.parallelism, Parallelism::Threads(4));
        assert!(toml::from_str::<Wrapper>("parallelism = 0").is_err());
        assert!(Parallelism::Threads(10_000).threads() >= 1);
    }

    #[test]
    fn validation_reports_problems() {
        let mut config = Config::default();
        config.privacy.telemetry = true;
        config.ai.provider = AiProviderKind::Anthropic;
        config.classification.generated = vec!["[bad".into()];
        let problems = config.validate();
        assert!(problems.iter().any(|p| p.contains("telemetry")));
        assert!(problems.iter().any(|p| p.contains("ai.model")));
        assert!(problems.iter().any(|p| p.contains("classification")));
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let error = toml::from_str::<Config>("[analysis]\nprofil = \"deep\"").unwrap_err();
        assert!(error.to_string().contains("profil"));
    }

    #[test]
    fn hash_changes_with_configuration() {
        let default = Config::default();
        let mut changed = Config::default();
        changed.thresholds.large_file_lines = 42;
        assert_ne!(default.hash(), changed.hash());
        assert_eq!(default.hash(), Config::default().hash());
        assert!(default.to_toml().unwrap().contains("[analysis]"));
    }

    #[test]
    fn effective_ignore_patterns_merge_defaults() {
        let config = IgnoreConfig {
            use_default_patterns: true,
            patterns: vec!["generated/".into(), "node_modules/".into()],
        };
        let patterns = config.effective_patterns();
        assert!(patterns.contains(&"node_modules/".to_owned()));
        assert_eq!(patterns.iter().filter(|p| *p == "node_modules/").count(), 1);
        assert!(patterns.contains(&"generated/".to_owned()));
        let bare = IgnoreConfig {
            use_default_patterns: false,
            patterns: vec![],
        };
        assert!(bare.effective_patterns().is_empty());
    }
}
