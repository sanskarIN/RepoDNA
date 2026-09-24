//! # repodna-core
//!
//! The domain model shared by every part of RepoDNA: the versioned `RepositoryDNA`
//! artifact, findings and their evidence, confidence levels, configuration, and the
//! helpers used to read and write artifacts safely.
//!
//! This crate deliberately contains no analysis logic. Analyzers live in their own
//! crates and produce the types defined here, which keeps the model stable, easy to
//! serialize, and independent from any user interface.

pub mod confidence;
pub mod config;
pub mod error;
pub mod evidence;
pub mod finding;
pub mod glob;
pub mod hash;
pub mod metric;
pub mod model;
pub mod paths;
pub mod severity;
pub mod time;

pub use confidence::Confidence;
pub use error::{CoreError, Result};
pub use evidence::Evidence;
pub use finding::{Finding, FindingCategory, Suppression};
pub use metric::Metric;
pub use model::artifact::{RepositoryDna, compute_dna_hash};
pub use model::metadata::SCHEMA_VERSION;
pub use severity::Severity;
pub use time::Timestamp;
