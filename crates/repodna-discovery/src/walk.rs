//! Gitignore-aware, parallel repository walking.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use ignore::overrides::OverrideBuilder;
use ignore::{WalkBuilder, WalkState};
use repodna_core::CancellationToken;
use repodna_core::model::languages::LanguageKind;
use repodna_core::paths;
use repodna_parser::LanguageRegistry;

use crate::classify::{Classification, ClassificationOverrides, classify};

/// Version-control metadata directories that are never walked.
const VCS_DIRECTORIES: &[&str] = &[".git", ".hg", ".svn", ".jj", ".bzr", "_darcs", ".fossil"];

/// Options for [`discover`].
#[derive(Debug, Clone)]
pub struct DiscoveryOptions {
    /// Respect `.gitignore`, `.ignore`, and `.git/info/exclude`.
    pub respect_gitignore: bool,
    /// Additional gitignore-style patterns to exclude.
    pub ignore_patterns: Vec<String>,
    /// Classification overrides.
    pub overrides: ClassificationOverrides,
    /// Stop after this many files and mark the result truncated.
    pub max_files: usize,
    /// Worker threads.
    pub threads: usize,
}

impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self {
            respect_gitignore: true,
            ignore_patterns: Vec::new(),
            overrides: ClassificationOverrides::default(),
            max_files: 1_000_000,
            threads: std::thread::available_parallelism().map_or(1, usize::from),
        }
    }
}

/// A file found during discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredFile {
    /// Repository-relative path.
    pub path: String,
    /// Absolute path on disk.
    pub absolute: PathBuf,
    /// Size in bytes.
    pub bytes: u64,
    /// Path-based classification.
    pub classification: Classification,
    /// Language detected from the path, if any.
    pub language: Option<String>,
    /// Kind of the detected language.
    pub language_kind: Option<LanguageKind>,
    /// Unix permission bits, when available.
    pub mode: Option<u32>,
}

/// The result of walking a repository.
#[derive(Debug, Clone, Default)]
pub struct Discovery {
    /// Files sorted by path.
    pub files: Vec<DiscoveredFile>,
    /// Symbolic links encountered (never followed).
    pub symlinks: u64,
    /// `true` when `max_files` was reached.
    pub truncated: bool,
    /// Non-fatal problems, e.g. unreadable directories.
    pub errors: Vec<String>,
    /// Ignore patterns applied in addition to gitignore rules.
    pub ignore_patterns: Vec<String>,
}

/// Errors that stop discovery.
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    /// The root does not exist.
    #[error("{0} does not exist")]
    NotFound(PathBuf),
    /// The root is not a directory.
    #[error("{0} is not a directory")]
    NotADirectory(PathBuf),
    /// An ignore pattern is invalid.
    #[error("invalid ignore pattern {pattern:?}: {message}")]
    InvalidPattern {
        /// The pattern.
        pattern: String,
        /// Why it is invalid.
        message: String,
    },
    /// The walk was cancelled.
    #[error("discovery was cancelled")]
    Cancelled,
}

#[cfg(unix)]
fn permission_mode(metadata: &std::fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(metadata.permissions().mode() & 0o7777)
}

#[cfg(not(unix))]
fn permission_mode(_metadata: &std::fs::Metadata) -> Option<u32> {
    None
}

/// Walks `root` and classifies every file.
///
/// Symbolic links are counted but never followed, version-control directories are
/// skipped, and results are sorted by path so output is deterministic regardless of
/// thread scheduling.
pub fn discover(
    root: &Path,
    options: &DiscoveryOptions,
    registry: &LanguageRegistry,
    cancel: &CancellationToken,
) -> Result<Discovery, DiscoveryError> {
    if !root.exists() {
        return Err(DiscoveryError::NotFound(root.to_path_buf()));
    }
    if !root.is_dir() {
        return Err(DiscoveryError::NotADirectory(root.to_path_buf()));
    }

    let mut overrides = OverrideBuilder::new(root);
    for pattern in &options.ignore_patterns {
        overrides
            .add(&format!("!{pattern}"))
            .map_err(|error| DiscoveryError::InvalidPattern {
                pattern: pattern.clone(),
                message: error.to_string(),
            })?;
    }
    let overrides = overrides
        .build()
        .map_err(|error| DiscoveryError::InvalidPattern {
            pattern: options.ignore_patterns.join(", "),
            message: error.to_string(),
        })?;

    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .parents(false)
        .git_global(false)
        .git_ignore(options.respect_gitignore)
        .git_exclude(options.respect_gitignore)
        .ignore(options.respect_gitignore)
        .require_git(false)
        .follow_links(false)
        .overrides(overrides)
        .threads(options.threads.max(1))
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !(entry.file_type().is_some_and(|t| t.is_dir())
                && VCS_DIRECTORIES.contains(&name.as_ref()))
        });

    let files = Mutex::new(Vec::new());
    let errors = Mutex::new(Vec::new());
    let symlinks = AtomicU64::new(0);
    let count = AtomicUsize::new(0);
    let truncated = std::sync::atomic::AtomicBool::new(false);
    let root = root.to_path_buf();

    builder.build_parallel().run(|| {
        Box::new(|result| {
            if cancel.is_cancelled() {
                return WalkState::Quit;
            }
            let entry = match result {
                Ok(entry) => entry,
                Err(error) => {
                    if let Ok(mut errors) = errors.lock() {
                        errors.push(error.to_string());
                    }
                    return WalkState::Continue;
                }
            };
            let Some(file_type) = entry.file_type() else {
                return WalkState::Continue;
            };
            if file_type.is_symlink() {
                symlinks.fetch_add(1, Ordering::Relaxed);
                return WalkState::Continue;
            }
            if !file_type.is_file() {
                return WalkState::Continue;
            }
            if count.fetch_add(1, Ordering::Relaxed) >= options.max_files {
                truncated.store(true, Ordering::Relaxed);
                return WalkState::Quit;
            }
            let Some(path) = paths::relative_to(&root, entry.path()) else {
                return WalkState::Continue;
            };
            let metadata = entry.metadata().ok();
            let language = registry.detect_path(&path);
            let language_kind = language.map(|spec| spec.kind);
            let classification = classify(&path, language_kind, &options.overrides);
            let file = DiscoveredFile {
                absolute: entry.path().to_path_buf(),
                bytes: metadata.as_ref().map_or(0, std::fs::Metadata::len),
                classification,
                language: language.map(|spec| spec.id.clone()),
                language_kind,
                mode: metadata.as_ref().and_then(permission_mode),
                path,
            };
            if let Ok(mut files) = files.lock() {
                files.push(file);
            }
            WalkState::Continue
        })
    });

    if cancel.is_cancelled() {
        return Err(DiscoveryError::Cancelled);
    }
    let mut files = files.into_inner().unwrap_or_default();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files.truncate(options.max_files);
    let mut errors = errors.into_inner().unwrap_or_default();
    errors.sort();
    Ok(Discovery {
        files,
        symlinks: symlinks.into_inner(),
        truncated: truncated.into_inner(),
        errors,
        ignore_patterns: options.ignore_patterns.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::structure::FileCategory;
    use std::fs;

    fn write(root: &Path, path: &str, contents: &str) {
        let full = root.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, contents).unwrap();
    }

    fn walk(root: &Path, options: &DiscoveryOptions) -> Discovery {
        discover(
            root,
            options,
            LanguageRegistry::builtin(),
            &CancellationToken::new(),
        )
        .unwrap()
    }

    #[test]
    fn walks_sorted_and_respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "src/main.rs", "fn main() {}\n");
        write(root, "src/lib.rs", "");
        write(root, ".gitignore", "build/\n*.log\n");
        write(root, "build/out.txt", "x");
        write(root, "debug.log", "x");
        write(root, ".github/workflows/ci.yml", "on: push\n");
        write(root, ".git/config", "[core]\n");
        write(root, "node_modules/x/index.js", "x");
        let options = DiscoveryOptions {
            ignore_patterns: vec!["node_modules/".into()],
            ..DiscoveryOptions::default()
        };
        let discovery = walk(root, &options);
        let paths: Vec<_> = discovery.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                ".github/workflows/ci.yml",
                ".gitignore",
                "src/lib.rs",
                "src/main.rs"
            ]
        );
        let main = discovery
            .files
            .iter()
            .find(|f| f.path == "src/main.rs")
            .unwrap();
        assert_eq!(main.language.as_deref(), Some("rust"));
        assert_eq!(main.classification.category, FileCategory::Source);
        assert_eq!(main.bytes, 13);
        assert!(!discovery.truncated);
    }

    #[test]
    fn gitignore_can_be_disabled() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), ".gitignore", "*.log\n");
        write(dir.path(), "debug.log", "x");
        let options = DiscoveryOptions {
            respect_gitignore: false,
            ..DiscoveryOptions::default()
        };
        assert_eq!(walk(dir.path(), &options).files.len(), 2);
    }

    #[test]
    fn truncates_at_max_files() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..20 {
            write(dir.path(), &format!("f{index:02}.txt"), "x");
        }
        let options = DiscoveryOptions {
            max_files: 5,
            threads: 1,
            ..DiscoveryOptions::default()
        };
        let discovery = walk(dir.path(), &options);
        assert!(discovery.truncated);
        assert_eq!(discovery.files.len(), 5);
    }

    #[cfg(unix)]
    #[test]
    fn counts_but_does_not_follow_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "secret.txt", "outside");
        write(dir.path(), "real.txt", "inside");
        std::os::unix::fs::symlink(outside.path(), dir.path().join("escape")).unwrap();
        std::os::unix::fs::symlink(dir.path().join("real.txt"), dir.path().join("alias.txt"))
            .unwrap();
        let discovery = walk(dir.path(), &DiscoveryOptions::default());
        let paths: Vec<_> = discovery.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["real.txt"]);
        assert_eq!(discovery.symlinks, 2);
    }

    #[test]
    fn reports_missing_roots_and_cancellation() {
        let missing = discover(
            Path::new("/definitely/missing/root"),
            &DiscoveryOptions::default(),
            LanguageRegistry::builtin(),
            &CancellationToken::new(),
        );
        assert!(matches!(missing, Err(DiscoveryError::NotFound(_))));
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.txt", "x");
        let token = CancellationToken::new();
        token.cancel();
        let cancelled = discover(
            dir.path(),
            &DiscoveryOptions::default(),
            LanguageRegistry::builtin(),
            &token,
        );
        assert!(matches!(cancelled, Err(DiscoveryError::Cancelled)));
    }
}
