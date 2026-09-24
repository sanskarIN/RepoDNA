//! Running analyses with configuration, plugins, the per-file cache, and local storage,
//! and loading earlier analyses from artifact files or the store.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use repodna_core::CancellationToken;
use repodna_core::io::read_artifact;
use repodna_core::model::artifact::RepositoryDna;
use repodna_core::time::Timestamp;
use repodna_engine::{AnalysisCache, AnalysisRequest, FetchOptions, InputSpec, Progress, analyze};
use repodna_git::url::{looks_like_url, sanitize_url};
use repodna_plugin::load_enabled;
use repodna_store::{FindingChanges, RepositoryLocation, ScanRecord, Store};

use crate::config::{ConfigOptions, load_config};
use crate::error::AppError;
use crate::paths::AppPaths;

/// How to run an analysis.
#[derive(Debug, Clone, Default)]
pub struct AnalyzeOptions {
    /// A directory, archive, or Git URL.
    pub input: String,
    /// Configuration layers and overrides.
    pub config: ConfigOptions,
    /// How URLs are fetched.
    pub fetch: FetchOptions,
    /// Do not record the analysis in local storage (and do not use the cache).
    pub no_store: bool,
    /// Record zero durations so repeated runs produce identical artifacts.
    pub reproducible: bool,
    /// Time the analysis is measured against (defaults to now or `SOURCE_DATE_EPOCH`).
    pub reference_time: Option<Timestamp>,
}

/// A finished analysis.
#[derive(Debug)]
pub struct AnalysisOutcome {
    /// The artifact.
    pub dna: RepositoryDna,
    /// The stored scan, when the analysis was stored.
    pub scan: Option<ScanRecord>,
    /// The previous stored scan of the same repository.
    pub previous: Option<ScanRecord>,
    /// Findings added and resolved since the previous scan.
    pub changes: Option<FindingChanges>,
    /// Plugins that ran.
    pub plugins: Vec<String>,
    /// Problems that did not stop the analysis (storage, plugins).
    pub warnings: Vec<String>,
}

fn canonical(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

/// The identity under which an analysis of `spec` is stored.
pub fn repository_location(spec: &InputSpec, dna: &RepositoryDna) -> RepositoryLocation {
    let location = match spec {
        InputSpec::Directory(path) | InputSpec::Archive(path) => canonical(path),
        InputSpec::Url(url) => sanitize_url(url),
    };
    RepositoryLocation {
        kind: dna.analysis_metadata.input.kind,
        location,
    }
}

/// Opens the store, turning failure into a warning so analysis can continue without it.
fn open_store(paths: &AppPaths, warnings: &mut Vec<String>) -> Option<Arc<Store>> {
    match paths.open_store() {
        Ok(store) => Some(Arc::new(store)),
        Err(error) => {
            warnings.push(format!(
                "Local storage is unavailable, so this analysis is not stored and the cache is off: {}. Run `repodna cache repair`.",
                error.message
            ));
            None
        }
    }
}

/// Records `dna` and compares it with the previous scan of the same repository.
fn record(
    store: &Store,
    location: &RepositoryLocation,
    dna: &RepositoryDna,
) -> Result<(ScanRecord, Option<ScanRecord>, Option<FindingChanges>), AppError> {
    let previous = store.scans(&location.id(), 1)?.into_iter().next();
    let scan = store.record(dna, location)?;
    let previous = previous.filter(|p| p.id != scan.id);
    let changes = previous
        .as_ref()
        .map(|p| store.finding_changes(&p.id, &scan.id))
        .transpose()?;
    Ok((scan, previous, changes))
}

/// Analyzes `options.input`.
pub fn run_analysis(
    paths: &AppPaths,
    options: &AnalyzeOptions,
    progress: Progress,
    cancel: &CancellationToken,
) -> Result<AnalysisOutcome, AppError> {
    let spec = InputSpec::detect(&options.input);
    let project_root = match &spec {
        InputSpec::Directory(path) => Some(path.as_path()),
        _ => None,
    };
    let loaded = load_config(paths, project_root, &options.config)?;
    let mut warnings = Vec::new();
    let store = if options.no_store {
        None
    } else {
        open_store(paths, &mut warnings)
    };
    let plugin_dirs = paths.plugin_dirs(
        &loaded.config.plugins.directories,
        &options.config.overrides.plugin_dirs,
    );
    let plugins = load_enabled(&loaded.config.plugins, &plugin_dirs, options.reproducible);
    let mut config_warnings = loaded.warnings;
    config_warnings.extend(plugins.warnings.iter().cloned());
    let request = AnalysisRequest {
        input: spec.clone(),
        config: loaded.config,
        config_sources: loaded.sources,
        config_warnings,
        fetch: options.fetch,
        languages: plugins.languages,
        extensions: plugins.extensions,
        cache: store.clone().map(|store| store as Arc<dyn AnalysisCache>),
        progress,
        reference_time: options.reference_time,
        reproducible: options.reproducible,
    };
    let dna = analyze(&request, cancel)?;

    let mut outcome = AnalysisOutcome {
        scan: None,
        previous: None,
        changes: None,
        plugins: plugins.active,
        warnings,
        dna,
    };
    if let Some(store) = store {
        let location = repository_location(&spec, &outcome.dna);
        match record(&store, &location, &outcome.dna) {
            Ok((scan, previous, changes)) => {
                outcome.scan = Some(scan);
                outcome.previous = previous;
                outcome.changes = changes;
            }
            Err(error) => outcome.warnings.push(format!(
                "The analysis finished but could not be stored: {}. Run `repodna cache repair`.",
                error.message
            )),
        }
    }
    Ok(outcome)
}

/// What a command should read an analysis from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Analyze a directory, archive, or URL now.
    Analyze(String),
    /// Read an artifact file (`.json` or `.repodna`).
    Artifact(PathBuf),
    /// Read the latest stored analysis of a repository (by identifier, path, or name).
    Stored(String),
}

/// Returns `true` when `path` holds an artifact rather than an archive.
fn is_artifact_file(path: &Path) -> bool {
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase());
    matches!(extension.as_deref(), Some("json" | "repodna"))
}

/// Interprets a command-line target.
pub fn resolve_target(target: &str) -> Target {
    if looks_like_url(target) {
        return Target::Analyze(target.to_owned());
    }
    let path = Path::new(target);
    if path.is_dir() {
        Target::Analyze(target.to_owned())
    } else if path.is_file() {
        if is_artifact_file(path) {
            Target::Artifact(path.to_path_buf())
        } else {
            Target::Analyze(target.to_owned())
        }
    } else {
        Target::Stored(target.to_owned())
    }
}

/// Where a loaded analysis came from.
#[derive(Debug)]
pub enum Source {
    /// Analyzed now.
    Fresh(Box<AnalysisOutcome>),
    /// Read from an artifact file.
    Artifact {
        /// The file.
        path: PathBuf,
        /// Compatibility notes.
        warnings: Vec<String>,
    },
    /// Read from local storage.
    Stored(Box<ScanRecord>),
}

/// An analysis ready to display.
#[derive(Debug)]
pub struct Loaded {
    /// The artifact.
    pub dna: RepositoryDna,
    /// Where it came from.
    pub source: Source,
}

/// Loads the latest stored analysis matching `query`.
pub fn load_stored(paths: &AppPaths, query: &str) -> Result<(RepositoryDna, ScanRecord), AppError> {
    let store = paths.open_store()?;
    let query_path = Path::new(query);
    let canonical_query = if query_path.exists() {
        canonical(query_path)
    } else {
        query.to_owned()
    };
    let repository = store.find_repository(&canonical_query)?.ok_or_else(|| {
        AppError::input(format!(
            "`{query}` is not a directory, archive, artifact file, URL, or stored repository"
        ))
        .with_hint("Run `repodna list` to see stored repositories, or pass a path to analyze.")
    })?;
    let scan = store
        .scans(&repository.id, 1)?
        .into_iter()
        .next()
        .ok_or_else(|| AppError::input(format!("`{query}` has no stored analyses")))?;
    let dna = store.load(&scan)?;
    Ok((dna, scan))
}

/// Loads `target`, analyzing it when it is a directory, archive, or URL.
pub fn load_target(
    paths: &AppPaths,
    target: &str,
    template: &AnalyzeOptions,
    progress: Progress,
    cancel: &CancellationToken,
) -> Result<Loaded, AppError> {
    match resolve_target(target) {
        Target::Analyze(input) => {
            let options = AnalyzeOptions {
                input,
                ..template.clone()
            };
            let outcome = run_analysis(paths, &options, progress, cancel)?;
            Ok(Loaded {
                dna: outcome.dna.clone(),
                source: Source::Fresh(Box::new(outcome)),
            })
        }
        Target::Artifact(path) => {
            let loaded = read_artifact(&path)?;
            Ok(Loaded {
                dna: loaded.artifact,
                source: Source::Artifact {
                    path,
                    warnings: loaded.warnings,
                },
            })
        }
        Target::Stored(query) => {
            let (dna, scan) = load_stored(paths, &query)?;
            Ok(Loaded {
                dna,
                source: Source::Stored(Box::new(scan)),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_testkit::write_tree;

    fn options(input: &Path) -> AnalyzeOptions {
        AnalyzeOptions {
            input: input.to_string_lossy().into_owned(),
            config: ConfigOptions {
                ignore_user_config: true,
                ..ConfigOptions::default()
            },
            reproducible: true,
            reference_time: Some(Timestamp::from_unix(1_790_000_000)),
            ..AnalyzeOptions::default()
        }
    }

    #[test]
    fn analyzes_stores_and_reloads() {
        let home = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        write_tree(
            repo.path(),
            &[
                ("README.md", "# Widget\n\nA small widget library.\n"),
                ("src/lib.rs", "pub fn widget() -> u32 {\n    42\n}\n"),
            ],
        );
        let paths = AppPaths::in_directory(home.path());
        let cancel = CancellationToken::new();
        let first =
            run_analysis(&paths, &options(repo.path()), Progress::default(), &cancel).unwrap();
        assert!(first.warnings.is_empty(), "{:?}", first.warnings);
        let scan = first.scan.as_ref().unwrap();
        assert!(first.previous.is_none());
        assert_eq!(scan.files, 2);

        write_tree(repo.path(), &[("src/extra.rs", "pub fn extra() {}\n")]);
        let second =
            run_analysis(&paths, &options(repo.path()), Progress::default(), &cancel).unwrap();
        assert_eq!(second.previous.as_ref().unwrap().id, scan.id);
        assert!(second.changes.is_some());

        let name = second.dna.identity.name.clone();
        let loaded = load_target(
            &paths,
            &name,
            &AnalyzeOptions::default(),
            Progress::default(),
            &cancel,
        )
        .unwrap();
        assert!(matches!(loaded.source, Source::Stored(_)));
        assert_eq!(loaded.dna.structure.total_files, 3);

        let artifact = home.path().join("scan.repodna");
        std::fs::write(&artifact, serde_json::to_string(&second.dna).unwrap()).unwrap();
        assert_eq!(
            resolve_target(&artifact.to_string_lossy()),
            Target::Artifact(artifact.clone())
        );
        let from_file = load_target(
            &paths,
            &artifact.to_string_lossy(),
            &AnalyzeOptions::default(),
            Progress::default(),
            &cancel,
        )
        .unwrap();
        assert_eq!(from_file.dna, second.dna);

        let missing = load_target(
            &paths,
            "no-such-repository",
            &AnalyzeOptions::default(),
            Progress::default(),
            &cancel,
        )
        .unwrap_err();
        assert_eq!(missing.exit_code(), 3);
    }

    #[test]
    fn analyzes_without_storage() {
        let home = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        write_tree(repo.path(), &[("main.py", "print('hi')\n")]);
        let paths = AppPaths::in_directory(home.path());
        let mut options = options(repo.path());
        options.no_store = true;
        let outcome = run_analysis(
            &paths,
            &options,
            Progress::default(),
            &CancellationToken::new(),
        )
        .unwrap();
        assert!(outcome.scan.is_none());
        assert!(!home.path().join("repodna.db").exists());
        assert_eq!(
            resolve_target("https://github.com/sanskarIN/RepoDNA"),
            Target::Analyze("https://github.com/sanskarIN/RepoDNA".into())
        );
    }
}
