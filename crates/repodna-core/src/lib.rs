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
pub mod error;
pub mod severity;
pub mod time;

pub use confidence::Confidence;
pub use error::{CoreError, Result};
pub use severity::Severity;
pub use time::Timestamp;
