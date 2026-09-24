//! Input resolution: what to analyze and where it lives on disk.

use std::path::{Path, PathBuf};

use repodna_core::cancel::CancellationToken;
use repodna_core::model::metadata::{InputInfo, InputKind};
use repodna_discovery::{ArchiveLimits, ExtractedArchive, detect_archive, extract};
use repodna_git::url::{UrlPolicy, looks_like_url, parse_remote, sanitize_url};
use repodna_git::{CloneOptions, GitRunner, clone_repository, repository_root};
use tempfile::TempDir;

use crate::EngineError;

/// What the user asked to analyze.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputSpec {
    /// A local directory: a Git working tree or plain files.
    Directory(PathBuf),
    /// A remote Git repository, cloned into a temporary directory.
    Url(String),
    /// A ZIP or TAR archive, extracted into a temporary directory.
    Archive(PathBuf),
}

impl InputSpec {
    /// Interprets a command-line argument: a URL, an archive, or a directory.
    pub fn detect(input: &str) -> Self {
        if looks_like_url(input) {
            return InputSpec::Url(input.to_owned());
        }
        let path = PathBuf::from(input);
        if path.is_file() && detect_archive(&path).is_some() {
            InputSpec::Archive(path)
        } else {
            InputSpec::Directory(path)
        }
    }
}

/// How remote inputs are fetched.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FetchOptions {
    /// Which otherwise-rejected URLs are acceptable.
    pub url_policy: UrlPolicy,
    /// Clone only this many commits.
    pub clone_depth: Option<u32>,
}

/// An input ready for analysis. Temporary directories are removed when it is dropped.
#[derive(Debug)]
pub struct PreparedInput {
    /// Directory to analyze.
    pub root: PathBuf,
    /// The Git working tree root, when `root` is the top of a Git repository.
    pub git_root: Option<PathBuf>,
    /// Where the input came from, for the artifact (never an absolute local path).
    pub info: InputInfo,
    /// Repository name guessed from the input.
    pub name: String,
    /// `true` when the network was used (cloning).
    pub network_used: bool,
    /// Notes about how the input was prepared.
    pub notes: Vec<String>,
    _clone: Option<TempDir>,
    _archive: Option<ExtractedArchive>,
}

fn directory_name(path: &Path) -> String {
    path.canonicalize()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .or_else(|| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "repository".to_owned())
}

/// Finds the Git root for `dir` when it is the top of a working tree.
fn git_root_of(
    dir: &Path,
    git: Option<&GitRunner>,
    cancel: &CancellationToken,
) -> Result<Option<PathBuf>, EngineError> {
    let Some(git) = git else {
        return Ok(None);
    };
    let Some(root) = repository_root(git, dir, cancel)? else {
        return Ok(None);
    };
    let same = match (root.canonicalize(), dir.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    Ok(same.then_some(root))
}

/// Resolves `spec` into a directory on disk, cloning or extracting as needed.
pub fn prepare_input(
    spec: &InputSpec,
    git: Option<&GitRunner>,
    options: FetchOptions,
    cancel: &CancellationToken,
) -> Result<PreparedInput, EngineError> {
    match spec {
        InputSpec::Directory(path) => {
            if !path.exists() {
                return Err(EngineError::NotFound(path.clone()));
            }
            if !path.is_dir() {
                return Err(EngineError::UnsupportedInput(path.display().to_string()));
            }
            let name = directory_name(path);
            let git_root = git_root_of(path, git, cancel)?;
            let kind = if git_root.is_some() {
                InputKind::GitRepository
            } else {
                InputKind::LocalDirectory
            };
            let mut notes = Vec::new();
            if git_root.is_none()
                && let Some(git) = git
                && repository_root(git, path, cancel)?.is_some()
            {
                notes.push(
                    "The directory is inside a Git repository but is not its root, so history was not analyzed; analyze the repository root to include history."
                        .to_owned(),
                );
            }
            Ok(PreparedInput {
                root: path.clone(),
                git_root,
                info: InputInfo {
                    kind,
                    display: name.clone(),
                },
                name,
                network_used: false,
                notes,
                _clone: None,
                _archive: None,
            })
        }
        InputSpec::Url(url) => {
            let git = git.ok_or(EngineError::Git(repodna_git::GitError::NotInstalled))?;
            let directory = tempfile::Builder::new()
                .prefix("repodna-clone-")
                .tempdir()
                .map_err(|error| EngineError::io("creating a temporary directory", error))?;
            let destination = directory.path().join("repository");
            clone_repository(
                git,
                url,
                &destination,
                CloneOptions {
                    depth: options.clone_depth,
                    policy: options.url_policy,
                },
                cancel,
            )?;
            let remote = parse_remote(url);
            let name = remote
                .name
                .clone()
                .unwrap_or_else(|| "repository".to_owned());
            let mut notes = Vec::new();
            if let Some(depth) = options.clone_depth {
                notes.push(format!(
                    "The repository was cloned with --depth {depth}; history analysis covers only those commits."
                ));
            }
            Ok(PreparedInput {
                root: destination.clone(),
                git_root: Some(destination),
                info: InputInfo {
                    kind: InputKind::GitUrl,
                    display: sanitize_url(url),
                },
                name,
                network_used: true,
                notes,
                _clone: Some(directory),
                _archive: None,
            })
        }
        InputSpec::Archive(path) => {
            if !path.exists() {
                return Err(EngineError::NotFound(path.clone()));
            }
            let archive = extract(path, ArchiveLimits::default())?;
            let file_name = path.file_name().map_or_else(
                || "archive".to_owned(),
                |n| n.to_string_lossy().into_owned(),
            );
            let name = file_name
                .trim_end_matches(".tar.gz")
                .trim_end_matches(".tgz")
                .trim_end_matches(".tar")
                .trim_end_matches(".zip")
                .to_owned();
            let mut notes = Vec::new();
            if !archive.skipped.is_empty() {
                notes.push(format!(
                    "{} archive entries were skipped (links, devices, or version-control metadata).",
                    archive.skipped.len()
                ));
            }
            Ok(PreparedInput {
                root: archive.root.clone(),
                git_root: None,
                info: InputInfo {
                    kind: InputKind::Archive,
                    display: file_name,
                },
                name,
                network_used: false,
                notes,
                _clone: None,
                _archive: Some(archive),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_testkit::{GitRepo, git_available};

    #[test]
    fn detects_input_kinds() {
        assert_eq!(
            InputSpec::detect("https://github.com/o/r"),
            InputSpec::Url("https://github.com/o/r".into())
        );
        assert!(matches!(InputSpec::detect("."), InputSpec::Directory(_)));
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("x.zip");
        std::fs::write(&archive, b"PK\x03\x04").unwrap();
        assert!(matches!(
            InputSpec::detect(archive.to_str().unwrap()),
            InputSpec::Archive(_)
        ));
    }

    #[test]
    fn prepares_plain_directories_without_absolute_paths() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("my-project");
        std::fs::create_dir(&project).unwrap();
        let prepared = prepare_input(
            &InputSpec::Directory(project.clone()),
            None,
            FetchOptions::default(),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(prepared.name, "my-project");
        assert_eq!(prepared.info.display, "my-project");
        assert_eq!(prepared.info.kind, InputKind::LocalDirectory);
        assert!(prepared.git_root.is_none());
        let missing = prepare_input(
            &InputSpec::Directory(dir.path().join("nope")),
            None,
            FetchOptions::default(),
            &CancellationToken::new(),
        );
        assert!(matches!(missing, Err(EngineError::NotFound(_))));
    }

    #[test]
    fn recognizes_git_roots_and_subdirectories() {
        if !git_available() {
            return;
        }
        let repo = GitRepo::new();
        repo.write("src/lib.rs", "pub fn a() {}\n");
        repo.commit("initial", "Ana", "ana@example.test", "2024-01-01T00:00:00Z");
        let git = GitRunner::detect().unwrap();
        let cancel = CancellationToken::new();
        let prepared = prepare_input(
            &InputSpec::Directory(repo.path().to_path_buf()),
            Some(&git),
            FetchOptions::default(),
            &cancel,
        )
        .unwrap();
        assert!(prepared.git_root.is_some());
        assert_eq!(prepared.info.kind, InputKind::GitRepository);
        let sub = prepare_input(
            &InputSpec::Directory(repo.path().join("src")),
            Some(&git),
            FetchOptions::default(),
            &cancel,
        )
        .unwrap();
        assert!(sub.git_root.is_none());
        assert_eq!(sub.notes.len(), 1);
    }
}
