//! Defensive security signals. Secret values are never stored.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::dependencies::AdvisoryStatus;
use super::{SectionStatus, is_false};
use crate::confidence::Confidence;

/// A location that looks like it contains a credential.
///
/// Only the location and a keyed fingerprint are stored; the value itself is never written
/// to artifacts, reports, logs, or telemetry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretCandidate {
    /// Stable identifier.
    pub id: String,
    /// Rule that matched, e.g. `private-key`.
    pub rule: String,
    /// What the rule detects.
    pub description: String,
    /// Repository-relative path.
    pub path: String,
    /// Line (1-based).
    pub line: u32,
    /// Confidence that the match is a real credential.
    pub confidence: Confidence,
    /// Keyed fingerprint that identifies identical values within this scan without
    /// revealing them.
    pub fingerprint: String,
    /// `true` when the path looks like tests, fixtures, examples, or documentation.
    #[serde(default, skip_serializing_if = "is_false")]
    pub in_test_or_example: bool,
}

/// Category of a risky pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PatternCategory {
    /// A risky construct in source code.
    Code,
    /// A risky setting in a configuration file.
    Configuration,
    /// A risky CI/CD workflow construct.
    Workflow,
    /// A risky container or orchestration setting.
    Container,
}

/// A potentially dangerous pattern.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatternCandidate {
    /// Stable identifier.
    pub id: String,
    /// Rule identifier, e.g. `tls-verification-disabled`.
    pub rule: String,
    /// Pattern category.
    pub category: PatternCategory,
    /// What was detected.
    pub description: String,
    /// Suggested review action.
    pub recommendation: String,
    /// Repository-relative path.
    pub path: String,
    /// Line (1-based), when the pattern is line-based.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Confidence that the match is the risky construct.
    pub confidence: Confidence,
}

/// A file permission that may be unsafe (reported on Unix-like systems only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PermissionSignal {
    /// Repository-relative path.
    pub path: String,
    /// Permission bits in octal, e.g. `0777`.
    pub mode: String,
    /// What is unusual about the permissions.
    pub issue: String,
}

/// A rule used by the security scanner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecurityRuleInfo {
    /// Rule identifier.
    pub id: String,
    /// `secret` or `pattern`.
    pub kind: String,
    /// What the rule detects.
    pub description: String,
}

/// Security signals.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecurityReport {
    /// Whether this section was analyzed.
    pub status: SectionStatus,
    /// Notes about limitations or partial results.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Possible credentials (values redacted).
    #[serde(default)]
    pub secrets: Vec<SecretCandidate>,
    /// Potentially dangerous patterns.
    #[serde(default)]
    pub patterns: Vec<PatternCandidate>,
    /// Unusual file permissions.
    #[serde(default)]
    pub permissions: Vec<PermissionSignal>,
    /// Text files scanned.
    pub files_scanned: u64,
    /// Rules applied.
    #[serde(default)]
    pub rules: Vec<SecurityRuleInfo>,
    /// Advisory integration status.
    #[serde(default)]
    pub advisories: AdvisoryStatus,
    /// What these signals do and do not mean.
    pub disclaimer: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_candidates_have_no_value_field() {
        let candidate = SecretCandidate {
            id: "s1".into(),
            rule: "private-key".into(),
            description: "Private key block".into(),
            path: "config/key.pem".into(),
            line: 1,
            confidence: Confidence::High,
            fingerprint: "abc123".into(),
            in_test_or_example: false,
        };
        let json = serde_json::to_value(&candidate).unwrap();
        let keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        assert!(!keys.iter().any(|key| key == "value" || key == "secret"));
    }
}
