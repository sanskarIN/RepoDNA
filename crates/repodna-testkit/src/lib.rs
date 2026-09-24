//! # repodna-testkit
//!
//! Helpers for tests: build Git repositories with fixed authors and dates so that commit
//! hashes and analysis results are deterministic, and write file trees in one call.
//!
//! Every Git command runs with an empty global configuration, no system configuration,
//! and signing disabled, so tests behave the same on every machine.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tempfile::TempDir;

/// Returns `true` when a `git` executable is available.
pub fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Writes `contents` to `root/path`, creating parent directories.
///
/// # Panics
///
/// Panics if the file cannot be written; this is a test helper.
pub fn write_file(root: &Path, path: &str, contents: impl AsRef<[u8]>) {
    let full = root.join(path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("create {parent:?}: {error}"));
    }
    std::fs::write(&full, contents).unwrap_or_else(|error| panic!("write {full:?}: {error}"));
}

/// Writes several files at once.
pub fn write_tree(root: &Path, files: &[(&str, &str)]) {
    for (path, contents) in files {
        write_file(root, path, contents);
    }
}

/// A temporary Git repository with deterministic commits.
pub struct GitRepo {
    directory: TempDir,
    config: PathBuf,
}

impl GitRepo {
    /// Initializes an empty repository on branch `main`.
    ///
    /// # Panics
    ///
    /// Panics if Git is missing or initialization fails; this is a test helper.
    pub fn new() -> Self {
        let directory = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
        let config = directory.path().join(".testkit-gitconfig");
        std::fs::write(&config, "").unwrap_or_else(|error| panic!("gitconfig: {error}"));
        let repo = Self { directory, config };
        repo.git(&["init", "-q", "-b", "main"]);
        // Keep the helper file out of the repository's history and status.
        std::fs::write(
            repo.path().join(".git/info/exclude"),
            ".testkit-gitconfig\n",
        )
        .unwrap_or_else(|error| panic!("exclude: {error}"));
        repo
    }

    /// The working-tree path.
    pub fn path(&self) -> &Path {
        self.directory.path()
    }

    /// Runs a Git command in the repository and returns its standard output.
    ///
    /// # Panics
    ///
    /// Panics if the command fails; this is a test helper.
    pub fn git(&self, args: &[&str]) -> String {
        self.git_with_env(args, &[])
    }

    fn git_with_env(&self, args: &[&str], env: &[(&str, &str)]) -> String {
        let output = Command::new("git")
            .current_dir(self.path())
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "tag.gpgsign=false",
                "-c",
                "core.autocrlf=false",
            ])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", &self.config)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .envs(env.iter().copied())
            .output()
            .unwrap_or_else(|error| panic!("run git {args:?}: {error}"));
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// Writes a file in the working tree.
    pub fn write(&self, path: &str, contents: impl AsRef<[u8]>) -> &Self {
        write_file(self.path(), path, contents);
        self
    }

    /// Removes a file from the working tree.
    ///
    /// # Panics
    ///
    /// Panics if the file cannot be removed; this is a test helper.
    pub fn remove(&self, path: &str) -> &Self {
        std::fs::remove_file(self.path().join(path))
            .unwrap_or_else(|error| panic!("remove {path}: {error}"));
        self
    }

    /// Renames a file with `git mv`.
    pub fn rename(&self, from: &str, to: &str) -> &Self {
        if let Some(parent) = self.path().join(to).parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
        }
        self.git(&["mv", from, to]);
        self
    }

    /// Stages everything and commits with a fixed author, e-mail, and RFC 3339 date.
    /// Returns the new commit hash.
    pub fn commit(&self, message: &str, author: &str, email: &str, date: &str) -> String {
        self.git(&["add", "-A"]);
        let env = [
            ("GIT_AUTHOR_NAME", author),
            ("GIT_AUTHOR_EMAIL", email),
            ("GIT_AUTHOR_DATE", date),
            ("GIT_COMMITTER_NAME", author),
            ("GIT_COMMITTER_EMAIL", email),
            ("GIT_COMMITTER_DATE", date),
        ];
        self.git_with_env(&["commit", "-q", "--allow-empty", "-m", message], &env);
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    /// Creates a lightweight tag, or an annotated one with a fixed date.
    pub fn tag(&self, name: &str, annotated_date: Option<&str>) -> &Self {
        match annotated_date {
            Some(date) => {
                let env = [
                    ("GIT_COMMITTER_NAME", "Tagger"),
                    ("GIT_COMMITTER_EMAIL", "tagger@example.invalid"),
                    ("GIT_COMMITTER_DATE", date),
                ];
                self.git_with_env(&["tag", "-a", name, "-m", name], &env);
            }
            None => {
                self.git(&["tag", name]);
            }
        }
        self
    }
}

impl Default for GitRepo {
    fn default() -> Self {
        Self::new()
    }
}
