//! Metrics: measured values with their definition, method, and limitations.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::confidence::Confidence;
use crate::evidence::finite;

/// A measured value, documented well enough that a reader can reproduce it.
///
/// Every metric answers: what is measured (`definition`), how (`method`), the current value,
/// the configured threshold if any, and the confidence and limitations of the measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Metric {
    /// Identifier, e.g. `structure.files.total`.
    pub id: String,
    /// Short human-readable label.
    pub label: String,
    /// Measured value.
    pub value: f64,
    /// Unit of the value, e.g. `files`, `lines`, `%`, `commits`, or `ratio`.
    pub unit: String,
    /// What the metric measures.
    pub definition: String,
    /// How the value was calculated.
    pub method: String,
    /// Threshold used for signals derived from this metric, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// Confidence of the measurement method.
    pub confidence: Confidence,
    /// Known limitations of the measurement.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

impl Metric {
    /// Creates a metric. Non-finite values are stored as zero because JSON cannot represent them.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        value: f64,
        unit: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value: round4(finite(value)),
            unit: unit.into(),
            definition: String::new(),
            method: String::new(),
            threshold: None,
            confidence: Confidence::High,
            limitations: Vec::new(),
        }
    }

    /// Sets the definition.
    #[must_use]
    pub fn definition(mut self, text: impl Into<String>) -> Self {
        self.definition = text.into();
        self
    }

    /// Sets the method.
    #[must_use]
    pub fn method(mut self, text: impl Into<String>) -> Self {
        self.method = text.into();
        self
    }

    /// Sets the threshold.
    #[must_use]
    pub fn threshold(mut self, value: f64) -> Self {
        self.threshold = Some(round4(finite(value)));
        self
    }

    /// Sets the confidence.
    #[must_use]
    pub fn confidence(mut self, confidence: Confidence) -> Self {
        self.confidence = confidence;
        self
    }

    /// Appends a limitation.
    #[must_use]
    pub fn limitation(mut self, text: impl Into<String>) -> Self {
        self.limitations.push(text.into());
        self
    }

    /// Returns `true` if the metric has a threshold and the value exceeds it.
    pub fn exceeds_threshold(&self) -> bool {
        self.threshold
            .is_some_and(|threshold| self.value > threshold)
    }
}

/// Rounds to four decimal places so serialized artifacts are compact and stable.
pub fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_documented_metrics() {
        let metric = Metric::new("quality.lines.max", "Longest file", 1234.0, "lines")
            .definition("Largest number of lines in a single file.")
            .method("Maximum over analyzed source files.")
            .threshold(1000.0)
            .confidence(Confidence::High)
            .limitation("Generated files are excluded.");
        assert!(metric.exceeds_threshold());
        let json = serde_json::to_value(&metric).unwrap();
        assert_eq!(json["unit"], "lines");
        assert_eq!(json["threshold"], 1000.0);
    }

    #[test]
    fn sanitizes_and_rounds_values() {
        assert_eq!(Metric::new("x", "x", f64::NAN, "ratio").value, 0.0);
        assert_eq!(Metric::new("x", "x", 0.123_456_7, "ratio").value, 0.1235);
        assert!(!Metric::new("x", "x", 1.0, "ratio").exceeds_threshold());
    }
}
