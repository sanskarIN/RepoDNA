//! Report errors.

use std::path::PathBuf;

/// Why a report could not be produced.
#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    /// An image could not be rendered.
    #[error("could not render the image: {0}")]
    Render(String),
    /// The requested options are invalid.
    #[error("invalid report options: {0}")]
    Options(String),
    /// JSON serialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// A file could not be written.
    #[error("could not write {path}: {source}")]
    Io {
        /// The path being written.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
}

impl ReportError {
    /// Wraps an I/O error with the path involved.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        ReportError::Io {
            path: path.into(),
            source,
        }
    }
}
