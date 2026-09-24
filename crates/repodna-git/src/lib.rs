//! # repodna-git
//!
//! Read-only Git history analysis. RepoDNA invokes the `git` executable through a
//! hardened runner instead of linking a Git library, which keeps the binary small,
//! supports every repository format Git itself supports, and degrades gracefully when Git
//! is not installed.

pub mod error;
pub mod log;
pub mod refs;
pub mod runner;
pub mod url;

pub use error::GitError;
pub use log::{ChangeStatus, FileChange, History, LogOptions, ParsedCommit, read_history};
pub use refs::{RepositoryState, read_branches, read_state, read_tags, repository_root};
pub use runner::GitRunner;
