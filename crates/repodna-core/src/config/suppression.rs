//! Suppression rules for false positives.
//!
//! Suppression never deletes findings: a matched finding is kept and marked with the rule's
//! reason and origin, so suppression stays visible in every report.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::finding::{Finding, Suppression};
use crate::glob::{Glob, GlobError, wildcard_match};

/// "Ignore finding X in path Y" — a user-defined suppression rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct SuppressionRule {
    /// Rule identifier or wildcard pattern, e.g. `security.secret` or `quality.*`.
    #[serde(default = "any_rule")]
    pub rule: String,
    /// Optional path glob; the rule then applies only to findings about matching paths.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Optional exact finding identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Why the finding is suppressed. Required so suppressions stay explainable.
    pub reason: String,
    /// Where the rule was defined; filled in when configuration is loaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

fn any_rule() -> String {
    "*".to_owned()
}

impl SuppressionRule {
    /// Checks the rule for mistakes that would make it silently ineffective.
    pub fn validate(&self) -> Result<(), String> {
        if self.reason.trim().is_empty() {
            return Err(format!(
                "suppression for rule {:?} needs a non-empty reason",
                self.rule
            ));
        }
        if self.rule.trim().is_empty() {
            return Err("suppression rule pattern must not be empty".to_owned());
        }
        if let Some(path) = &self.path {
            Glob::new(path).map_err(|error: GlobError| error.to_string())?;
        }
        Ok(())
    }

    /// Returns `true` if the rule applies to `finding`.
    pub fn matches(&self, finding: &Finding) -> bool {
        if !wildcard_match(&self.rule, &finding.rule) {
            return false;
        }
        if let Some(id) = &self.id
            && id != &finding.id
        {
            return false;
        }
        match &self.path {
            None => true,
            Some(pattern) => {
                let Ok(glob) = Glob::new(pattern) else {
                    return false;
                };
                finding.paths.iter().any(|path| glob.matches(path))
                    || finding
                        .evidence
                        .iter()
                        .filter_map(|evidence| evidence.path())
                        .any(|path| glob.matches(path))
            }
        }
    }
}

/// Marks findings matched by any rule as suppressed and returns how many were suppressed.
///
/// The first matching rule wins so that the recorded reason is deterministic.
pub fn apply_suppressions(findings: &mut [Finding], rules: &[SuppressionRule]) -> usize {
    let mut suppressed = 0;
    for finding in findings.iter_mut() {
        if let Some(rule) = rules.iter().find(|rule| rule.matches(finding)) {
            finding.suppressed = Some(Suppression {
                reason: rule.reason.clone(),
                source: rule
                    .source
                    .clone()
                    .unwrap_or_else(|| "configuration".to_owned()),
            });
            suppressed += 1;
        }
    }
    suppressed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::confidence::Confidence;
    use crate::evidence::Evidence;
    use crate::finding::FindingCategory;
    use crate::severity::Severity;

    fn secret_finding(path: &str) -> Finding {
        Finding::new(
            "security.secret",
            path,
            FindingCategory::Security,
            Severity::Warning,
            Confidence::Medium,
            "Possible credential",
        )
        .evidence(Evidence::line(path, 3))
    }

    fn rule(rule: &str, path: Option<&str>) -> SuppressionRule {
        SuppressionRule {
            rule: rule.into(),
            path: path.map(Into::into),
            id: None,
            reason: "Intentional test fixture".into(),
            source: Some("repodna.toml".into()),
        }
    }

    #[test]
    fn matches_rule_and_path_globs() {
        let fixture = secret_finding("tests/fixtures/app.env");
        let real = secret_finding("config/app.env");
        let suppression = rule("security.*", Some("tests/fixtures/**"));
        assert!(suppression.matches(&fixture));
        assert!(!suppression.matches(&real));
        assert!(!rule("quality.*", None).matches(&fixture));
    }

    #[test]
    fn marks_findings_without_removing_them() {
        let mut findings = vec![
            secret_finding("tests/fixtures/app.env"),
            secret_finding("src/config.env"),
        ];
        let count = apply_suppressions(&mut findings, &[rule("*", Some("tests/**"))]);
        assert_eq!(count, 1);
        assert_eq!(findings.len(), 2);
        let suppression = findings[0].suppressed.as_ref().unwrap();
        assert_eq!(suppression.source, "repodna.toml");
        assert!(!findings[1].is_suppressed());
    }

    #[test]
    fn exact_identifiers_narrow_the_rule() {
        let finding = secret_finding("a.env");
        let mut by_id = rule("*", None);
        by_id.id = Some(finding.id.clone());
        assert!(by_id.matches(&finding));
        by_id.id = Some("security.secret:000000000000".into());
        assert!(!by_id.matches(&finding));
    }

    #[test]
    fn validation_requires_a_reason_and_valid_globs() {
        let mut invalid = rule("*", Some("[unterminated"));
        assert!(invalid.validate().is_err());
        invalid.path = None;
        invalid.reason = "  ".into();
        assert!(invalid.validate().unwrap_err().contains("reason"));
        assert!(rule("*", Some("**/*.pem")).validate().is_ok());
    }

    #[test]
    fn parses_from_toml_with_default_rule() {
        let parsed: SuppressionRule =
            toml::from_str("path = \"vendor/**\"\nreason = \"third-party\"").unwrap();
        assert_eq!(parsed.rule, "*");
        assert_eq!(parsed.path.as_deref(), Some("vendor/**"));
    }
}
