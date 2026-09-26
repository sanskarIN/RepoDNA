//! Raw metrics, normalized signals, and per-section confidence.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::confidence::Confidence;
use crate::metric::Metric;

/// A metric normalized to 0–1 for visualization, with its interpretation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedSignal {
    /// Identifier.
    pub id: String,
    /// Label.
    pub label: String,
    /// Normalized value (0–1).
    pub value: f64,
    /// Neutral interpretation of the value.
    pub interpretation: String,
    /// Confidence.
    pub confidence: Confidence,
}

/// Confidence of an analysis section and the reason for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SectionConfidence {
    /// Section identifier, e.g. `architecture`.
    pub section: String,
    /// Confidence.
    pub confidence: Confidence,
    /// Why the confidence was assigned.
    pub reason: String,
}

/// All measurements of the analysis.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetricsReport {
    /// Raw metrics, sorted by identifier.
    #[serde(default)]
    pub raw: Vec<Metric>,
    /// Normalized signals.
    #[serde(default)]
    pub signals: Vec<NormalizedSignal>,
    /// Confidence per section.
    #[serde(default)]
    pub confidence: Vec<SectionConfidence>,
}

impl MetricsReport {
    /// Looks up a raw metric by identifier.
    pub fn get(&self, id: &str) -> Option<&Metric> {
        self.raw.iter().find(|metric| metric.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_raw_metrics() {
        let report = MetricsReport {
            raw: vec![Metric::new("structure.files", "Files", 3.0, "files")],
            ..MetricsReport::default()
        };
        assert_eq!(report.get("structure.files").map(|m| m.value), Some(3.0));
        assert!(report.get("missing").is_none());
    }
}
