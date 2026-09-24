//! Confidence levels attached to analysis results.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How strongly the available evidence supports an analysis result.
///
/// Confidence describes the *method*, not the repository: a dependency edge resolved from
/// an explicit relative import is `High`, while a module boundary inferred from directory
/// names is `Medium`. `Unavailable` means the analysis could not be performed at all, which
/// is different from "nothing was detected".
///
/// Variants are ordered from weakest to strongest, so `Confidence::Low < Confidence::High`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Confidence {
    /// The analysis could not be performed, so no conclusion can be drawn.
    Unavailable,
    /// A weak heuristic signal; treat as a pointer for manual investigation.
    Low,
    /// Supported by consistent structural evidence, but inferred rather than declared.
    Medium,
    /// Directly supported by explicit, verifiable evidence in the repository.
    High,
}

impl Confidence {
    /// All confidence levels from strongest to weakest.
    pub const ALL: [Confidence; 4] = [
        Confidence::High,
        Confidence::Medium,
        Confidence::Low,
        Confidence::Unavailable,
    ];

    /// Returns the human-readable label, e.g. `"Medium"`.
    pub const fn label(self) -> &'static str {
        match self {
            Confidence::Unavailable => "Unavailable",
            Confidence::Low => "Low",
            Confidence::Medium => "Medium",
            Confidence::High => "High",
        }
    }

    /// Returns the weaker of two confidence levels.
    ///
    /// A conclusion built from several pieces of evidence is only as strong as its
    /// weakest input, so combining results should use this method.
    pub fn weakest(self, other: Confidence) -> Confidence {
        self.min(other)
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_from_weakest_to_strongest() {
        assert!(Confidence::Unavailable < Confidence::Low);
        assert!(Confidence::Low < Confidence::Medium);
        assert!(Confidence::Medium < Confidence::High);
        assert_eq!(Confidence::High.weakest(Confidence::Low), Confidence::Low);
    }

    #[test]
    fn serializes_as_kebab_case() {
        assert_eq!(
            serde_json::to_string(&Confidence::Medium).unwrap(),
            "\"medium\""
        );
        assert_eq!(
            serde_json::from_str::<Confidence>("\"unavailable\"").unwrap(),
            Confidence::Unavailable
        );
        assert_eq!(Confidence::High.to_string(), "High");
    }
}
