//! # repodna-git
//!
//! Read-only Git history analysis. RepoDNA invokes the `git` executable through a
//! hardened runner instead of linking a Git library, which keeps the binary small,
//! supports every repository format Git itself supports, and degrades gracefully when Git
//! is not installed.

pub mod error;
pub mod runner;

pub use error::GitError;
pub use runner::GitRunner;
