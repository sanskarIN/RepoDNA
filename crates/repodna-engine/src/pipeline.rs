//! The analysis pipeline: resolves the input, runs every enabled stage, and assembles the
//! artifact.
//!
//! Stages that cannot run are recorded instead of failing the analysis: a missing Git
//! installation makes the Git section unavailable, and a stage the profile does not enable
//! is recorded as skipped. Only invalid configuration, an unusable input, and cancellation
//! stop an analysis.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use repodna_core::cancel::CancellationToken;
use repodna_core::config::{Config, Stage, StageSet, apply_suppressions};
use repodna_core::finding::{Finding, FindingCategory, normalize_findings};
use repodna_core::hash::stable_id;
use repodna_core::model::SectionStatus;
use repodna_core::model::artifact::RepositoryDna;
use repodna_core::model::identity::RepositoryIdentity;
use repodna_core::model::metadata::{
    AnalysisMetadata, AnalyzerRun, AnalyzerStatus, DataSource, DataSourceKind, InputKind,
    PlatformInfo, PrivacyInfo,
};
use repodna_core::model::project::{CommandCandidate, CommandPurpose, ExecutionResult};
use repodna_core::time::Timestamp;
use repodna_dependencies::DependencyAnalysis;
use repodna_git::url::parse_remote;
use repodna_git::{GitError, GitRunner};
use repodna_parser::{LanguageRegistry, LanguageSpec};
use repodna_project::execute::{ExecutionOptions, execute};
use repodna_quality::findings::hotspot_findings;

use crate::EngineError;
use crate::code::{
    entrypoints, hotspots, run_architecture, run_project, run_quality, run_security,
};
use crate::deps::{
    EXTERNAL_CONCENTRATION_SHARE, complete_report, dependency_findings, run_dependencies,
};
use crate::fingerprint::{fingerprint, metrics};
use crate::history::{GitStage, git_findings, run_git_stage};
use crate::input::{FetchOptions, InputSpec, prepare_input};
use crate::insights::build_insights;
use crate::progress::{Progress, ProgressEvent};
use crate::scan::{AnalysisCache, ScanOptions, scan_files};
use crate::structure::{language_report, structure_findings, structure_report};
use crate::timeline::{TimelineInput, run_timeline};

/// Work that runs after the built-in stages while the input is still on disk, such as
/// plugins.
pub trait Extension: Send + Sync {
    /// Name used in warnings, e.g. `plugin licenses`.
    fn name(&self) -> String;

    /// Runs the extension. It may add findings, metrics, and plugin records to `dna`. An error
    /// is recorded as a warning and never fails the analysis.
    fn run(
        &self,
        root: &Path,
        dna: &mut RepositoryDna,
        cancel: &CancellationToken,
    ) -> Result<(), String>;
}

/// Everything needed to run an analysis.
pub struct AnalysisRequest {
    /// What to analyze.
    pub input: InputSpec,
    /// The effective configuration.
    pub config: Config,
    /// Configuration layers that contributed, recorded in the artifact.
    pub config_sources: Vec<String>,
    /// Warnings raised while loading the configuration.
    pub config_warnings: Vec<String>,
    /// How remote inputs are fetched.
    pub fetch: FetchOptions,
    /// Additional language definitions (declarative language plugins).
    pub languages: Vec<LanguageSpec>,
    /// Work that runs after the built-in stages when the plugins stage is enabled.
    pub extensions: Vec<Box<dyn Extension>>,
    /// Cache of per-file analysis results, used when `performance.cache` is enabled.
    pub cache: Option<Arc<dyn AnalysisCache>>,
    /// Receives progress events.
    pub progress: Progress,
    /// Time the analysis is measured against. Defaults to now, or `SOURCE_DATE_EPOCH` when
    /// it is set.
    pub reference_time: Option<Timestamp>,
    /// Record zero durations so that repeated runs over the same input produce identical
    /// artifacts.
    pub reproducible: bool,
}

impl AnalysisRequest {
    /// A request with default options.
    pub fn new(input: InputSpec, config: Config) -> Self {
        Self {
            input,
            config,
            config_sources: vec!["defaults".to_owned()],
            config_warnings: Vec::new(),
            fetch: FetchOptions::default(),
            languages: Vec::new(),
            extensions: Vec::new(),
            cache: None,
            progress: Progress::default(),
            reference_time: None,
            reproducible: false,
        }
    }
}

/// Records stage outcomes and emits progress events.
struct Recorder<'a> {
    progress: &'a Progress,
    reproducible: bool,
    runs: Vec<AnalyzerRun>,
}

impl Recorder<'_> {
    fn begin(&self, stage: Stage) -> Instant {
        self.progress.emit(ProgressEvent::StageStarted(stage));
        Instant::now()
    }

    fn end(
        &mut self,
        stage: Stage,
        started: Instant,
        status: AnalyzerStatus,
        message: Option<String>,
    ) {
        let duration = started.elapsed();
        self.progress.emit(ProgressEvent::StageFinished {
            stage,
            status,
            duration,
        });
        self.push(stage, status, duration, message);
    }

    fn record(&mut self, stage: Stage, status: AnalyzerStatus, message: impl Into<String>) {
        self.push(stage, status, Duration::ZERO, Some(message.into()));
    }

    fn push(
        &mut self,
        stage: Stage,
        status: AnalyzerStatus,
        duration: Duration,
        message: Option<String>,
    ) {
        self.runs.push(AnalyzerRun {
            stage: stage.id().to_owned(),
            label: stage.label().to_owned(),
            status,
            duration_ms: if self.reproducible {
                0
            } else {
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
            },
            message,
        });
    }
}

/// Analyzer status for a finished section.
fn status_of(section: SectionStatus) -> AnalyzerStatus {
    match section {
        SectionStatus::Analyzed => AnalyzerStatus::Completed,
        SectionStatus::Partial => AnalyzerStatus::Partial,
        SectionStatus::Skipped | SectionStatus::Unavailable => AnalyzerStatus::Skipped,
    }
}

fn not_enabled(config: &Config) -> String {
    format!(
        "Not enabled by the {} profile or the configuration.",
        config.analysis.profile
    )
}

/// Runs the first detected command of each purpose, in order, until one fails.
fn run_commands(
    root: &Path,
    candidates: &mut [CommandCandidate],
    purposes: &[CommandPurpose],
    options: &ExecutionOptions,
    cancel: &CancellationToken,
) -> Vec<ExecutionResult> {
    let mut results = Vec::new();
    for purpose in purposes {
        let Some(candidate) = candidates.iter_mut().find(|c| c.purpose == *purpose) else {
            continue;
        };
        let result = execute(root, candidate, options, cancel);
        candidate.verified = result.success;
        let stop = !result.success;
        results.push(result);
        if stop {
            break;
        }
    }
    results
}

/// Module membership as `(module id, member paths)`, sorted by module.
fn module_members(paths: &[&str], module_of: &[Option<String>]) -> Vec<(String, Vec<String>)> {
    let mut members: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (path, module) in paths.iter().zip(module_of) {
        if let Some(module) = module {
            members.entry(module).or_default().push((*path).to_owned());
        }
    }
    members
        .into_iter()
        .map(|(module, paths)| (module.to_owned(), paths))
        .collect()
}

fn keep_project_finding(finding: &Finding, config: &Config) -> bool {
    let analysis = &config.analysis;
    match finding.category {
        FindingCategory::Tests => analysis.include_tests,
        FindingCategory::Build => analysis.include_build,
        FindingCategory::Documentation => analysis.include_docs,
        _ => true,
    }
}

fn privacy(config: &Config, network_used: bool, git_ran: bool, security_ran: bool) -> PrivacyInfo {
    let mut redactions = vec!["Credentials are removed from remote URLs.".to_owned()];
    if security_ran {
        redactions.push(
            "Secret candidates are stored without their values, as keyed fingerprints.".to_owned(),
        );
    }
    if git_ran {
        redactions.push("Contributor e-mail addresses are not stored.".to_owned());
        if config.privacy.anonymize_contributors {
            redactions.push("Contributor names are replaced with pseudonyms.".to_owned());
        }
        if !config.privacy.include_commit_messages {
            redactions.push("Commit messages are omitted.".to_owned());
        }
    }
    PrivacyInfo {
        telemetry: false,
        network_used,
        remote_ai: false,
        preset: repodna_core::config::PrivacyPreset::Local,
        redactions,
    }
}

/// Absolute forms of the input's location, longest first.
fn location_needles(roots: &[&Path]) -> Vec<String> {
    let mut needles: Vec<String> = Vec::new();
    for root in roots {
        let mut candidates = vec![root.to_path_buf()];
        if let Ok(canonical) = root.canonicalize() {
            candidates.push(canonical);
        }
        if let Ok(absolute) = std::path::absolute(root) {
            candidates.push(absolute);
        }
        for candidate in candidates {
            let text = candidate.display().to_string();
            let text = text.trim_end_matches(['/', '\\']).to_owned();
            if candidate.is_absolute() && text.len() > 1 && !needles.contains(&text) {
                needles.push(text);
            }
        }
    }
    needles.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    needles
}

/// Removes the input's local location from free text (notes, warnings, stage messages, and
/// command output), so artifacts never reveal where the repository lives on disk. Paths
/// below the location become relative; the location itself becomes `.`.
fn scrub_locations(dna: &mut RepositoryDna, needles: &[String]) {
    if needles.is_empty() {
        return;
    }
    let scrub = |text: &mut String| {
        for needle in needles {
            if text.contains(needle.as_str()) {
                *text = text
                    .replace(&format!("{needle}/"), "")
                    .replace(&format!("{needle}\\"), "")
                    .replace(needle.as_str(), ".");
            }
        }
    };
    let metadata = &mut dna.analysis_metadata;
    metadata.warnings.iter_mut().for_each(scrub);
    metadata
        .analyzers
        .iter_mut()
        .filter_map(|run| run.message.as_mut())
        .for_each(scrub);
    for notes in [
        &mut dna.structure.notes,
        &mut dna.languages.notes,
        &mut dna.architecture.notes,
        &mut dna.git.notes,
        &mut dna.code_quality.notes,
        &mut dna.dependencies.notes,
        &mut dna.tests.notes,
        &mut dna.builds.notes,
        &mut dna.docs.notes,
        &mut dna.security.notes,
        &mut dna.evolution.notes,
        &mut dna.similarity.notes,
    ] {
        notes.iter_mut().for_each(scrub);
    }
    for execution in dna
        .builds
        .executions
        .iter_mut()
        .chain(dna.tests.executions.iter_mut())
    {
        execution.output_tail.iter_mut().for_each(scrub);
    }
}

/// Runs an analysis and returns the artifact.
///
/// # Errors
///
/// Returns an error when the configuration is invalid, the input cannot be prepared (it
/// does not exist, cannot be cloned, or cannot be extracted), discovery fails, or the
/// analysis is cancelled. Failures of individual stages are recorded in the artifact.
pub fn analyze(
    request: &AnalysisRequest,
    cancel: &CancellationToken,
) -> Result<RepositoryDna, EngineError> {
    let started = Instant::now();
    let config = &request.config;
    let problems = config.validate();
    if !problems.is_empty() {
        return Err(EngineError::Config(problems.join("; ")));
    }
    let now = request
        .reference_time
        .unwrap_or_else(Timestamp::now_or_source_date_epoch);
    let stages: StageSet = config.effective_stages();
    let extended;
    let registry: &LanguageRegistry = if request.languages.is_empty() {
        LanguageRegistry::builtin()
    } else {
        extended = LanguageRegistry::builtin_with(request.languages.clone());
        &extended
    };
    let mut recorder = Recorder {
        progress: &request.progress,
        reproducible: request.reproducible,
        runs: Vec::new(),
    };
    let mut warnings: Vec<String> = request.config_warnings.clone();

    // Git is needed for history and to clone URLs.
    let wants_git = stages.contains(Stage::Git) || matches!(request.input, InputSpec::Url(_));
    let detected_git = wants_git.then(GitRunner::detect);
    let git = detected_git
        .as_ref()
        .and_then(|result| result.as_ref().ok());
    let prepared = prepare_input(&request.input, git, request.fetch, cancel)?;
    warnings.extend(prepared.notes.iter().cloned());
    let root = prepared.root.as_path();

    // Discovery and the file pass (parsing, tokenizing, and security scanning per file).
    let begun = recorder.begin(Stage::Discovery);
    let mut scan = scan_files(
        root,
        &ScanOptions {
            config,
            stages,
            registry,
            progress: &request.progress,
            cache: request
                .cache
                .as_deref()
                .filter(|_| config.performance.cache),
        },
        cancel,
    )?;
    let discovery_status = if scan.truncated || !scan.errors.is_empty() {
        AnalyzerStatus::Partial
    } else {
        AnalyzerStatus::Completed
    };
    let mut message = format!("{} files", scan.files.len());
    if scan.cache_hits > 0 {
        message.push_str(&format!(
            "; {} analyses reused from the cache",
            scan.cache_hits
        ));
    }
    recorder.end(Stage::Discovery, begun, discovery_status, Some(message));
    if !scan.errors.is_empty() {
        warnings.push(format!(
            "{} paths could not be read during discovery; see the structure notes.",
            scan.errors.len()
        ));
    }
    if stages.contains(Stage::Parsing) {
        let (status, message) = if scan.tokens_truncated {
            (
                AnalyzerStatus::Partial,
                "Parsed during the file pass. The token budget was reached, so duplication and similarity cover only part of the code.",
            )
        } else {
            (
                AnalyzerStatus::Completed,
                "Parsed during the file pass; its time is included in discovery.",
            )
        };
        recorder.record(Stage::Parsing, status, message);
    } else {
        recorder.record(Stage::Parsing, AnalyzerStatus::Skipped, not_enabled(config));
    }

    let mut dna = RepositoryDna::new(RepositoryIdentity::default(), AnalysisMetadata::default());
    dna.structure = structure_report(
        &scan,
        usize::try_from(config.analysis.max_symbols).unwrap_or(usize::MAX),
    );
    dna.languages = language_report(&scan, registry);
    let mut findings: Vec<Finding> = structure_findings(&dna.structure, &config.thresholds);
    let current_files: BTreeSet<String> = scan
        .files
        .iter()
        .map(|file| file.record.path.clone())
        .collect();

    // Git history.
    let mut git_stage: Option<GitStage> = None;
    if stages.contains(Stage::Git) {
        match (&detected_git, &prepared.git_root) {
            (Some(Ok(runner)), Some(git_root)) => {
                let begun = recorder.begin(Stage::Git);
                match run_git_stage(runner, git_root, &current_files, config, now, cancel) {
                    Ok(stage) => {
                        let message = format!("{} commits", stage.report.commit_count);
                        recorder.end(Stage::Git, begun, AnalyzerStatus::Completed, Some(message));
                        git_stage = Some(stage);
                    }
                    Err(GitError::Cancelled) => return Err(EngineError::Cancelled),
                    Err(error) => {
                        let message = format!("Git history could not be read: {error}");
                        dna.git.status = SectionStatus::Unavailable;
                        dna.git.notes.push(message.clone());
                        warnings.push(message.clone());
                        recorder.end(Stage::Git, begun, AnalyzerStatus::Failed, Some(message));
                    }
                }
            }
            (Some(Err(error)), _) => {
                let mut message = error.to_string();
                if let Some(hint) = error.hint() {
                    message = format!("{message}. {hint}");
                }
                dna.git.status = SectionStatus::Unavailable;
                dna.git.notes.push(message.clone());
                warnings.push(message.clone());
                recorder.record(Stage::Git, AnalyzerStatus::Skipped, message);
            }
            _ => {
                let message =
                    "The input is not the root of a Git repository, so history was not analyzed.";
                dna.git.status = SectionStatus::Unavailable;
                dna.git.notes.push(message.to_owned());
                recorder.record(Stage::Git, AnalyzerStatus::Skipped, message);
            }
        }
    } else {
        recorder.record(Stage::Git, AnalyzerStatus::Skipped, not_enabled(config));
    }

    // Manifests and lockfiles; they also feed architecture and project detection.
    let dependencies = if stages.contains(Stage::Dependencies) {
        let begun = recorder.begin(Stage::Dependencies);
        let dependencies = run_dependencies(&scan.contents);
        let message = format!(
            "{} manifests and lockfiles",
            dependencies.report.manifests.len()
        );
        recorder.end(
            Stage::Dependencies,
            begun,
            status_of(dependencies.report.status),
            Some(message),
        );
        dependencies
    } else {
        recorder.record(
            Stage::Dependencies,
            AnalyzerStatus::Skipped,
            not_enabled(config),
        );
        if stages.contains(Stage::Architecture) || stages.contains(Stage::Project) {
            run_dependencies(&scan.contents)
        } else {
            DependencyAnalysis::default()
        }
    };
    cancel.check()?;

    // Architecture.
    let architecture = if stages.contains(Stage::Architecture) {
        let begun = recorder.begin(Stage::Architecture);
        let output = run_architecture(&scan, &dependencies, config, cancel)?;
        let message = format!(
            "{} modules, {} resolved imports",
            output.report.modules.len(),
            output.report.resolved_imports
        );
        recorder.end(
            Stage::Architecture,
            begun,
            status_of(output.report.status),
            Some(message),
        );
        Some(output)
    } else {
        recorder.record(
            Stage::Architecture,
            AnalyzerStatus::Skipped,
            not_enabled(config),
        );
        None
    };
    match &architecture {
        Some(output) => {
            findings.extend(output.findings.iter().cloned());
            for (record, module) in dna.structure.files.iter_mut().zip(&output.module_of) {
                record.module.clone_from(module);
            }
            dna.structure.entrypoints = output.entrypoints.clone();
            dna.languages.interactions = output.interactions.clone();
        }
        None => dna.structure.entrypoints = entrypoints(&scan, &dependencies),
    }

    // Quality, duplication, and similarity.
    if stages.contains(Stage::Quality) {
        let begun = recorder.begin(Stage::Quality);
        let output = run_quality(
            &scan,
            architecture.as_ref(),
            git_stage.as_ref().map(|stage| &stage.report),
            config,
            stages,
            cancel,
        )?;
        recorder.end(Stage::Quality, begun, status_of(output.report.status), None);
        for (stage, status) in [
            (Stage::Duplication, output.report.duplication.status),
            (Stage::Similarity, output.similarity.status),
        ] {
            if stages.contains(stage) {
                recorder.record(
                    stage,
                    status_of(status),
                    "Runs within the quality stage; its time is included there.",
                );
            } else {
                recorder.record(stage, AnalyzerStatus::Skipped, not_enabled(config));
            }
        }
        dna.code_quality = output.report;
        dna.similarity = output.similarity;
        findings.extend(output.findings);
    } else {
        for stage in [Stage::Quality, Stage::Duplication, Stage::Similarity] {
            recorder.record(stage, AnalyzerStatus::Skipped, not_enabled(config));
        }
    }

    // Hotspots and history findings.
    if let Some(stage) = git_stage.as_mut() {
        let (ranked, candidates) = hotspots(&scan, &stage.report, architecture.as_ref(), config);
        findings.extend(hotspot_findings(&ranked, candidates));
        stage.report.hot_spots = ranked;
        findings.extend(git_findings(&stage.report, &config.thresholds));
    }

    // Tests, build, CI, environment, and documentation.
    if stages.contains(Stage::Project) {
        let begun = recorder.begin(Stage::Project);
        let mut output = run_project(&scan, &dependencies);
        let execution = &config.execution;
        let options = ExecutionOptions {
            timeout: Duration::from_secs(execution.timeout_seconds),
            max_output_lines: usize::try_from(execution.max_output_lines).unwrap_or(usize::MAX),
        };
        if execution.allow_build_commands {
            output.builds.executions = run_commands(
                root,
                &mut output.builds.commands,
                &[CommandPurpose::Install, CommandPurpose::Build],
                &options,
                cancel,
            );
        }
        if execution.allow_test_commands {
            output.tests.executions = run_commands(
                root,
                &mut output.tests.commands,
                &[CommandPurpose::Test],
                &options,
                cancel,
            );
        }
        cancel.check()?;
        let analysis = &config.analysis;
        if analysis.include_tests {
            dna.tests = output.tests;
        }
        if analysis.include_build {
            dna.builds = output.builds;
        }
        if analysis.include_docs {
            dna.docs = output.docs;
        }
        findings.extend(
            output
                .findings
                .into_iter()
                .filter(|finding| keep_project_finding(finding, config)),
        );
        recorder.end(Stage::Project, begun, AnalyzerStatus::Completed, None);
    } else {
        recorder.record(Stage::Project, AnalyzerStatus::Skipped, not_enabled(config));
    }

    // Security.
    let security_ran = stages.contains(Stage::Security);
    if security_ran {
        let begun = recorder.begin(Stage::Security);
        let (report, security) = run_security(&mut scan);
        let message = format!(
            "{} secret candidates, {} pattern candidates",
            report.secrets.len(),
            report.patterns.len()
        );
        recorder.end(
            Stage::Security,
            begun,
            status_of(report.status),
            Some(message),
        );
        dna.security = report;
        findings.extend(security);
    } else {
        recorder.record(
            Stage::Security,
            AnalyzerStatus::Skipped,
            not_enabled(config),
        );
    }

    // Dependencies completed with import concentration and manifest history.
    if stages.contains(Stage::Dependencies) {
        let mut report = dependencies.report.clone();
        let externals = architecture
            .as_ref()
            .map_or(&[][..], |output| &output.report.external[..]);
        let (history, latest) = git_stage.as_ref().map_or((&[][..], None), |stage| {
            (
                &stage.report.file_history[..],
                stage
                    .report
                    .last_commit
                    .as_ref()
                    .map(|commit| commit.timestamp),
            )
        });
        complete_report(
            &mut report,
            externals,
            history,
            latest,
            config.thresholds.stale_manifest_days,
        );
        findings.extend(dependency_findings(&report, EXTERNAL_CONCENTRATION_SHARE));
        dna.dependencies = report;
    }

    // Evolution and the Time Machine.
    let mut recent = None;
    let historical = stages.contains(Stage::HistoricalArchitecture);
    let mut historical_recorded = false;
    match (&git_stage, git, stages.contains(Stage::Evolution)) {
        (Some(stage), Some(runner), true) => {
            let begun = recorder.begin(Stage::Evolution);
            let paths: Vec<&str> = scan
                .files
                .iter()
                .map(|file| file.record.path.as_str())
                .collect();
            let modules = architecture
                .as_ref()
                .map(|output| module_members(&paths, &output.module_of))
                .unwrap_or_default();
            let current: HashSet<&str> = paths.iter().copied().collect();
            let timeline = run_timeline(
                &TimelineInput {
                    git: runner,
                    root: prepared.git_root.as_deref().unwrap_or(root),
                    stage,
                    modules: &modules,
                    current_files: &current,
                    current_dependencies: &dna.dependencies,
                    config,
                    registry,
                    historical_architecture: historical,
                },
                cancel,
            );
            match timeline {
                Ok(timeline) => {
                    let status = if timeline.notes.is_empty() {
                        status_of(timeline.evolution.report.status)
                    } else {
                        AnalyzerStatus::Partial
                    };
                    let message = format!(
                        "{} snapshots, {} events",
                        timeline.evolution.report.snapshots.len(),
                        timeline.evolution.report.events.len()
                    );
                    recorder.end(Stage::Evolution, begun, status, Some(message));
                    if historical {
                        recorder.record(
                            Stage::HistoricalArchitecture,
                            status,
                            "Runs within the evolution stage; its time is included there.",
                        );
                        historical_recorded = true;
                    }
                    dna.evolution = timeline.evolution.report;
                    dna.evolution.notes.extend(timeline.notes);
                    findings.extend(timeline.evolution.findings);
                    recent = timeline.recent;
                }
                Err(GitError::Cancelled) => return Err(EngineError::Cancelled),
                Err(error) => {
                    let message = format!("Evolution could not be reconstructed: {error}");
                    dna.evolution.status = SectionStatus::Unavailable;
                    dna.evolution.notes.push(message.clone());
                    warnings.push(message.clone());
                    recorder.end(
                        Stage::Evolution,
                        begun,
                        AnalyzerStatus::Failed,
                        Some(message),
                    );
                }
            }
        }
        (_, _, true) => {
            let message = "Git history is not available, so evolution was not reconstructed.";
            dna.evolution.status = SectionStatus::Unavailable;
            dna.evolution.notes.push(message.to_owned());
            recorder.record(Stage::Evolution, AnalyzerStatus::Skipped, message);
        }
        (_, _, false) => recorder.record(
            Stage::Evolution,
            AnalyzerStatus::Skipped,
            not_enabled(config),
        ),
    }
    if !historical_recorded {
        let message = if historical {
            "Evolution did not run, so no historical snapshots were sampled.".to_owned()
        } else {
            not_enabled(config)
        };
        recorder.record(
            Stage::HistoricalArchitecture,
            AnalyzerStatus::Skipped,
            message,
        );
    }
    let git_ran = git_stage.is_some();
    let git_version = git.map(|runner| runner.version().to_owned());
    let remotes = git_stage
        .as_ref()
        .map(|stage| stage.remotes.clone())
        .unwrap_or_default();
    let (revision, dirty) = git_stage.as_ref().map_or((None, None), |stage| {
        (stage.state.head.clone(), stage.state.dirty)
    });
    if let Some(stage) = git_stage {
        dna.git = stage.report;
    }
    if let Some(output) = architecture {
        dna.architecture = output.report;
    }
    dna.findings = findings;

    // Plugins and other extensions.
    if stages.contains(Stage::Plugins) && !request.extensions.is_empty() {
        let begun = recorder.begin(Stage::Plugins);
        let mut failures = Vec::new();
        for extension in &request.extensions {
            cancel.check()?;
            if let Err(error) = extension.run(root, &mut dna, cancel) {
                failures.push(format!("{}: {error}", extension.name()));
            }
        }
        cancel.check()?;
        let status = if failures.is_empty() {
            AnalyzerStatus::Completed
        } else {
            AnalyzerStatus::Partial
        };
        warnings.extend(failures.iter().cloned());
        let message = (!failures.is_empty()).then(|| failures.join("; "));
        recorder.end(Stage::Plugins, begun, status, message);
    } else {
        recorder.record(Stage::Plugins, AnalyzerStatus::Skipped, not_enabled(config));
    }

    // Suppressions, ordering, fingerprint, metrics, and insights.
    let suppressed = apply_suppressions(&mut dna.findings, &config.suppressions);
    normalize_findings(&mut dna.findings);
    dna.fingerprint = fingerprint(&dna);
    dna.metrics = metrics(&dna, &dna.fingerprint);
    if stages.contains(Stage::Insights) {
        let begun = recorder.begin(Stage::Insights);
        dna.insights = build_insights(&dna, recent);
        recorder.end(Stage::Insights, begun, AnalyzerStatus::Completed, None);
    } else {
        recorder.record(
            Stage::Insights,
            AnalyzerStatus::Skipped,
            not_enabled(config),
        );
    }

    // Identity and metadata.
    let origin = remotes
        .iter()
        .find(|remote| remote.name == "origin")
        .or_else(|| remotes.first())
        .map(|remote| parse_remote(&remote.url));
    dna.identity = RepositoryIdentity {
        name: origin
            .as_ref()
            .and_then(|remote| remote.name.clone())
            .unwrap_or_else(|| prepared.name.clone()),
        owner: origin.as_ref().and_then(|remote| remote.owner.clone()),
        remotes,
        default_branch: dna.git.default_branch.clone(),
        revision: revision.clone(),
        license: dna.docs.license.clone(),
        primary_languages: dna.languages.primary.clone(),
        description: dna.docs.description.clone(),
        is_git_repository: prepared.git_root.is_some() || root.join(".git").exists(),
        size_class: dna.structure.size_class,
    };
    let mut data_sources = vec![DataSource {
        kind: DataSourceKind::LocalFiles,
        name: "file system".to_owned(),
        detail: format!("{} files discovered", dna.structure.total_files),
        accessed_at: now,
    }];
    if prepared.info.kind == InputKind::GitUrl {
        data_sources.push(DataSource {
            kind: DataSourceKind::RemoteClone,
            name: "git clone".to_owned(),
            detail: prepared.info.display.clone(),
            accessed_at: now,
        });
    }
    if git_ran {
        data_sources.push(DataSource {
            kind: DataSourceKind::GitHistory,
            name: git_version.map_or_else(|| "git".to_owned(), |version| format!("git {version}")),
            detail: format!("{} commits read", dna.git.commit_count),
            accessed_at: now,
        });
    }
    let config_hash = config.hash();
    let generated = now.to_rfc3339();
    let run_id = stable_id(&[
        dna.fingerprint.dna_hash.as_str(),
        generated.as_str(),
        config_hash.as_str(),
        config.analysis.profile.id(),
    ]);
    let partial = recorder.runs.iter().any(|run| {
        matches!(
            run.status,
            AnalyzerStatus::Failed | AnalyzerStatus::Cancelled
        )
    });
    dna.analysis_metadata = AnalysisMetadata {
        id: run_id,
        generated_at: now,
        profile: config.analysis.profile,
        input: prepared.info.clone(),
        revision,
        dirty,
        analyzers: recorder.runs,
        duration_ms: if request.reproducible {
            0
        } else {
            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
        },
        platform: PlatformInfo::current(),
        config_hash,
        config_sources: request.config_sources.clone(),
        thresholds: config.thresholds.clone(),
        suppressions: config.suppressions.clone(),
        suppressed_findings: u32::try_from(suppressed).unwrap_or(u32::MAX),
        partial,
        warnings,
        data_sources,
        privacy: privacy(config, prepared.network_used, git_ran, security_ran),
        ai: None,
    };
    let mut roots = vec![prepared.root.as_path()];
    roots.extend(prepared.git_root.as_deref());
    scrub_locations(&mut dna, &location_needles(&roots));
    Ok(dna)
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::confidence::Confidence;
    use repodna_core::config::{AnalysisProfile, SuppressionRule};
    use repodna_core::severity::Severity;
    use repodna_testkit::{GitRepo, git_available, write_tree};

    const PROJECT: &[(&str, &str)] = &[
        (
            "Cargo.toml",
            "[package]\nname = \"widget\"\nversion = \"0.1.0\"\ndescription = \"A small widget library\"\n\n[dependencies]\nserde = \"1\"\n",
        ),
        (
            "src/main.rs",
            "mod util;\n\nfn main() {\n    util::greet();\n}\n",
        ),
        (
            "src/util.rs",
            "pub fn greet() {\n    if true {\n        println!(\"hi\");\n    }\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn works() {}\n}\n",
        ),
        (
            "README.md",
            "# Widget\n\n## Installation\n\ncargo install widget\n",
        ),
    ];

    fn request(root: &Path, profile: AnalysisProfile) -> AnalysisRequest {
        let mut config = Config::default();
        config.analysis.profile = profile;
        let mut request = AnalysisRequest::new(InputSpec::Directory(root.to_path_buf()), config);
        request.reference_time = Timestamp::from_ymd(2024, 6, 1);
        request.reproducible = true;
        request
    }

    fn stage_ids(dna: &RepositoryDna) -> Vec<&str> {
        dna.analysis_metadata
            .analyzers
            .iter()
            .map(|run| run.stage.as_str())
            .collect()
    }

    fn status(dna: &RepositoryDna, stage: Stage) -> AnalyzerStatus {
        dna.analysis_metadata
            .analyzers
            .iter()
            .find(|run| run.stage == stage.id())
            .map(|run| run.status)
            .unwrap()
    }

    #[test]
    fn quick_profile_analyzes_files_without_git() {
        let dir = tempfile::tempdir().unwrap();
        write_tree(dir.path(), PROJECT);
        let dna = analyze(
            &request(dir.path(), AnalysisProfile::Quick),
            &CancellationToken::new(),
        )
        .unwrap();
        let expected: Vec<&str> = Stage::ALL.iter().map(|stage| stage.id()).collect();
        assert_eq!(stage_ids(&dna), expected);
        assert_eq!(status(&dna, Stage::Discovery), AnalyzerStatus::Completed);
        assert_eq!(status(&dna, Stage::Git), AnalyzerStatus::Skipped);
        assert_eq!(dna.structure.total_files, 4);
        assert_eq!(dna.languages.primary, vec!["rust"]);
        assert_eq!(dna.git.status, SectionStatus::Skipped);
        assert_eq!(dna.architecture.status, SectionStatus::Skipped);
        assert_eq!(dna.dependencies.status, SectionStatus::Skipped);
        assert_eq!(dna.tests.status, SectionStatus::Analyzed);
        assert!(
            dna.structure
                .entrypoints
                .iter()
                .any(|e| e.path == "src/main.rs")
        );
        assert_eq!(
            dna.docs.description.as_deref(),
            Some("A small widget library")
        );
        assert_eq!(dna.identity.description, dna.docs.description);
        assert!(!dna.identity.is_git_repository);
        assert_eq!(dna.analysis_metadata.input.kind, InputKind::LocalDirectory);
        assert!(dna.fingerprint.dna_hash.starts_with("rdna1-"));
        assert!(!dna.insights.first_look.is_empty());
        assert!(dna.metrics.get("structure.files").is_some());
        assert!(!dna.analysis_metadata.partial);
        assert!(!dna.analysis_metadata.privacy.network_used);
    }

    #[test]
    fn standard_profile_reads_history_and_dependencies() {
        if !git_available() {
            return;
        }
        let repo = GitRepo::new();
        for (path, content) in PROJECT {
            repo.write(path, content);
        }
        repo.commit("initial", "Ana", "ana@example.test", "2024-01-10T10:00:00Z");
        repo.write("src/util.rs", "pub fn greet() {}\n");
        repo.commit("simplify", "Bo", "bo@example.test", "2024-05-10T10:00:00Z");
        repo.git(&[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/widget.git",
        ]);
        let dna = analyze(
            &request(repo.path(), AnalysisProfile::Standard),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(status(&dna, Stage::Git), AnalyzerStatus::Completed);
        assert_eq!(dna.git.commit_count, 2);
        assert_eq!(dna.evolution.status, SectionStatus::Analyzed);
        assert!(!dna.evolution.snapshots.is_empty());
        assert_eq!(dna.dependencies.status, SectionStatus::Analyzed);
        assert!(
            dna.dependencies
                .dependencies
                .iter()
                .any(|d| d.name == "serde")
        );
        assert_eq!(dna.architecture.status, SectionStatus::Analyzed);
        assert!(dna.structure.files.iter().any(|f| f.module.is_some()));
        assert_eq!(dna.identity.name, "widget");
        assert_eq!(dna.identity.owner.as_deref(), Some("acme"));
        assert!(dna.identity.is_git_repository);
        assert_eq!(dna.identity.revision, dna.analysis_metadata.revision);
        assert!(dna.identity.revision.is_some());
        assert_eq!(dna.analysis_metadata.input.kind, InputKind::GitRepository);
        assert!(
            dna.analysis_metadata
                .data_sources
                .iter()
                .any(|source| source.kind == DataSourceKind::GitHistory)
        );
        assert!(dna.insights.recent_changes.is_some());
        assert_eq!(
            status(&dna, Stage::HistoricalArchitecture),
            AnalyzerStatus::Skipped
        );
        let mut sorted = dna.findings.clone();
        normalize_findings(&mut sorted);
        assert_eq!(sorted, dna.findings);
    }

    #[test]
    fn reproducible_runs_produce_identical_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        write_tree(dir.path(), PROJECT);
        let run = || {
            let dna = analyze(
                &request(dir.path(), AnalysisProfile::Deep),
                &CancellationToken::new(),
            )
            .unwrap();
            serde_json::to_string(&dna).unwrap()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn applies_suppressions_and_runs_extensions() {
        struct AddFinding;
        impl Extension for AddFinding {
            fn name(&self) -> String {
                "test extension".to_owned()
            }
            fn run(
                &self,
                _root: &Path,
                dna: &mut RepositoryDna,
                _cancel: &CancellationToken,
            ) -> Result<(), String> {
                dna.findings.push(Finding::new(
                    "plugin.example",
                    "repository",
                    FindingCategory::Plugin,
                    Severity::Warning,
                    Confidence::Low,
                    "Example plugin finding",
                ));
                Ok(())
            }
        }
        struct Fails;
        impl Extension for Fails {
            fn name(&self) -> String {
                "broken extension".to_owned()
            }
            fn run(
                &self,
                _root: &Path,
                _dna: &mut RepositoryDna,
                _cancel: &CancellationToken,
            ) -> Result<(), String> {
                Err("exited with status 2".to_owned())
            }
        }
        let dir = tempfile::tempdir().unwrap();
        write_tree(dir.path(), PROJECT);
        let mut request = request(dir.path(), AnalysisProfile::Standard);
        request.config.plugins.enabled = vec!["example".to_owned()];
        request.config.suppressions = vec![SuppressionRule {
            rule: "docs.license-missing".to_owned(),
            path: None,
            id: None,
            reason: "License is added at release time".to_owned(),
            source: Some("test".to_owned()),
        }];
        request.extensions = vec![Box::new(AddFinding), Box::new(Fails)];
        let dna = analyze(&request, &CancellationToken::new()).unwrap();
        assert_eq!(status(&dna, Stage::Plugins), AnalyzerStatus::Partial);
        assert!(
            dna.analysis_metadata
                .warnings
                .iter()
                .any(|w| w == "broken extension: exited with status 2")
        );
        assert_eq!(dna.findings[0].rule, "plugin.example");
        let license = dna
            .findings
            .iter()
            .find(|f| f.rule == "docs.license-missing")
            .unwrap();
        assert!(license.is_suppressed());
        assert_eq!(dna.analysis_metadata.suppressed_findings, 1);
        assert_eq!(dna.finding_counts().suppressed, 1);
    }

    #[test]
    fn rejects_invalid_configuration_and_honors_cancellation() {
        let dir = tempfile::tempdir().unwrap();
        write_tree(dir.path(), PROJECT);
        let mut invalid = request(dir.path(), AnalysisProfile::Quick);
        invalid.config.analysis.max_file_bytes = 0;
        assert!(matches!(
            analyze(&invalid, &CancellationToken::new()),
            Err(EngineError::Config(_))
        ));
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            analyze(&request(dir.path(), AnalysisProfile::Quick), &cancel),
            Err(EngineError::Cancelled)
        ));
        let missing = request(&dir.path().join("missing"), AnalysisProfile::Quick);
        assert!(matches!(
            analyze(&missing, &CancellationToken::new()),
            Err(EngineError::NotFound(_))
        ));
    }

    #[test]
    fn scrubs_local_locations_from_messages() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        std::fs::create_dir(&root).unwrap();
        let needles = location_needles(&[root.as_path()]);
        assert!(!needles.is_empty());
        let mut dna =
            RepositoryDna::new(RepositoryIdentity::default(), AnalysisMetadata::default());
        let shown = needles[needles.len() - 1].clone();
        dna.analysis_metadata.warnings = vec![
            format!("could not read {shown}/src/secret.rs: permission denied"),
            format!("{shown} is not a Git repository"),
            "nothing to hide".to_owned(),
        ];
        dna.structure.notes = vec![format!(
            "IO error for {shown}{}a.txt",
            std::path::MAIN_SEPARATOR
        )];
        scrub_locations(&mut dna, &needles);
        assert_eq!(
            dna.analysis_metadata.warnings,
            vec![
                "could not read src/secret.rs: permission denied",
                ". is not a Git repository",
                "nothing to hide"
            ]
        );
        assert_eq!(dna.structure.notes, vec!["IO error for a.txt"]);
        assert!(location_needles(&[Path::new(".")]).iter().all(|n| n != "."));
    }

    #[test]
    fn records_why_history_is_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        write_tree(dir.path(), PROJECT);
        let dna = analyze(
            &request(dir.path(), AnalysisProfile::Standard),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(dna.git.status, SectionStatus::Unavailable);
        assert_eq!(dna.evolution.status, SectionStatus::Unavailable);
        assert_eq!(status(&dna, Stage::Git), AnalyzerStatus::Skipped);
        assert!(!dna.git.notes.is_empty());
        let git = dna
            .metrics
            .confidence
            .iter()
            .find(|c| c.section == "git")
            .unwrap();
        assert_eq!(git.confidence, Confidence::Unavailable);
    }
}
