//! Local storage for RepoDNA.
//!
//! Everything lives under one directory (see [`paths`]): a SQLite database that indexes
//! repositories, scans, metrics, and findings, the analysis artifacts themselves as JSON
//! files, and a content-addressed cache of per-file analysis results. The source
//! repositories are never written to.
//!
//! The database schema is versioned and upgraded in transactions, so an interrupted or
//! failed upgrade leaves the previous data intact. A database that cannot be opened is
//! never deleted automatically; [`repair`] moves it aside and rebuilds the index from the
//! stored artifacts.

pub mod error;
pub mod paths;
pub mod schema;
pub mod store;

pub use error::StoreError;
pub use paths::{StorePaths, config_file, data_home};
pub use store::{
    FindingChanges, RepositoryLocation, RepositoryRecord, ScanRecord, Store, StoredFinding,
};
