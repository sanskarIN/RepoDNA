//! Errors from Git analysis.

use std::path::{Path, PathBuf};

/// Something went wrong while running or interpreting Git.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    /// No usable `git` executable was found.
    #[error("Git is not installed or not on PATH")]
    NotInstalled,
    /// The path is not inside a Git working tree.
    #[error("{0} is not a Git repository")]
    NotARepository(PathBuf),
    /// Git refused to operate on a repository owned by another user.
    #[error("Git refused to read {0} because it is owned by another user")]
    UnsafeRepository(PathBuf),
    /// A Git command exited with an error.
    #[error("{command} failed{}: {stderr}", status.map(|code| format!(" with exit code {code}")).unwrap_or_default())]
    CommandFailed {
        /// The command, without URLs or credentials.
        command: String,
        /// Exit code, if the process exited normally.
        status: Option<i32>,
        /// Standard error, truncated.
        stderr: String,
    },
    /// A Git command took longer than the timeout.
    #[error("{command} timed out")]
    Timeout {
        /// The command.
        command: String,
    },
    /// The operation was cancelled.
    #[error("the Git operation was cancelled")]
    Cancelled,
    /// A revision argument was rejected.
    #[error("invalid revision {0:?}")]
    InvalidRevision(String),
    /// A remote URL was rejected.
    #[error("invalid repository URL: {0}")]
    InvalidUrl(String),
    /// Git output could not be interpreted.
    #[error("unexpected Git output: {0}")]
    Parse(String),
    /// An I/O error occurred while talking to Git.
    #[error("I/O error while running Git: {0}")]
    Io(#[from] std::io::Error),
}

/// Redact HTTP(S) userinfo echoed by Git before an error reaches logs or the UI.
///
/// Git can include the original URL in a failed clone's stderr, even though
/// command descriptions avoid URLs. Only the authority's userinfo is removed;
/// host, path and Git's actionable diagnostics remain available. The scan is
/// ASCII case-insensitive so URLs such as HTTPS:// also receive protection.
fn redact_http_url_credentials(stderr: &str) -> String {
    let lowercase = stderr.to_ascii_lowercase();
    let mut redacted = String::with_capacity(stderr.len());
    let mut cursor = 0;

    while cursor < stderr.len() {
        let next = ["https://", "http://"]
            .iter()
            .filter_map(|scheme| {
                lowercase[cursor..]
                    .find(scheme)
                    .map(|offset| (cursor + offset, scheme.len()))
            })
            .min_by_key(|(position, _)| *position);
        let Some((scheme_start, scheme_len)) = next else {
            break;
        };
        let authority_start = scheme_start + scheme_len;
        redacted.push_str(&stderr[cursor..authority_start]);
        let authority_end = stderr[authority_start..]
            .find(|character: char| {
                character == '/'
                    || character == '?'
                    || character == '#'
                    || character.is_whitespace()
                    || matches!(character, '\'' | '"' | '<' | '>')
            })
            .map_or(stderr.len(), |offset| authority_start + offset);
        let authority = &stderr[authority_start..authority_end];
        if let Some((_, host)) = authority.rsplit_once('@') {
            redacted.push_str("[REDACTED]@");
            redacted.push_str(host);
        } else {
            redacted.push_str(authority);
        }
        cursor = authority_end;
    }

    redacted.push_str(&stderr[cursor..]);
    redacted
}

impl GitError {
    pub(crate) fn from_failure(
        command: &str,
        status: Option<i32>,
        stderr: &str,
        repo: Option<&Path>,
    ) -> Self {
        let lower = stderr.to_ascii_lowercase();
        let repo_path = repo.map(Path::to_path_buf).unwrap_or_default();
        if lower.contains("dubious ownership") || lower.contains("unsafe repository") {
            return GitError::UnsafeRepository(repo_path);
        }
        if lower.contains("not a git repository") {
            return GitError::NotARepository(repo_path);
        }
        GitError::CommandFailed {
            command: command.to_owned(),
            status,
            stderr: redact_http_url_credentials(stderr)
                .trim()
                .chars()
                .take(2_000)
                .collect(),
        }
    }

    /// An actionable suggestion for the user, when one exists.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            GitError::NotInstalled => Some(
                "Install Git to analyze history, or use `--profile quick` to analyze files only.",
            ),
            GitError::NotARepository(_) => Some(
                "The directory has no Git metadata. RepoDNA can still analyze its files; history-based sections will be unavailable.",
            ),
            GitError::UnsafeRepository(_) => Some(
                "If you trust this repository, run `git config --global --add safe.directory <path>` and try again.",
            ),
            GitError::Timeout { .. } => Some(
                "Lower `analysis.max_commits` in your configuration or use `--profile quick` for very large histories.",
            ),
            GitError::InvalidUrl(_) => Some(
                "Use an https:// or ssh:// URL (or git@host:owner/repo). Local paths should be passed directly, not as URLs.",
            ),
            GitError::CommandFailed { stderr, .. } => remote_hint(stderr),
            _ => None,
        }
    }
}

/// What Git says when a remote refuses access, and when it cannot be reached at all.
const ACCESS_DENIED: &[&str] = &[
    "authentication failed",
    "could not read username",
    "could not read password",
    "terminal prompts disabled",
    "permission denied",
    "repository not found",
    "returned error: 401",
    "returned error: 403",
    "returned error: 404",
];
const UNREACHABLE: &[&str] = &[
    "could not resolve host",
    "unable to access",
    "failed to connect",
    "connection timed out",
    "connection refused",
    "network is unreachable",
    "could not resolve proxy",
];

/// A hint for a clone or fetch that failed because of the remote, when Git says why.
fn remote_hint(stderr: &str) -> Option<&'static str> {
    let lower = stderr.to_ascii_lowercase();
    if ACCESS_DENIED.iter().any(|text| lower.contains(text)) {
        Some(
            "The repository may be private, or the address may be wrong. RepoDNA never prompts for credentials: set them up for Git itself (a credential helper or an SSH agent), or clone the repository yourself and pass its local path.",
        )
    } else if UNREACHABLE.iter().any(|text| lower.contains(text)) {
        Some(
            "RepoDNA could not reach the repository's host. Check the address, your network connection, and any proxy, or clone the repository yourself and pass its local path.",
        )
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_failures() {
        let unsafe_repo = GitError::from_failure(
            "git log",
            Some(128),
            "fatal: detected dubious ownership in repository at '/x'",
            Some(Path::new("/x")),
        );
        assert!(matches!(unsafe_repo, GitError::UnsafeRepository(_)));
        assert!(unsafe_repo.hint().unwrap().contains("safe.directory"));
        let not_repo =
            GitError::from_failure("git log", Some(128), "fatal: not a git repository", None);
        assert!(matches!(not_repo, GitError::NotARepository(_)));
        let other = GitError::from_failure("git log", Some(1), "boom", None);
        assert_eq!(other.to_string(), "git log failed with exit code 1: boom");
        assert!(other.hint().is_none());
    }

    #[test]
    fn redacts_credentials_echoed_by_git_without_losing_diagnostics() {
        let stderr = "fatal: unable to access 'https://user:example-secret@github.com/org/repo.git/': The requested URL returned error: 403";
        let failed = GitError::from_failure("git clone", Some(128), stderr, None);
        let message = failed.to_string();
        assert!(!message.contains("example-secret"), "{message}");
        assert!(!message.contains("user:"), "{message}");
        assert!(message.contains("https://[REDACTED]@github.com/org/repo.git/"));
        assert!(message.contains("error: 403"));
        assert!(failed.hint().unwrap().contains("may be private"));

        let mixed = "first HTTPS://login:another-secret@github.com/a then http://a:b@host.test/b";
        let scrubbed = redact_http_url_credentials(mixed);
        assert_eq!(
            scrubbed,
            "first HTTPS://[REDACTED]@github.com/a then http://[REDACTED]@host.test/b"
        );
        assert_eq!(
            redact_http_url_credentials("fatal: unable to access 'https://example.com/a.git'"),
            "fatal: unable to access 'https://example.com/a.git'"
        );
    }

    #[test]
    fn explains_why_a_clone_failed() {
        let clone = |stderr: &str| GitError::from_failure("git clone", Some(128), stderr, None);
        let private = clone(
            "fatal: could not read Username for 'https://github.com': terminal prompts disabled",
        );
        assert!(private.hint().unwrap().contains("may be private"));
        let missing = clone(
            "fatal: unable to access 'https://example.com/a.git/': The requested URL returned error: 404",
        );
        assert!(missing.hint().unwrap().contains("may be private"));
        let offline = clone(
            "fatal: unable to access 'https://example.com/a.git/': Could not resolve host: example.com",
        );
        assert!(offline.hint().unwrap().contains("could not reach"));
        let proxy = clone(
            "fatal: unable to access 'https://example.com/a.git/': CONNECT tunnel failed, response 403",
        );
        assert!(proxy.hint().unwrap().contains("could not reach"));
    }
}
