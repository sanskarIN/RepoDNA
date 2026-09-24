//! A hardened runner for Git commands.
//!
//! RepoDNA only *reads* repositories, and repositories can be hostile: a `.git/config`
//! shipped inside an archive or a shared directory could configure commands that Git
//! would execute. Every invocation therefore:
//!
//! * disables `core.fsmonitor` (which can run an arbitrary command during `status`),
//! * never runs text-conversion filters or external diff drivers (`--no-textconv`,
//!   `--no-ext-diff` on history commands),
//! * disables the `ext::` and `file://` transports,
//! * never prompts for credentials (`GIT_TERMINAL_PROMPT=0`) or takes optional locks
//!   (`GIT_OPTIONAL_LOCKS=0`, so read-only commands never write the index),
//! * ignores environment variables that redirect Git to another repository,
//! * enforces a timeout and honors cancellation, killing the process if needed.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use repodna_core::CancellationToken;

use crate::error::GitError;

/// Configuration overrides applied to every Git invocation.
const HARDENING: &[&str] = &[
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.untrackedCache=false",
    "-c",
    "core.quotePath=false",
    "-c",
    "i18n.logOutputEncoding=UTF-8",
    "-c",
    "log.showSignature=false",
    "-c",
    "color.ui=never",
    "-c",
    "core.pager=cat",
    "-c",
    "protocol.ext.allow=never",
    "-c",
    "protocol.file.allow=never",
];

/// Environment variables that could point Git at a different repository or object store.
const REDIRECTING_VARIABLES: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_EXTERNAL_DIFF",
    "GIT_SSH_COMMAND_ON_CLONE",
];

/// Default timeout for one Git command.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Maximum captured standard-error bytes.
const MAX_STDERR: usize = 16 * 1024;

/// Runs Git commands safely.
#[derive(Debug, Clone)]
pub struct GitRunner {
    program: PathBuf,
    version: String,
    timeout: Duration,
}

impl GitRunner {
    /// Finds Git on `PATH` (or at `$REPODNA_GIT`) and records its version.
    pub fn detect() -> Result<Self, GitError> {
        let program =
            std::env::var_os("REPODNA_GIT").map_or_else(|| PathBuf::from("git"), PathBuf::from);
        let output = Command::new(&program)
            .arg("--version")
            .stdin(Stdio::null())
            .output()
            .map_err(|_| GitError::NotInstalled)?;
        if !output.status.success() {
            return Err(GitError::NotInstalled);
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let version = text
            .trim()
            .strip_prefix("git version ")
            .unwrap_or(text.trim())
            .to_owned();
        Ok(Self {
            program,
            version,
            timeout: DEFAULT_TIMEOUT,
        })
    }

    /// The Git version string, e.g. `2.43.0`.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Returns a runner with a different per-command timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn command(&self, repo: Option<&Path>, args: &[&str]) -> Command {
        let mut command = Command::new(&self.program);
        if let Some(repo) = repo {
            command.arg("-C").arg(repo);
        }
        command.args(HARDENING).args(args);
        command
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_PAGER", "cat")
            .env("PAGER", "cat")
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for variable in REDIRECTING_VARIABLES {
            command.env_remove(variable);
        }
        command
    }

    /// Runs a command and returns its standard output.
    pub fn output(
        &self,
        repo: &Path,
        args: &[&str],
        cancel: &CancellationToken,
    ) -> Result<Vec<u8>, GitError> {
        let mut output = Vec::new();
        self.stream(Some(repo), args, cancel, |reader| {
            reader.read_to_end(&mut output).map_err(GitError::Io)?;
            Ok(())
        })?;
        Ok(output)
    }

    /// Runs a command and returns its trimmed standard output as text.
    pub fn text(
        &self,
        repo: &Path,
        args: &[&str],
        cancel: &CancellationToken,
    ) -> Result<String, GitError> {
        let bytes = self.output(repo, args, cancel)?;
        Ok(String::from_utf8_lossy(&bytes).trim().to_owned())
    }

    /// Runs a command outside any repository (e.g. `clone`).
    pub fn run_global(&self, args: &[&str], cancel: &CancellationToken) -> Result<(), GitError> {
        self.stream(None, args, cancel, |reader| {
            std::io::copy(reader, &mut std::io::sink()).map_err(GitError::Io)?;
            Ok(())
        })
    }

    /// Runs a command and passes its standard output to `consume` as it is produced.
    ///
    /// A watchdog thread kills the process when the timeout elapses or `cancel` is
    /// triggered, which ends the stream; the error then reports why.
    pub fn stream<F>(
        &self,
        repo: Option<&Path>,
        args: &[&str],
        cancel: &CancellationToken,
        consume: F,
    ) -> Result<(), GitError>
    where
        F: FnOnce(&mut dyn BufRead) -> Result<(), GitError>,
    {
        cancel.check().map_err(|_| GitError::Cancelled)?;
        let description = describe(args);
        let mut child = self.command(repo, args).spawn().map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                GitError::NotInstalled
            } else {
                GitError::Io(error)
            }
        })?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stderr_reader = thread::spawn(move || {
            let mut captured = Vec::new();
            if let Some(stderr) = stderr {
                let _ = stderr.take(MAX_STDERR as u64).read_to_end(&mut captured);
            }
            String::from_utf8_lossy(&captured).into_owned()
        });

        let child = Arc::new(Mutex::new(child));
        let finished = Arc::new(AtomicBool::new(false));
        let timed_out = Arc::new(AtomicBool::new(false));
        let watchdog = {
            let child = Arc::clone(&child);
            let finished = Arc::clone(&finished);
            let timed_out = Arc::clone(&timed_out);
            let cancel = cancel.clone();
            let deadline = Instant::now() + self.timeout;
            thread::spawn(move || {
                while !finished.load(Ordering::SeqCst) {
                    let expired = Instant::now() >= deadline;
                    if expired || cancel.is_cancelled() {
                        timed_out.store(expired, Ordering::SeqCst);
                        if let Ok(mut child) = child.lock() {
                            let _ = child.kill();
                        }
                        return;
                    }
                    thread::sleep(Duration::from_millis(25));
                }
            })
        };

        let consumed = match stdout {
            Some(stdout) => consume(&mut BufReader::new(stdout)),
            None => Ok(()),
        };
        let status = wait(&child);
        finished.store(true, Ordering::SeqCst);
        let _ = watchdog.join();
        let stderr = stderr_reader.join().unwrap_or_default();

        if cancel.is_cancelled() {
            return Err(GitError::Cancelled);
        }
        if timed_out.load(Ordering::SeqCst) {
            return Err(GitError::Timeout {
                command: description,
            });
        }
        let status = status?;
        if !status.success() {
            return Err(GitError::from_failure(
                &description,
                status.code(),
                &stderr,
                repo,
            ));
        }
        consumed
    }
}

fn wait(child: &Arc<Mutex<Child>>) -> Result<std::process::ExitStatus, GitError> {
    loop {
        {
            let mut guard = child
                .lock()
                .map_err(|_| GitError::Parse("process lock poisoned".to_owned()))?;
            if let Some(status) = guard.try_wait().map_err(GitError::Io)? {
                return Ok(status);
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
}

/// A short description of a command for error messages (subcommand and flags only).
fn describe(args: &[&str]) -> String {
    let shown: Vec<&str> = args
        .iter()
        .copied()
        .filter(|arg| arg.starts_with('-') || !arg.contains("://"))
        .take(6)
        .collect();
    format!("git {}", shown.join(" "))
}

/// Rejects values that Git could interpret as options when passed as revisions.
pub fn validate_revision(revision: &str) -> Result<(), GitError> {
    if revision.is_empty()
        || revision.starts_with('-')
        || revision.chars().any(|c| c.is_control() || c == ' ')
    {
        return Err(GitError::InvalidRevision(revision.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_testkit::{GitRepo, git_available};

    #[test]
    fn detects_git_and_runs_commands() {
        if !git_available() {
            return;
        }
        let runner = GitRunner::detect().unwrap();
        assert!(runner.version().chars().next().unwrap().is_ascii_digit());
        let repo = GitRepo::new();
        repo.write("a.txt", "hi");
        repo.commit("init", "A", "a@example.invalid", "2024-01-01T00:00:00Z");
        let head = runner
            .text(
                repo.path(),
                &["rev-parse", "HEAD"],
                &CancellationToken::new(),
            )
            .unwrap();
        assert_eq!(head.len(), 40);
    }

    #[test]
    fn reports_failures_with_context() {
        if !git_available() {
            return;
        }
        let runner = GitRunner::detect().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let error = runner
            .text(
                dir.path(),
                &["rev-parse", "HEAD"],
                &CancellationToken::new(),
            )
            .unwrap_err();
        assert!(matches!(error, GitError::NotARepository(_)), "{error:?}");
    }

    #[test]
    fn honors_cancellation() {
        if !git_available() {
            return;
        }
        let runner = GitRunner::detect().unwrap();
        let token = CancellationToken::new();
        token.cancel();
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            runner.text(dir.path(), &["--version"], &token),
            Err(GitError::Cancelled)
        ));
    }

    #[test]
    fn validates_revisions() {
        assert!(validate_revision("v1.0.0").is_ok());
        assert!(validate_revision("HEAD~3").is_ok());
        assert!(validate_revision("--output=/tmp/x").is_err());
        assert!(validate_revision("a b").is_err());
        assert!(validate_revision("").is_err());
    }

    #[test]
    fn descriptions_omit_urls() {
        assert_eq!(
            describe(&[
                "clone",
                "--depth",
                "1",
                "https://token@example.com/r.git",
                "dest"
            ]),
            "git clone --depth 1 dest"
        );
    }
}
