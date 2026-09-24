//! Engine errors with hints for users.

use std::path::PathBuf;

use repodna_core::cancel::Cancelled;
use repodna_discovery::DiscoveryError;
use repodna_discovery::archive::ArchiveError;
use repodna_git::GitError;

/// Why an analysis could not run.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// The input path does not exist.
    #[error("{0} does not exist")]
    NotFound(PathBuf),
    /// The input is neither a directory, an archive, nor a URL.
    #[error("{0} is not a directory or a supported archive")]
    UnsupportedInput(String),
    /// The configuration cannot be applied.
    #[error("invalid configuration: {0}")]
    Config(String),
    /// Discovery failed.
    #[error(transparent)]
    Discovery(#[from] DiscoveryError),
    /// A Git operation failed.
    #[error(transparent)]
    Git(#[from] GitError),
    /// An archive could not be extracted.
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    /// A file-system operation failed.
    #[error("{context}: {source}")]
    Io {
        /// What was being done.
        context: String,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The analysis was cancelled.
    #[error("the analysis was cancelled")]
    Cancelled,
}

impl From<Cancelled> for EngineError {
    fn from(_: Cancelled) -> Self {
        EngineError::Cancelled
    }
}

impl EngineError {
    /// Wraps an I/O error with context.
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        EngineError::Io {
            context: context.into(),
            source,
        }
    }

    /// A suggestion for fixing the problem, when there is one.
    pub fn hint(&self) -> Option<String> {
        match self {
            EngineError::NotFound(_) => {
                Some("Check the path, or pass a Git URL or an archive instead.".to_owned())
            }
            EngineError::UnsupportedInput(_) => Some(
                "Pass a directory, a .zip, .tar, .tar.gz, or .tgz archive, or a Git URL."
                    .to_owned(),
            ),
            EngineError::Git(error) => error.hint().map(str::to_owned),
            EngineError::Archive(_) => {
                Some("Check that the archive is complete and not password-protected.".to_owned())
            }
            EngineError::Config(_) => {
                Some("Fix the value in your repodna.toml or user configuration.".to_owned())
            }
            EngineError::Discovery(DiscoveryError::InvalidPattern { .. }) => {
                Some("Fix the ignore pattern in your configuration.".to_owned())
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_have_messages_and_hints() {
        let error = EngineError::NotFound(PathBuf::from("missing"));
        assert_eq!(error.to_string(), "missing does not exist");
        assert!(error.hint().is_some());
        let io = EngineError::io("reading x", std::io::Error::other("boom"));
        assert_eq!(io.to_string(), "reading x: boom");
        assert!(io.hint().is_none());
        assert!(matches!(
            EngineError::from(Cancelled),
            EngineError::Cancelled
        ));
    }
}
