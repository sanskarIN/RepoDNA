//! The RepoDNA analysis engine.
//!
//! The engine resolves an input (a directory, a Git URL, or an archive), runs the analysis
//! stages enabled by the configuration, and assembles a [`RepositoryDna`] artifact. Stages
//! that cannot run (for example Git history when Git is not installed) are recorded as
//! unavailable or skipped with a reason instead of failing the whole analysis.
//!
//! [`RepositoryDna`]: repodna_core::model::artifact::RepositoryDna

pub mod error;
pub mod input;
pub mod progress;
pub mod scan;
pub mod structure;

pub use error::EngineError;
pub use input::{FetchOptions, InputSpec, PreparedInput, prepare_input};
pub use progress::{Progress, ProgressEvent};
