//! Finding severity levels and the documented criteria behind them.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How urgently a finding deserves attention.
///
/// Severity is never assigned arbitrarily: every analyzer maps its findings to one of these
/// levels using the criteria returned by [`Severity::criteria`], which are also published in
/// the documentation. Most quality signals are `Info` or `Attention`, because a metric on its
/// own rarely proves a defect.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// Neutral observations that describe the repository.
    Info,
    /// Structural signals that are worth reviewing but do not indicate a problem by themselves.
    Attention,
    /// Likely problems supported by at least medium-confidence evidence.
    Warning,
    /// High-confidence evidence of an immediate risk, such as private key material.
    Critical,
}

impl Severity {
    /// All severities from most to least urgent.
    pub const ALL: [Severity; 4] = [
        Severity::Critical,
        Severity::Warning,
        Severity::Attention,
        Severity::Info,
    ];

    /// Returns the human-readable label, e.g. `"Attention"`.
    pub const fn label(self) -> &'static str {
        match self {
            Severity::Info => "Informational",
            Severity::Attention => "Attention",
            Severity::Warning => "Warning",
            Severity::Critical => "Critical",
        }
    }

    /// Returns the published criterion an analyzer must satisfy to use this severity.
    pub const fn criteria(self) -> &'static str {
        match self {
            Severity::Info => {
                "Descriptive observations with no implied action, such as a dormant period or a newly introduced module."
            }
            Severity::Attention => {
                "Structural signals worth reviewing (hotspots, large or complex code, duplication, missing optional documentation) that do not establish a defect on their own."
            }
            Severity::Warning => {
                "Likely problems backed by medium or high confidence evidence, such as a module dependency cycle, a lockfile that disagrees with its manifest, or a probable credential."
            }
            Severity::Critical => {
                "High-confidence evidence of an immediate exposure risk, such as private key material outside test or example paths."
            }
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_by_urgency() {
        assert!(Severity::Info < Severity::Attention);
        assert!(Severity::Attention < Severity::Warning);
        assert!(Severity::Warning < Severity::Critical);
    }

    #[test]
    fn every_severity_documents_its_criteria() {
        for severity in Severity::ALL {
            assert!(
                severity.criteria().len() > 40,
                "{severity} needs a real criterion"
            );
        }
        assert_eq!(
            serde_json::to_string(&Severity::Attention).unwrap(),
            "\"attention\""
        );
    }
}
