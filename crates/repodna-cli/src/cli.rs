//! Command-line arguments.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use repodna_core::config::{AnalysisProfile, PrivacyPreset, ReportTheme};
use repodna_core::severity::Severity;

/// Exit codes, shown at the end of `repodna --help`.
pub const EXIT_CODES: &str = "Exit codes:
  0    success
  1    unexpected internal error
  2    invalid command-line usage
  3    input not found, unreadable, or unsupported
  4    invalid configuration
  5    CI policy violated (findings at or above --fail-on)
  6    local storage could not be read or written
  7    an external tool or service failed (Git, network, AI provider)
  130  cancelled (Ctrl+C)

Findings are analysis signals backed by evidence, not formal guarantees.
Documentation: https://github.com/sanskarIN/RepoDNA";

/// Understand any codebase: evidence-backed analysis of structure, architecture, history,
/// and health. Local-first; no telemetry.
#[derive(Debug, Parser)]
#[command(name = "repodna", version, after_help = EXIT_CODES, max_term_width = 100)]
pub struct Cli {
    /// Options that apply to every command.
    #[command(flatten)]
    pub global: GlobalArgs,
    /// The command to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Options that apply to every command.
#[derive(Debug, Clone, Args)]
pub struct GlobalArgs {
    /// When to use color. NO_COLOR is respected in auto mode.
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto, global = true)]
    pub color: ColorChoice,
    /// Print only results and errors, without progress.
    #[arg(short, long, global = true)]
    pub quiet: bool,
    /// Read this configuration file after your user configuration.
    #[arg(long, value_name = "FILE", global = true)]
    pub config: Option<PathBuf>,
    /// Ignore your user configuration file.
    #[arg(long, global = true)]
    pub no_user_config: bool,
    /// Ignore the repository's own repodna.toml.
    #[arg(long, global = true)]
    pub no_project_config: bool,
}

/// When to use color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorChoice {
    /// Use color when writing to a terminal and NO_COLOR is not set.
    Auto,
    /// Always use color.
    Always,
    /// Never use color.
    Never,
}

fn parse_profile(value: &str) -> Result<AnalysisProfile, String> {
    value.parse()
}

/// How analyses run, for commands that may analyze their target.
#[derive(Debug, Clone, Default, Args)]
pub struct AnalysisArgs {
    /// Analysis profile: quick, standard, deep, history-only, architecture-only,
    /// dependencies-only, or security-only.
    #[arg(long, value_parser = parse_profile, value_name = "PROFILE")]
    pub profile: Option<AnalysisProfile>,
    /// Do not store the analysis locally (this also skips the cache).
    #[arg(long)]
    pub no_store: bool,
    /// Do not use the per-file analysis cache.
    #[arg(long)]
    pub no_cache: bool,
    /// Enable a plugin by name for this run (repeatable).
    #[arg(long = "plugin", value_name = "NAME")]
    pub plugins: Vec<String>,
    /// Also search this directory for plugins (repeatable).
    #[arg(long = "plugin-dir", value_name = "DIR")]
    pub plugin_dirs: Vec<PathBuf>,
    /// Replace contributor names with pseudonyms.
    #[arg(long)]
    pub anonymize: bool,
    /// Do not keep commit subjects in the artifact.
    #[arg(long)]
    pub no_commit_messages: bool,
    /// Worker threads (0 means one per CPU).
    #[arg(long, value_name = "N")]
    pub threads: Option<usize>,
    /// Analyze at most this many commits.
    #[arg(long, value_name = "N")]
    pub max_commits: Option<u64>,
    /// Clone URLs with at most this many commits of history.
    #[arg(long, value_name = "N")]
    pub clone_depth: Option<u32>,
    /// Allow cloning from hosts on private networks.
    #[arg(long)]
    pub allow_private_hosts: bool,
    /// Allow cloning over unencrypted http:// and git:// URLs.
    #[arg(long)]
    pub allow_insecure_urls: bool,
    /// Record zero durations so repeated runs produce identical artifacts.
    #[arg(long)]
    pub reproducible: bool,
}

/// Privacy preset for outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PrivacyArg {
    /// Everything in the artifact (secret values and absolute paths are never included).
    Local,
    /// Remove commit messages, marker comment text, and command output.
    Share,
    /// Also anonymize contributors and remove remote URLs and symbol names.
    Public,
}

impl From<PrivacyArg> for PrivacyPreset {
    fn from(value: PrivacyArg) -> Self {
        match value {
            PrivacyArg::Local => PrivacyPreset::Local,
            PrivacyArg::Share => PrivacyPreset::Share,
            PrivacyArg::Public => PrivacyPreset::Public,
        }
    }
}

/// Report theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ThemeArg {
    /// Balanced light theme.
    Professional,
    /// Reduced decoration.
    Minimal,
    /// Denser, with more numbers.
    Technical,
    /// Dark background.
    Dark,
}

impl From<ThemeArg> for ReportTheme {
    fn from(value: ThemeArg) -> Self {
        match value {
            ThemeArg::Professional => ReportTheme::Professional,
            ThemeArg::Minimal => ReportTheme::Minimal,
            ThemeArg::Technical => ReportTheme::Technical,
            ThemeArg::Dark => ReportTheme::Dark,
        }
    }
}

/// Minimum severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SeverityArg {
    /// Informational and above (everything).
    Info,
    /// Attention signals and above.
    Attention,
    /// Warnings and above.
    Warning,
    /// Critical only.
    Critical,
}

impl From<SeverityArg> for Severity {
    fn from(value: SeverityArg) -> Self {
        match value {
            SeverityArg::Info => Severity::Info,
            SeverityArg::Attention => Severity::Attention,
            SeverityArg::Warning => Severity::Warning,
            SeverityArg::Critical => Severity::Critical,
        }
    }
}

/// Help text for targets.
const TARGET_HELP: &str = "A directory, archive, or Git URL to analyze; an artifact file (.json or .repodna); or the name, path, or identifier of a stored repository";

/// Every command.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Analyze a repository and store the result.
    #[command(visible_alias = "scan")]
    Analyze(AnalyzeCmd),
    /// Write reports: a bundle (HTML, Markdown, JSON, card, CSV) or one format.
    Report(ReportCmd),
    /// Show the inferred architecture.
    Architecture(ViewCmd),
    /// Show declared dependencies and dependency signals.
    Dependencies(ViewCmd),
    /// Show Git history and contributor activity.
    History(ViewCmd),
    /// Show change hotspots and complexity signals.
    Hotspots(ViewCmd),
    /// Show the Time Machine snapshots and evolution.
    Timeline(ViewCmd),
    /// List findings with their evidence.
    Findings(FindingsCmd),
    /// Show any report sections in the terminal.
    Show(ShowCmd),
    /// Compare two or more repositories or analyses.
    Compare(CompareCmd),
    /// Generate the Project DNA card (SVG or PNG).
    Card(CardCmd),
    /// Generate README badges.
    Badge(BadgeCmd),
    /// Write a developer onboarding guide.
    Onboarding(OnboardingCmd),
    /// Explain a repository with an optional AI provider (off unless configured).
    Explain(ExplainCmd),
    /// Summarize an analysis for CI, optionally failing on findings.
    Ci(CiCmd),
    /// Export an analysis as a portable .repodna file.
    Export(ExportCmd),
    /// Import a .repodna or .json artifact into local storage.
    Import(ImportCmd),
    /// List stored repositories.
    #[command(visible_alias = "ls")]
    List(ListCmd),
    /// Write a commented repodna.toml into a repository.
    Init(InitCmd),
    /// Show, create, and check configuration.
    Config(ConfigCmd),
    /// List, inspect, enable, and disable plugins.
    Plugins(PluginsCmd),
    /// Inspect, clear, and repair local storage and caches.
    Cache(CacheCmd),
    /// Delete stored analyses.
    Clean(CleanCmd),
    /// Check the installation, storage, tools, and configuration.
    Doctor(DoctorCmd),
    /// Print version information.
    Version(VersionCmd),
    /// Serve the web interface and a local API on this machine (127.0.0.1 only).
    Serve(ServeCmd),
    /// Print the JSON Schema of the analysis artifact or the configuration file.
    Schema(SchemaCmd),
    /// Print a shell completion script.
    Completions(CompletionsCmd),
}

/// Output format of `analyze`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AnalyzeFormat {
    /// A readable summary.
    Text,
    /// The complete artifact as JSON.
    Json,
    /// The full report as Markdown.
    Markdown,
}

/// `repodna analyze`.
#[derive(Debug, Args)]
pub struct AnalyzeCmd {
    /// A directory, archive, or Git URL.
    #[arg(default_value = ".")]
    pub input: String,
    /// What to print.
    #[arg(long, value_enum, default_value_t = AnalyzeFormat::Text)]
    pub format: AnalyzeFormat,
    /// Also write a report bundle into this directory.
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<PathBuf>,
    /// Privacy preset for printed and written output.
    #[arg(long, value_enum, default_value_t = PrivacyArg::Local)]
    pub privacy: PrivacyArg,
    /// Write into an existing directory that RepoDNA did not create.
    #[arg(long)]
    pub force: bool,
    /// How the analysis runs.
    #[command(flatten)]
    pub analysis: AnalysisArgs,
}

/// Report formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ReportFormat {
    /// A directory with every format.
    Bundle,
    /// One self-contained HTML page.
    Html,
    /// Markdown.
    Markdown,
    /// The artifact as JSON.
    Json,
    /// One CSV table.
    Csv,
}

/// `repodna report`.
#[derive(Debug, Args)]
pub struct ReportCmd {
    /// The repository or analysis to use (see the help text).
    #[arg(default_value = ".", help = TARGET_HELP)]
    pub target: String,
    /// Report format.
    #[arg(long, value_enum, default_value_t = ReportFormat::Bundle)]
    pub format: ReportFormat,
    /// Where to write: a directory for bundles (default repodna-report), a file otherwise
    /// (default: standard output).
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<PathBuf>,
    /// Report theme.
    #[arg(long, value_enum)]
    pub theme: Option<ThemeArg>,
    /// Privacy preset.
    #[arg(long, value_enum)]
    pub privacy: Option<PrivacyArg>,
    /// Comma-separated sections to include (default: all).
    #[arg(long, value_delimiter = ',', value_name = "IDS")]
    pub sections: Vec<String>,
    /// Omit the RepoDNA credit line.
    #[arg(long)]
    pub no_branding: bool,
    /// CSV table: files, findings, metrics, hotspots, dependencies, contributors, languages.
    #[arg(long, default_value = "findings", value_name = "TABLE")]
    pub table: String,
    /// Replace existing files or write into a directory RepoDNA did not create.
    #[arg(long)]
    pub force: bool,
    /// How the analysis runs.
    #[command(flatten)]
    pub analysis: AnalysisArgs,
}

/// Output format of terminal views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ViewFormat {
    /// Readable text.
    Text,
    /// Markdown.
    Markdown,
    /// The relevant part of the artifact as JSON.
    Json,
}

/// Views such as `repodna architecture`.
#[derive(Debug, Args)]
pub struct ViewCmd {
    /// The repository or analysis to use (see the help text).
    #[arg(default_value = ".", help = TARGET_HELP)]
    pub target: String,
    /// Output format.
    #[arg(long, value_enum, default_value_t = ViewFormat::Text)]
    pub format: ViewFormat,
    /// Privacy preset.
    #[arg(long, value_enum, default_value_t = PrivacyArg::Local)]
    pub privacy: PrivacyArg,
    /// How the analysis runs.
    #[command(flatten)]
    pub analysis: AnalysisArgs,
}

/// `repodna findings`.
#[derive(Debug, Args)]
pub struct FindingsCmd {
    /// The repository or analysis to use (see the help text).
    #[arg(default_value = ".", help = TARGET_HELP)]
    pub target: String,
    /// Show findings at or above this severity.
    #[arg(long, value_enum, default_value_t = SeverityArg::Info)]
    pub severity: SeverityArg,
    /// Show only rules starting with this prefix, e.g. `security.`.
    #[arg(long, value_name = "PREFIX")]
    pub rule: Option<String>,
    /// Include suppressed findings.
    #[arg(long)]
    pub include_suppressed: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = ViewFormat::Text)]
    pub format: ViewFormat,
    /// Privacy preset.
    #[arg(long, value_enum, default_value_t = PrivacyArg::Local)]
    pub privacy: PrivacyArg,
    /// How the analysis runs.
    #[command(flatten)]
    pub analysis: AnalysisArgs,
}

/// `repodna show`.
#[derive(Debug, Args)]
pub struct ShowCmd {
    /// The repository or analysis to use (see the help text).
    #[arg(default_value = ".", help = TARGET_HELP)]
    pub target: String,
    /// Comma-separated section identifiers, e.g. `summary,tests,build` (`repodna show
    /// --list` prints them).
    #[arg(
        long,
        value_delimiter = ',',
        value_name = "IDS",
        required_unless_present = "list"
    )]
    pub section: Vec<String>,
    /// List the section identifiers.
    #[arg(long)]
    pub list: bool,
    /// Output format (text or markdown).
    #[arg(long, value_enum, default_value_t = ViewFormat::Text)]
    pub format: ViewFormat,
    /// Privacy preset.
    #[arg(long, value_enum, default_value_t = PrivacyArg::Local)]
    pub privacy: PrivacyArg,
    /// How the analysis runs.
    #[command(flatten)]
    pub analysis: AnalysisArgs,
}

/// Comparison formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CompareFormat {
    /// Readable text.
    Text,
    /// Markdown.
    Markdown,
    /// JSON.
    Json,
    /// A self-contained HTML page.
    Html,
}

/// `repodna compare`.
#[derive(Debug, Args)]
pub struct CompareCmd {
    /// Two or more targets (directories, URLs, artifact files, or stored repositories).
    #[arg(num_args = 2.., required = true, value_name = "TARGET")]
    pub targets: Vec<String>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = CompareFormat::Text)]
    pub format: CompareFormat,
    /// Write to this file instead of standard output.
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    /// HTML theme.
    #[arg(long, value_enum, default_value_t = ThemeArg::Professional)]
    pub theme: ThemeArg,
    /// Privacy preset applied to every analysis.
    #[arg(long, value_enum, default_value_t = PrivacyArg::Local)]
    pub privacy: PrivacyArg,
    /// Replace an existing output file that RepoDNA did not create.
    #[arg(long)]
    pub force: bool,
    /// How analyses run.
    #[command(flatten)]
    pub analysis: AnalysisArgs,
}

/// Card formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CardFormat {
    /// Scalable vector graphics.
    Svg,
    /// A 2x PNG image.
    Png,
}

/// `repodna card`.
#[derive(Debug, Args)]
pub struct CardCmd {
    /// The repository or analysis to use (see the help text).
    #[arg(default_value = ".", help = TARGET_HELP)]
    pub target: String,
    /// Image format.
    #[arg(long, value_enum, default_value_t = CardFormat::Svg)]
    pub format: CardFormat,
    /// Use the dark palette.
    #[arg(long)]
    pub dark: bool,
    /// Omit the RepoDNA credit line.
    #[arg(long)]
    pub no_branding: bool,
    /// Output file (default: dna-card.svg or dna-card.png).
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    /// Privacy preset.
    #[arg(long, value_enum, default_value_t = PrivacyArg::Local)]
    pub privacy: PrivacyArg,
    /// Replace an existing file that RepoDNA did not create.
    #[arg(long)]
    pub force: bool,
    /// How the analysis runs.
    #[command(flatten)]
    pub analysis: AnalysisArgs,
}

/// `repodna badge`.
#[derive(Debug, Args)]
pub struct BadgeCmd {
    /// The repository or analysis to use (see the help text).
    #[arg(default_value = ".", help = TARGET_HELP)]
    pub target: String,
    /// Badges to write: all, dna, languages, architecture, activity, or tests.
    #[arg(long, default_value = "all", value_name = "KIND")]
    pub kind: String,
    /// Directory for the badge files.
    #[arg(short, long, default_value = "badges", value_name = "DIR")]
    pub output: PathBuf,
    /// Replace existing files that RepoDNA did not create.
    #[arg(long)]
    pub force: bool,
    /// How the analysis runs.
    #[command(flatten)]
    pub analysis: AnalysisArgs,
}

/// `repodna onboarding`.
#[derive(Debug, Args)]
pub struct OnboardingCmd {
    /// The repository or analysis to use (see the help text).
    #[arg(default_value = ".", help = TARGET_HELP)]
    pub target: String,
    /// Directory for the guide.
    #[arg(short, long, default_value = "repodna-onboarding", value_name = "DIR")]
    pub output: PathBuf,
    /// Privacy preset.
    #[arg(long, value_enum, default_value_t = PrivacyArg::Local)]
    pub privacy: PrivacyArg,
    /// Write into an existing directory that RepoDNA did not create.
    #[arg(long)]
    pub force: bool,
    /// How the analysis runs.
    #[command(flatten)]
    pub analysis: AnalysisArgs,
}

/// Explanation topics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AboutArg {
    /// The whole repository.
    Repository,
    /// The inferred architecture.
    Architecture,
    /// How the repository evolved.
    History,
    /// Dependency relationships.
    Dependencies,
    /// An onboarding guide.
    Onboarding,
}

/// Explanation output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ExplainFormat {
    /// Readable text.
    Text,
    /// Markdown.
    Markdown,
    /// JSON with provenance.
    Json,
}

/// `repodna explain`.
#[derive(Debug, Args)]
pub struct ExplainCmd {
    /// The repository or analysis to use (see the help text).
    #[arg(default_value = ".", help = TARGET_HELP)]
    pub target: String,
    /// What to explain.
    #[arg(long, value_enum, default_value_t = AboutArg::Repository, conflicts_with_all = ["module", "hotspot", "ask"])]
    pub about: AboutArg,
    /// Explain one module or directory.
    #[arg(long, value_name = "PATH", conflicts_with_all = ["hotspot", "ask"])]
    pub module: Option<String>,
    /// Explain why a file is (or is not) a hotspot.
    #[arg(long, value_name = "PATH", conflicts_with = "ask")]
    pub hotspot: Option<String>,
    /// Ask a question about the repository.
    #[arg(long, value_name = "QUESTION")]
    pub ask: Option<String>,
    /// Print what would be sent, without contacting any provider.
    #[arg(long)]
    pub dry_run: bool,
    /// Allow a provider that sends evidence off this machine, for this run.
    #[arg(long)]
    pub allow_remote_ai: bool,
    /// Ask the provider again instead of reusing a cached explanation.
    #[arg(long)]
    pub fresh: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = ExplainFormat::Text)]
    pub format: ExplainFormat,
    /// Write to this file instead of standard output.
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    /// Replace an existing output file that RepoDNA did not create.
    #[arg(long)]
    pub force: bool,
    /// How the analysis runs.
    #[command(flatten)]
    pub analysis: AnalysisArgs,
}

/// CI failure threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FailOn {
    /// Never fail (report only).
    Never,
    /// Fail on informational findings and above.
    Info,
    /// Fail on attention signals and above.
    Attention,
    /// Fail on warnings and above.
    Warning,
    /// Fail on critical findings.
    Critical,
}

/// CI output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CiFormat {
    /// Plain text.
    Text,
    /// Markdown, for job summaries.
    Markdown,
    /// JSON.
    Json,
    /// GitHub Actions annotations plus a text summary.
    Github,
}

/// `repodna ci`.
#[derive(Debug, Args)]
pub struct CiCmd {
    /// The repository or analysis to use (see the help text).
    #[arg(default_value = ".", help = TARGET_HELP)]
    pub target: String,
    /// Fail (exit code 5) when findings at or above this severity exist.
    #[arg(long, value_enum, default_value_t = FailOn::Never)]
    pub fail_on: FailOn,
    /// Compare with this baseline artifact, e.g. from the base branch.
    #[arg(long, value_name = "FILE")]
    pub baseline: Option<PathBuf>,
    /// With --baseline, fail only on findings the baseline does not have.
    #[arg(long, requires = "baseline")]
    pub new_only: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = CiFormat::Text)]
    pub format: CiFormat,
    /// Also write the Markdown summary to this file (for example $GITHUB_STEP_SUMMARY).
    #[arg(long, value_name = "FILE")]
    pub summary_file: Option<PathBuf>,
    /// How the analysis runs.
    #[command(flatten)]
    pub analysis: AnalysisArgs,
}

/// `repodna export`.
#[derive(Debug, Args)]
pub struct ExportCmd {
    /// The repository or analysis to use (see the help text).
    #[arg(default_value = ".", help = TARGET_HELP)]
    pub target: String,
    /// Output file (default: repodna-<name>-<date>.repodna).
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    /// Privacy preset.
    #[arg(long, value_enum, default_value_t = PrivacyArg::Share)]
    pub privacy: PrivacyArg,
    /// Replace an existing file that RepoDNA did not create.
    #[arg(long)]
    pub force: bool,
    /// How the analysis runs.
    #[command(flatten)]
    pub analysis: AnalysisArgs,
}

/// `repodna import`.
#[derive(Debug, Args)]
pub struct ImportCmd {
    /// The artifact file.
    pub file: PathBuf,
}

/// Formats for listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ListFormat {
    /// Readable text.
    Text,
    /// JSON.
    Json,
}

/// `repodna list`.
#[derive(Debug, Args)]
pub struct ListCmd {
    /// Output format.
    #[arg(long, value_enum, default_value_t = ListFormat::Text)]
    pub format: ListFormat,
}

/// `repodna init`.
#[derive(Debug, Args)]
pub struct InitCmd {
    /// The repository to write repodna.toml into.
    #[arg(default_value = ".")]
    pub directory: PathBuf,
    /// Replace an existing repodna.toml.
    #[arg(long)]
    pub force: bool,
}

/// `repodna config`.
#[derive(Debug, Args)]
pub struct ConfigCmd {
    /// What to do.
    #[command(subcommand)]
    pub action: ConfigAction,
}

/// Configuration actions.
#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Print the path of your user configuration file.
    Path,
    /// Print the effective configuration for a repository and where it came from.
    Show {
        /// The repository whose repodna.toml applies.
        #[arg(default_value = ".")]
        directory: PathBuf,
        /// Print JSON instead of TOML.
        #[arg(long)]
        json: bool,
    },
    /// Create your user configuration file from a commented template.
    Init {
        /// Replace an existing user configuration file.
        #[arg(long)]
        force: bool,
    },
    /// Check every configuration layer for a repository.
    Validate {
        /// The repository whose repodna.toml is checked.
        #[arg(default_value = ".")]
        directory: PathBuf,
    },
}

/// `repodna plugins`.
#[derive(Debug, Args)]
pub struct PluginsCmd {
    /// What to do (default: list).
    #[command(subcommand)]
    pub action: Option<PluginsAction>,
}

/// Plugin actions.
#[derive(Debug, Subcommand)]
pub enum PluginsAction {
    /// List discovered plugins and whether they are enabled.
    List,
    /// Print the user plugin directory.
    Path,
    /// Show a plugin's manifest and permissions.
    Show {
        /// Plugin name.
        name: String,
    },
    /// Enable a plugin in your user configuration.
    Enable {
        /// Plugin name.
        name: String,
    },
    /// Disable a plugin in your user configuration.
    Disable {
        /// Plugin name.
        name: String,
    },
    /// Validate a plugin directory without running it.
    Check {
        /// The plugin directory.
        directory: PathBuf,
    },
}

/// `repodna cache`.
#[derive(Debug, Args)]
pub struct CacheCmd {
    /// What to do (default: stats).
    #[command(subcommand)]
    pub action: Option<CacheAction>,
}

/// Cache actions.
#[derive(Debug, Subcommand)]
pub enum CacheAction {
    /// Show storage and cache sizes.
    Stats {
        /// Print JSON.
        #[arg(long)]
        json: bool,
    },
    /// Clear the per-file analysis cache and cached explanations.
    Clear,
    /// Check the database, set a damaged one aside, and rebuild the index from stored
    /// artifacts.
    Repair,
    /// Delete the database and cache (stored artifacts are kept and can be re-indexed).
    Reset {
        /// Confirm the reset.
        #[arg(long)]
        yes: bool,
    },
}

/// `repodna clean`.
#[derive(Debug, Args)]
pub struct CleanCmd {
    /// A stored repository (name, path, or identifier).
    #[arg(required_unless_present = "all")]
    pub target: Option<String>,
    /// Clean every stored repository.
    #[arg(long, conflicts_with = "target")]
    pub all: bool,
    /// Keep the newest N analyses of each repository.
    #[arg(long, default_value_t = 0, value_name = "N")]
    pub keep: usize,
    /// Delete without asking; without it, only show what would be deleted.
    #[arg(long)]
    pub yes: bool,
}

/// `repodna doctor`.
#[derive(Debug, Args)]
pub struct DoctorCmd {
    /// Print JSON.
    #[arg(long)]
    pub json: bool,
}

/// `repodna version`.
#[derive(Debug, Args)]
pub struct VersionCmd {
    /// Print JSON.
    #[arg(long)]
    pub json: bool,
}

/// `repodna serve`.
#[derive(Debug, Args)]
pub struct ServeCmd {
    /// Port on 127.0.0.1 (0 picks a free port).
    #[arg(long, default_value_t = 7878)]
    pub port: u16,
    /// Serve the web interface from this directory instead of the built-in files.
    #[arg(long, value_name = "DIR")]
    pub web_dir: Option<PathBuf>,
    /// Do not allow starting analyses from the browser.
    #[arg(long)]
    pub no_scan: bool,
    /// How analyses started from the browser run.
    #[command(flatten)]
    pub analysis: AnalysisArgs,
}

/// Which schema to print.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SchemaKind {
    /// The analysis artifact (`repodna.json`, `.repodna` files).
    Artifact,
    /// The configuration file (`repodna.toml` and the user configuration).
    Config,
}

/// `repodna schema`.
#[derive(Debug, Args)]
pub struct SchemaCmd {
    /// Which schema.
    #[arg(value_enum, default_value_t = SchemaKind::Artifact)]
    pub kind: SchemaKind,
}

/// `repodna completions`.
#[derive(Debug, Args)]
pub struct CompletionsCmd {
    /// The shell.
    #[arg(value_enum)]
    pub shell: clap_complete::Shell,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn arguments_are_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_common_invocations() {
        let cli =
            Cli::try_parse_from(["repodna", "scan", ".", "--profile", "deep", "--quiet"]).unwrap();
        assert!(cli.global.quiet);
        let Command::Analyze(analyze) = cli.command else {
            panic!("expected analyze");
        };
        assert_eq!(analyze.analysis.profile, Some(AnalysisProfile::Deep));
        assert!(Cli::try_parse_from(["repodna", "compare", "a"]).is_err());
        assert!(Cli::try_parse_from(["repodna", "analyze", "--profile", "nope"]).is_err());
        assert!(
            Cli::try_parse_from(["repodna", "explain", "--module", "a", "--ask", "b"]).is_err()
        );
        let cli = Cli::try_parse_from(["repodna", "ci", "--fail-on", "warning"]).unwrap();
        assert!(matches!(cli.command, Command::Ci(ref ci) if ci.fail_on == FailOn::Warning));
    }
}
