//! The versioned `RepositoryDNA` artifact schema.
//!
//! Each analysis area has its own section type. Every section carries a
//! [`SectionStatus`] so consumers can tell "the analyzer ran and found nothing" apart
//! from "the analyzer did not run" — RepoDNA must never present missing analysis as a
//! negative result.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod languages;
pub mod structure;

/// Whether an analysis section was produced, and how completely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SectionStatus {
    /// The analyzer ran to completion.
    Analyzed,
    /// The analyzer ran, but some inputs could not be processed; see the section notes.
    Partial,
    /// The analyzer was not enabled by the selected profile or configuration.
    #[default]
    Skipped,
    /// The analyzer could not run, for example because Git is not installed.
    Unavailable,
}

impl SectionStatus {
    /// Returns `true` when the section contains analysis results (complete or partial).
    pub const fn has_results(self) -> bool {
        matches!(self, SectionStatus::Analyzed | SectionStatus::Partial)
    }

    /// Human-readable label.
    pub const fn label(self) -> &'static str {
        match self {
            SectionStatus::Analyzed => "Analyzed",
            SectionStatus::Partial => "Partially analyzed",
            SectionStatus::Skipped => "Not analyzed",
            SectionStatus::Unavailable => "Unavailable",
        }
    }
}

/// Serde helper: skip serializing `false` booleans to keep artifacts compact.
pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_status_is_skipped() {
        assert_eq!(SectionStatus::default(), SectionStatus::Skipped);
        assert!(!SectionStatus::Skipped.has_results());
        assert!(SectionStatus::Partial.has_results());
        assert_eq!(
            serde_json::to_string(&SectionStatus::Unavailable).unwrap(),
            "\"unavailable\""
        );
    }
}
