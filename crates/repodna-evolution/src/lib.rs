//! Evolution analysis for RepoDNA: how a repository came to be what it is.
//!
//! Everything here is derived from commit history and from tree listings of selected
//! revisions. Statements are either facts that link to commits and snapshots, or
//! interpretations that are labeled as such; nothing is inferred about people.

pub mod ages;
pub mod analysis;
pub mod epochs;
pub mod events;
pub mod names;
pub mod recent;
pub mod snapshots;
pub mod story;

pub use analysis::{EvolutionInput, EvolutionOutput, analyze};
pub use names::LanguageNames;

#[cfg(test)]
pub(crate) mod testutil;
