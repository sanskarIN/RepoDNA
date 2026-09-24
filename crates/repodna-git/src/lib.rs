//! # repodna-git
//!
//! Read-only Git history analysis. RepoDNA invokes the `git` executable through a
//! hardened runner instead of linking a Git library, which keeps the binary small,
//! supports every repository format Git itself supports, and degrades gracefully when Git
//! is not installed.

pub mod clone;
pub mod error;
pub mod history;
pub mod log;
pub mod refs;
pub mod runner;
pub mod tree;
pub mod url;

pub use clone::{CloneOptions, clone_repository};
pub use error::GitError;
pub use history::{HistoryOptions, aggregate, area_of, contributor_id, parse_version, releases};
pub use log::{ChangeStatus, FileChange, History, LogOptions, ParsedCommit, read_history};
pub use refs::{RepositoryState, read_branches, read_state, read_tags, repository_root};
pub use runner::GitRunner;
pub use tree::{BlobLimits, TreeEntry, list_tree, read_blobs};
