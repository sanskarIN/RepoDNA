//! Error types shared by the RepoDNA domain model.

use std::path::PathBuf;

/// Errors produced while loading configuration or reading and writing artifacts.
///
/// Every variant carries enough context to explain *what* happened; [`CoreError::hint`]
/// adds *what the user can do next* where a useful suggestion exists.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// A file could not be read or written.
    #[error("could not access {path}: {source}")]
    Io {
        /// The path that was being accessed.
        path: PathBuf,
        /// The underlying operating-system error.
        #[source]
        source: std::io::Error,
    },

    /// A configuration file could not be parsed or contained invalid values.
    #[error("invalid configuration in {origin}: {message}")]
    Config {
        /// Where the configuration came from (a file path or a description such as "command line").
        origin: String,
        /// Human-readable description of the problem.
        message: String,
    },

    /// A document claimed to be a RepoDNA artifact but its structure is not valid.
    #[error("invalid RepoDNA artifact: {0}")]
    InvalidArtifact(String),

    /// The artifact uses a schema version this build cannot read.
    #[error(
        "unsupported artifact schema version {found} (this build of RepoDNA reads schema {supported})"
    )]
    UnsupportedSchema {
        /// The schema version found in the artifact.
        found: String,
        /// The schema version supported by this build.
        supported: String,
    },

    /// JSON serialization or deserialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl CoreError {
    /// Creates an [`CoreError::Io`] error for `path`.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// Creates a [`CoreError::Config`] error.
    pub fn config(origin: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Config {
            origin: origin.into(),
            message: message.into(),
        }
    }

    /// Returns an actionable suggestion for resolving the error, when one is known.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::Io { source, .. } => match source.kind() {
                std::io::ErrorKind::NotFound => {
                    Some("Check that the path exists and is spelled correctly.")
                }
                std::io::ErrorKind::PermissionDenied => Some(
                    "Check the file permissions, or run RepoDNA as a user that can read the path.",
                ),
                _ => None,
            },
            Self::Config { .. } => Some(
                "Run `repodna config validate` to see every configuration problem, or `repodna config show` to inspect the effective configuration.",
            ),
            Self::InvalidArtifact(_) | Self::Json(_) => Some(
                "Regenerate the artifact with `repodna analyze <path> --output <file>`; artifacts must not be edited by hand.",
            ),
            Self::UnsupportedSchema { .. } => Some(
                "Open the artifact with the RepoDNA version that produced it, or re-analyze the repository with this version.",
            ),
        }
    }
}

/// Convenience result alias for fallible core operations.
pub type Result<T, E = CoreError> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_errors_explain_not_found() {
        let error = CoreError::io(
            "missing.toml",
            std::io::Error::new(std::io::ErrorKind::NotFound, "no such file"),
        );
        assert!(error.to_string().contains("missing.toml"));
        assert_eq!(
            error.hint(),
            Some("Check that the path exists and is spelled correctly.")
        );
    }

    #[test]
    fn schema_errors_name_both_versions() {
        let error = CoreError::UnsupportedSchema {
            found: "2.0".into(),
            supported: "1.0".into(),
        };
        let message = error.to_string();
        assert!(message.contains("2.0"));
        assert!(message.contains("1.0"));
        assert!(error.hint().is_some());
    }
}
