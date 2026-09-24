//! User-configurable thresholds. They are stored with every artifact so readers know which
//! limits produced the signals they see.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Thresholds used by analyzers to turn measurements into signals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct Thresholds {
    /// Code lines above which a file is reported as large.
    pub large_file_lines: u64,
    /// Lines above which a function is reported as large.
    pub large_function_lines: u32,
    /// Approximate cyclomatic complexity above which a function is reported.
    pub high_complexity: u32,
    /// Block nesting depth above which a function is reported.
    pub deep_nesting: u32,
    /// Share of recent churn (0–1) above which a directory is reported as a high-churn area.
    pub high_churn_share: f64,
    /// Share of internal module edges (0–1) pointing at one module above which a
    /// dependency-concentration signal is reported.
    pub dependency_concentration: f64,
    /// Minimum duplicated block size in normalized tokens.
    pub duplicate_min_tokens: u32,
    /// Minimum estimated Jaccard similarity (0–1) for similar-file pairs.
    pub similarity: f64,
    /// Minimum gap in days to report a dormant period.
    pub dormant_days: u32,
    /// Size of the "recent" window in days (before the latest commit).
    pub recent_days: u32,
    /// Minimum commits for a file to be considered as a hotspot.
    pub hotspot_min_commits: u32,
    /// Days a manifest must be unchanged before a stale-declarations signal is reported.
    pub stale_manifest_days: u32,
    /// Size in bytes above which a committed binary file is reported.
    pub large_binary_bytes: u64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            large_file_lines: 1000,
            large_function_lines: 80,
            high_complexity: 15,
            deep_nesting: 5,
            high_churn_share: 0.3,
            dependency_concentration: 0.3,
            duplicate_min_tokens: 70,
            similarity: 0.8,
            dormant_days: 90,
            recent_days: 90,
            hotspot_min_commits: 3,
            stale_manifest_days: 730,
            large_binary_bytes: 5 * 1024 * 1024,
        }
    }
}

impl Thresholds {
    /// Returns a description of every out-of-range value.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let mut require = |ok: bool, message: &str| {
            if !ok {
                problems.push(message.to_owned());
            }
        };
        require(
            self.large_file_lines > 0,
            "thresholds.large_file_lines must be greater than 0",
        );
        require(
            self.large_function_lines > 0,
            "thresholds.large_function_lines must be greater than 0",
        );
        require(
            self.high_complexity > 0,
            "thresholds.high_complexity must be greater than 0",
        );
        require(
            self.deep_nesting > 0,
            "thresholds.deep_nesting must be greater than 0",
        );
        require(
            (0.0..=1.0).contains(&self.high_churn_share),
            "thresholds.high_churn_share must be between 0 and 1",
        );
        require(
            (0.0..=1.0).contains(&self.dependency_concentration),
            "thresholds.dependency_concentration must be between 0 and 1",
        );
        require(
            self.duplicate_min_tokens >= 10,
            "thresholds.duplicate_min_tokens must be at least 10",
        );
        require(
            (0.0..=1.0).contains(&self.similarity) && self.similarity > 0.0,
            "thresholds.similarity must be greater than 0 and at most 1",
        );
        require(
            self.dormant_days > 0,
            "thresholds.dormant_days must be greater than 0",
        );
        require(
            self.recent_days > 0,
            "thresholds.recent_days must be greater than 0",
        );
        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        assert!(Thresholds::default().validate().is_empty());
    }

    #[test]
    fn reports_out_of_range_values() {
        let thresholds = Thresholds {
            high_churn_share: 1.5,
            duplicate_min_tokens: 3,
            ..Thresholds::default()
        };
        let problems = thresholds.validate();
        assert_eq!(problems.len(), 2);
        assert!(problems[0].contains("high_churn_share"));
    }

    #[test]
    fn partial_tables_fill_in_defaults() {
        let thresholds: Thresholds = toml::from_str("large_file_lines = 500").unwrap();
        assert_eq!(thresholds.large_file_lines, 500);
        assert_eq!(thresholds.high_complexity, 15);
        assert!(toml::from_str::<Thresholds>("unknown = 1").is_err());
    }
}
