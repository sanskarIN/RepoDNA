//! Evolution analysis for RepoDNA: how a repository came to be what it is.
//!
//! Everything here is derived from commit history and from tree listings of selected
//! revisions. Statements are either facts that link to commits and snapshots, or
//! interpretations that are labeled as such; nothing is inferred about people.

pub mod ages;
pub mod epochs;
pub mod events;
pub mod recent;
pub mod snapshots;
pub mod story;

#[cfg(test)]
pub(crate) mod testutil;
