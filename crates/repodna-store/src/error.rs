//! Storage errors with hints for users.

use std::path::PathBuf;

/// Why a storage operation failed.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// No storage directory could be determined.
    #[error("no storage directory could be determined (HOME is not set)")]
    NoHome,
    /// A file-system operation failed.
    #[error("could not access {path}: {source}")]
    Io {
        /// The path being accessed.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
    /// The database reported an error.
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    /// The database was written by a newer RepoDNA.
    #[error(
        "the database uses schema version {found}, but this RepoDNA supports up to {supported}"
    )]
    NewerSchema {
        /// Version found in the database.
        found: i64,
        /// Newest version this build understands.
        supported: i64,
    },
    /// A stored artifact could not be read.
    #[error(transparent)]
    Artifact(#[from] repodna_core::error::CoreError),
    /// Stored data could not be encoded or decoded.
    #[error("stored data is invalid: {0}")]
    Invalid(String),
}

impl StoreError {
    /// Wraps an I/O error with the path involved.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        StoreError::Io {
            path: path.into(),
            source,
        }
    }

    /// A suggestion for fixing the problem, when there is one.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            StoreError::NoHome => {
                Some("Set REPODNA_HOME to the directory RepoDNA should store its data in.")
            }
            StoreError::Database(_) => Some(
                "Run `repodna cache repair` to move the database aside and rebuild it from the stored artifacts.",
            ),
            StoreError::NewerSchema { .. } => Some(
                "Upgrade RepoDNA, or point REPODNA_HOME at a different directory for this version.",
            ),
            StoreError::Io { .. } => {
                Some("Check that the storage directory exists and is writable.")
            }
            StoreError::Artifact(_) | StoreError::Invalid(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_explain_what_to_do() {
        let error = StoreError::NewerSchema {
            found: 9,
            supported: 1,
        };
        assert!(error.to_string().contains("schema version 9"));
        assert!(error.hint().unwrap().contains("Upgrade"));
        assert!(StoreError::NoHome.hint().unwrap().contains("REPODNA_HOME"));
    }
}
