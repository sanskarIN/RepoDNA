//! Command implementations and the context they share.

pub mod analyze;
pub mod ci;
pub mod compare;
pub mod doctor;
pub mod manage;
pub mod plugins;
pub mod report;
pub mod serve;
pub mod views;

use std::io::Write;
use std::path::{Path, PathBuf};

use repodna_app::{
    AnalyzeOptions, AppError, AppPaths, ConfigOptions, Loaded, Overrides, Source, write_file,
};
use repodna_core::CancellationToken;
use repodna_core::config::Config;
use repodna_core::model::artifact::RepositoryDna;
use repodna_engine::{FetchOptions, Progress};
use repodna_git::url::UrlPolicy;

use crate::cli::{AnalysisArgs, GlobalArgs};
use crate::progress::Reporter;
use crate::term::Style;

/// Everything commands share.
pub struct Ctx {
    /// Global options.
    pub global: GlobalArgs,
    /// Styling for standard output.
    pub out: Style,
    /// Styling for standard error.
    pub err: Style,
    /// Storage and configuration locations.
    pub paths: AppPaths,
    /// Set when the user presses Ctrl+C.
    pub cancel: CancellationToken,
    /// Progress output, unless quiet.
    pub reporter: Option<Reporter>,
}

impl Ctx {
    /// Configuration layers and overrides for a command.
    pub fn config_options(&self, analysis: &AnalysisArgs) -> ConfigOptions {
        ConfigOptions {
            explicit: self.global.config.clone(),
            ignore_user_config: self.global.no_user_config,
            ignore_project_config: self.global.no_project_config,
            overrides: Overrides {
                profile: analysis.profile,
                cache: analysis.no_cache.then_some(false),
                anonymize_contributors: analysis.anonymize.then_some(true),
                include_commit_messages: analysis.no_commit_messages.then_some(false),
                plugins: analysis.plugins.clone(),
                plugin_dirs: analysis.plugin_dirs.clone(),
                threads: analysis.threads,
                max_commits: analysis.max_commits,
            },
        }
    }

    /// Analysis options for `input`.
    pub fn analyze_options(&self, input: &str, analysis: &AnalysisArgs) -> AnalyzeOptions {
        AnalyzeOptions {
            input: input.to_owned(),
            config: self.config_options(analysis),
            fetch: FetchOptions {
                url_policy: UrlPolicy {
                    allow_insecure: analysis.allow_insecure_urls,
                    allow_private_hosts: analysis.allow_private_hosts,
                },
                clone_depth: analysis.clone_depth,
            },
            no_store: analysis.no_store,
            reproducible: analysis.reproducible,
            reference_time: None,
        }
    }

    /// The progress sink for analyses.
    pub fn progress(&self) -> Progress {
        self.reporter
            .as_ref()
            .map_or_else(Progress::default, Reporter::progress)
    }

    /// Prints a status line on standard error (not in quiet mode).
    pub fn note(&self, text: &str) {
        if let Some(reporter) = &self.reporter {
            reporter.line(text);
        }
    }

    /// Prints a warning on standard error (not in quiet mode).
    pub fn warn(&self, text: &str) {
        if let Some(reporter) = &self.reporter {
            reporter.line(&format!("{} {text}", self.err.yellow("warning:")));
        }
    }

    /// Loads a target, analyzing it when needed, and says where the data came from.
    pub fn load(&self, target: &str, analysis: &AnalysisArgs) -> Result<Loaded, AppError> {
        let template = self.analyze_options(target, analysis);
        if matches!(
            repodna_app::resolve_target(target),
            repodna_app::Target::Analyze(_)
        ) {
            self.note(&format!(
                "{} {} · analyzing {}",
                self.err.bold("RepoDNA"),
                env!("CARGO_PKG_VERSION"),
                target
            ));
        }
        let loaded = repodna_app::load_target(
            &self.paths,
            target,
            &template,
            self.progress(),
            &self.cancel,
        );
        if let Some(reporter) = &self.reporter {
            reporter.finish();
        }
        let loaded = loaded?;
        match &loaded.source {
            Source::Fresh(outcome) => {
                for warning in &outcome.warnings {
                    self.warn(warning);
                }
            }
            Source::Artifact { path, warnings } => {
                self.note(&self.err.dim(&format!(
                    "Reading {}: a snapshot analyzed {}{}, not the current state of the repository.",
                    path.display(),
                    loaded.dna.analysis_metadata.generated_at.date_string(),
                    revision_suffix(&loaded.dna)
                )));
                for warning in warnings {
                    self.warn(warning);
                }
            }
            Source::Stored(scan) => {
                self.note(&self.err.dim(&format!(
                    "Using the stored analysis of {} from {}{}. Pass a path to analyze again.",
                    loaded.dna.identity.name,
                    scan.generated_at.date_string(),
                    revision_suffix(&loaded.dna)
                )));
            }
        }
        Ok(loaded)
    }

    /// The effective configuration for a target (repository configuration applies to
    /// local directories only).
    pub fn config_for(&self, target: &str, analysis: &AnalysisArgs) -> Result<Config, AppError> {
        let path = Path::new(target);
        let root = path.is_dir().then_some(path);
        Ok(repodna_app::load_config(&self.paths, root, &self.config_options(analysis))?.config)
    }

    /// Writes `text` to `output`, or prints it when no output is given.
    pub fn emit(&self, output: Option<&PathBuf>, text: &str, force: bool) -> Result<(), AppError> {
        match output {
            Some(path) => {
                write_file(path, text.as_bytes(), force)?;
                self.note(&format!("Wrote {}", path.display()));
                Ok(())
            }
            None => print(text),
        }
    }
}

/// Prints to standard output, treating a closed pipe as success.
pub fn print(text: &str) -> Result<(), AppError> {
    let mut out = std::io::stdout().lock();
    let result = out
        .write_all(text.as_bytes())
        .and_then(|()| {
            if text.ends_with('\n') {
                Ok(())
            } else {
                out.write_all(b"\n")
            }
        })
        .and_then(|()| out.flush());
    match result {
        Err(error) if error.kind() != std::io::ErrorKind::BrokenPipe => Err(AppError::internal(
            format!("cannot write to standard output: {error}"),
        )),
        _ => Ok(()),
    }
}

/// ` (revision abc123def456)` or nothing.
pub fn revision_suffix(dna: &RepositoryDna) -> String {
    dna.analysis_metadata
        .revision
        .as_deref()
        .map(|revision| format!(" (revision {})", &revision[..revision.len().min(12)]))
        .unwrap_or_default()
}

/// Serializes a value as pretty JSON.
pub fn to_json<T: serde::Serialize + ?Sized>(value: &T) -> Result<String, AppError> {
    serde_json::to_string_pretty(value)
        .map(|mut json| {
            json.push('\n');
            json
        })
        .map_err(|error| AppError::internal(error.to_string()))
}
