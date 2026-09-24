//! The security report, permission signals, and security findings.

use std::collections::HashMap;

use repodna_core::confidence::Confidence;
use repodna_core::evidence::Evidence;
use repodna_core::finding::{Finding, FindingCategory};
use repodna_core::model::SectionStatus;
use repodna_core::model::dependencies::AdvisoryStatus;
use repodna_core::model::security::{
    PatternCandidate, PermissionSignal, SecretCandidate, SecurityReport, SecurityRuleInfo,
};
use repodna_core::severity::Severity;

use crate::patterns::PATTERN_RULES;
use crate::secrets::SECRET_RULES;

/// Maximum secret or pattern candidates kept in the report.
pub const MAX_CANDIDATES: usize = 500;

/// Maximum findings per rule family.
pub const MAX_FINDINGS: usize = 50;

/// What the security signals do and do not mean.
pub const DISCLAIMER: &str = "These are defensive signals from static pattern matching. A clean \
result does not mean the code is secure, and a match does not prove a vulnerability. Secret \
values are never stored: only their location and a keyed fingerprint are recorded. No \
vulnerability advisory database was consulted.";

/// Reports unusual permission bits (Unix modes) of repository files.
pub fn permission_signals<'a>(
    files: impl IntoIterator<Item = (&'a str, u32)>,
) -> Vec<PermissionSignal> {
    let mut signals = Vec::new();
    for (path, mode) in files {
        let mut issues = Vec::new();
        if mode & 0o002 != 0 {
            issues.push("writable by every user on the system");
        }
        if mode & 0o4000 != 0 {
            issues.push("runs with the owner's privileges (setuid)");
        }
        if mode & 0o2000 != 0 {
            issues.push("runs with the group's privileges (setgid)");
        }
        if !issues.is_empty() {
            signals.push(PermissionSignal {
                path: path.to_owned(),
                mode: format!("{:04o}", mode & 0o7777),
                issue: issues.join("; "),
            });
        }
    }
    signals.sort_by(|a, b| a.path.cmp(&b.path));
    signals
}

/// Every rule the scanner applies, for the report's rule list.
pub fn rule_catalog() -> Vec<SecurityRuleInfo> {
    let mut rules: Vec<SecurityRuleInfo> = SECRET_RULES
        .iter()
        .map(|rule| SecurityRuleInfo {
            id: rule.id.to_owned(),
            kind: "secret".to_owned(),
            description: rule.description.to_owned(),
        })
        .collect();
    rules.extend(PATTERN_RULES.iter().map(|rule| SecurityRuleInfo {
        id: rule.id.to_owned(),
        kind: "pattern".to_owned(),
        description: rule.description.to_owned(),
    }));
    for (id, description) in [
        (
            "workflow-script-injection",
            "Text controlled by issue or pull request authors is interpolated into a shell command",
        ),
        (
            "env-file-committed",
            "An environment file with values is committed",
        ),
    ] {
        rules.push(SecurityRuleInfo {
            id: id.to_owned(),
            kind: "pattern".to_owned(),
            description: description.to_owned(),
        });
    }
    rules
}

/// Assembles the security section from per-file scan results.
pub fn build_report(
    mut secrets: Vec<SecretCandidate>,
    mut patterns: Vec<PatternCandidate>,
    permissions: Vec<PermissionSignal>,
    files_scanned: u64,
) -> SecurityReport {
    let mut notes = Vec::new();
    secrets.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.line.cmp(&b.line))
            .then_with(|| a.rule.cmp(&b.rule))
    });
    patterns.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.line.cmp(&b.line))
            .then_with(|| a.rule.cmp(&b.rule))
    });
    for (label, count) in [("secret", secrets.len()), ("pattern", patterns.len())] {
        if count > MAX_CANDIDATES {
            notes.push(format!(
                "{count} {label} candidates were found; the report keeps the first {MAX_CANDIDATES} in path order."
            ));
        }
    }
    secrets.truncate(MAX_CANDIDATES);
    patterns.truncate(MAX_CANDIDATES);
    SecurityReport {
        status: SectionStatus::Analyzed,
        notes,
        secrets,
        patterns,
        permissions,
        files_scanned,
        rules: rule_catalog(),
        advisories: AdvisoryStatus {
            provider: None,
            checked_at: None,
            note: "No vulnerability advisory source was consulted; dependency versions are listed without security claims.".to_owned(),
        },
        disclaimer: DISCLAIMER.to_owned(),
    }
}

fn secret_severity(candidate: &SecretCandidate) -> Severity {
    if candidate.in_test_or_example || candidate.confidence < Confidence::Medium {
        Severity::Attention
    } else if candidate.rule == "private-key" && candidate.confidence == Confidence::High {
        Severity::Critical
    } else {
        Severity::Warning
    }
}

fn pattern_severity(candidate: &PatternCandidate) -> Severity {
    match candidate.confidence {
        Confidence::High => Severity::Warning,
        Confidence::Medium => Severity::Attention,
        _ => Severity::Info,
    }
}

/// Derives findings from the security section.
pub fn security_findings(report: &SecurityReport) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut fingerprint_counts: HashMap<&str, usize> = HashMap::new();
    for candidate in &report.secrets {
        *fingerprint_counts
            .entry(candidate.fingerprint.as_str())
            .or_default() += 1;
    }
    let mut secrets: Vec<&SecretCandidate> = report.secrets.iter().collect();
    secrets.sort_by(|a, b| {
        secret_severity(b)
            .cmp(&secret_severity(a))
            .then_with(|| b.confidence.cmp(&a.confidence))
            .then_with(|| a.path.cmp(&b.path))
            .then(a.line.cmp(&b.line))
    });
    for candidate in secrets.into_iter().take(MAX_FINDINGS) {
        let repeats = fingerprint_counts
            .get(candidate.fingerprint.as_str())
            .copied()
            .unwrap_or(1);
        let mut summary = format!(
            "Line {} matches the {} rule. The value is not stored; its fingerprint is {}.",
            candidate.line, candidate.rule, candidate.fingerprint
        );
        if repeats > 1 {
            summary.push_str(&format!(
                " The same value appears in {repeats} places in this scan."
            ));
        }
        if candidate.in_test_or_example {
            summary.push_str(" The path looks like a test, fixture, example, or document.");
        }
        findings.push(
            Finding::new(
                "security.secret",
                &candidate.id,
                FindingCategory::Security,
                secret_severity(candidate),
                candidate.confidence,
                format!("Possible {} in {}", candidate.description.to_lowercase(), candidate.path),
            )
            .summary(summary)
            .rationale("Credentials committed to a repository are exposed to everyone with access to it and to its history, including forks and clones.")
            .method("Line-based pattern matching with keyword prefilters, placeholder exclusions, and entropy checks; values are fingerprinted with a keyed HMAC and never stored.")
            .evidence(Evidence::line(&candidate.path, candidate.line).with_note("value redacted"))
            .evidence(Evidence::observation(format!(
                "Fingerprint {} (equal fingerprints mean equal values within this scan)",
                candidate.fingerprint
            )))
            .limitation("Pattern matching cannot tell whether a credential is valid, revoked, or a placeholder.")
            .next_step("If the credential is real, revoke and rotate it first, then remove it from the code and the history, and load it from a secret store or environment variable.")
            .path(candidate.path.clone()),
        );
    }

    let mut patterns: Vec<&PatternCandidate> = report.patterns.iter().collect();
    patterns.sort_by(|a, b| {
        pattern_severity(b)
            .cmp(&pattern_severity(a))
            .then_with(|| a.path.cmp(&b.path))
            .then(a.line.cmp(&b.line))
    });
    for candidate in patterns.into_iter().take(MAX_FINDINGS) {
        let location = match candidate.line {
            Some(line) => Evidence::line(&candidate.path, line),
            None => Evidence::file(&candidate.path),
        };
        findings.push(
            Finding::new(
                format!("security.{}", candidate.rule),
                &candidate.id,
                FindingCategory::Security,
                pattern_severity(candidate),
                candidate.confidence,
                format!("{} in {}", candidate.description, candidate.path),
            )
            .summary(candidate.description.clone())
            .rationale("The construct is a common source of security problems when it reaches production or handles untrusted input.")
            .method("Pattern matching on comment-stripped lines, limited to the file types where the construct is meaningful.")
            .evidence(location)
            .limitation("The scanner does not know whether the code path is reachable, only used in development, or already mitigated elsewhere.")
            .next_step(candidate.recommendation.clone())
            .path(candidate.path.clone()),
        );
    }

    for signal in report.permissions.iter().take(MAX_FINDINGS) {
        findings.push(
            Finding::new(
                "security.permission",
                &signal.path,
                FindingCategory::Security,
                Severity::Attention,
                Confidence::High,
                format!("{} has mode {}", signal.path, signal.mode),
            )
            .summary(format!("The file is {}.", signal.issue))
            .rationale("Unusual permission bits committed to a repository are copied to every checkout on systems that honor them.")
            .method("File mode bits as recorded on disk (Unix-like systems only).")
            .evidence(Evidence::file(&signal.path).with_note(format!("mode {}", signal.mode)))
            .next_step("Remove the extra permission bits unless the file really needs them.")
            .path(signal.path.clone()),
        );
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SecretScanner, scan_patterns};
    use repodna_parser::LanguageRegistry;

    #[test]
    fn reports_unusual_permissions() {
        let signals = permission_signals([
            ("bin/tool", 0o100_755),
            ("tmp/shared.txt", 0o100_666),
            ("bin/su", 0o104_755),
        ]);
        let summary: Vec<(&str, &str)> = signals
            .iter()
            .map(|s| (s.path.as_str(), s.mode.as_str()))
            .collect();
        assert_eq!(
            summary,
            vec![("bin/su", "4755"), ("tmp/shared.txt", "0666")]
        );
        assert!(signals[0].issue.contains("setuid"));
    }

    #[test]
    fn builds_report_and_findings_with_criteria_based_severity() {
        let scanner = SecretScanner::new([1; 32]);
        let key = [
            "-----BEGIN ",
            "PRIVATE KEY-----\nMIIBVQIBADANBg\n-----END ",
            "PRIVATE KEY-----\n",
        ]
        .concat();
        let token = ["gh", "p_", &"Zq7Xw2Ve9Rt4".repeat(3)].concat();
        let mut secrets = scanner.scan("deploy/id_rsa", &key);
        secrets.extend(scanner.scan("src/client.py", &format!("TOKEN = \"{token}\"\n")));
        secrets.extend(scanner.scan("tests/test_client.py", &format!("T = \"{token}\"\n")));
        let patterns = scan_patterns(
            "src/net.py",
            "requests.get(u, verify=False)\n",
            LanguageRegistry::builtin().get("python"),
        );
        let report = build_report(
            secrets,
            patterns,
            permission_signals([("run.sh", 0o100_777)]),
            4,
        );
        assert_eq!(report.status, SectionStatus::Analyzed);
        assert_eq!(report.secrets.len(), 3);
        assert_eq!(report.files_scanned, 4);
        assert!(report.rules.iter().any(|rule| rule.id == "private-key"));
        assert!(report.disclaimer.contains("never stored"));

        let findings = security_findings(&report);
        let summary: Vec<(&str, Severity)> = findings
            .iter()
            .map(|f| (f.rule.as_str(), f.severity))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("security.secret", Severity::Critical),
                ("security.secret", Severity::Warning),
                ("security.secret", Severity::Attention),
                ("security.tls-verification-disabled", Severity::Attention),
                ("security.permission", Severity::Attention),
            ]
        );
        assert!(
            findings[1]
                .summary
                .contains("same value appears in 2 places")
        );
        let text = format!("{findings:?}");
        assert!(!text.contains(&token));
    }
}
