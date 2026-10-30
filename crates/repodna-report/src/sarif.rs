//! Offline SARIF 2.1.0 export from evidence-backed RepoDNA findings.
//! Repository-wide observations are not assigned invented file locations.

use std::collections::{BTreeMap, HashSet};

use repodna_core::evidence::Evidence;
use repodna_core::finding::Finding;
use repodna_core::model::artifact::RepositoryDna;
use repodna_core::severity::Severity;

use crate::ci::CiFindingChange;

fn safe_uri(path: &str) -> Option<String> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return None;
    }
    let mut uri = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'.' | b'_' | b'~') {
            uri.push(char::from(byte));
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    Some(uri)
}

fn source_location(finding: &Finding) -> Option<serde_json::Value> {
    let evidence = finding.evidence.iter().find_map(|entry| {
        let (path, line) = match entry {
            Evidence::File { path, line, .. }
            | Evidence::Symbol { path, line, .. }
            | Evidence::DependencyEdge {
                path: Some(path),
                line,
                ..
            } => (path.as_str(), *line),
            _ => return None,
        };
        safe_uri(path).map(|uri| (uri, line))
    });
    let (uri, line) = evidence.or_else(|| {
        finding
            .paths
            .iter()
            .find_map(|path| safe_uri(path).map(|uri| (uri, None)))
    })?;
    let mut physical = serde_json::json!({
        "artifactLocation": { "uri": uri }
    });
    if let Some(line) = line.filter(|line| *line > 0) {
        physical["region"] = serde_json::json!({"startLine": line});
    }
    Some(serde_json::json!({"physicalLocation": physical}))
}

fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "error",
        Severity::Warning => "warning",
        Severity::Attention | Severity::Info => "note",
    }
}

/// Creates a deterministic SARIF 2.1.0 run from active, source-located findings.
///
/// An introduced scope of Some(empty) emits no results. No repository file
/// contents, absolute paths, or invented source lines are included.
pub fn sarif(dna: &RepositoryDna, introduced: Option<&[CiFindingChange]>) -> serde_json::Value {
    let allowed = introduced.map(|changes| {
        changes
            .iter()
            .map(|finding| finding.id.as_str())
            .collect::<HashSet<_>>()
    });
    let mut findings: Vec<_> = dna
        .findings
        .iter()
        .filter(|f| {
            !f.is_suppressed()
                && !f.rule.is_empty()
                && allowed
                    .as_ref()
                    .is_none_or(|ids| ids.contains(f.id.as_str()))
        })
        .collect();
    findings.sort_by(|a, b| b.severity.cmp(&a.severity).then_with(|| a.id.cmp(&b.id)));
    let mut seen = HashSet::new();
    let mut rules = BTreeMap::new();
    let mut results = Vec::new();
    for finding in findings {
        if !seen.insert(finding.id.as_str()) {
            continue;
        }
        let Some(location) = source_location(finding) else {
            continue;
        };
        rules.entry(finding.rule.as_str()).or_insert_with(|| {
            serde_json::json!({
                "id": finding.rule,
                "shortDescription": { "text": finding.rule },
            })
        });
        results.push(serde_json::json!({
            "ruleId": finding.rule,
            "level": sarif_level(finding.severity),
            "message": { "text": finding.title },
            "locations": [location],
            "partialFingerprints": { "repodnaStableFindingId": finding.id },
        }));
    }
    serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": "RepoDNA",
                "informationUri": "https://sanskarin.github.io/RepoDNA",
                "rules": rules.into_values().collect::<Vec<_>>()
            }},
            "results": results
        }]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::confidence::Confidence;
    use repodna_core::finding::{Finding, FindingCategory};
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::metadata::AnalysisMetadata;

    fn artifact(findings: Vec<Finding>) -> RepositoryDna {
        let mut dna = RepositoryDna::new(
            RepositoryIdentity {
                name: "demo".into(),
                ..RepositoryIdentity::default()
            },
            AnalysisMetadata::default(),
        );
        dna.findings = findings;
        dna
    }

    fn finding(subject: &str, severity: Severity) -> Finding {
        Finding::new(
            "architecture.cycle",
            subject,
            FindingCategory::Architecture,
            severity,
            Confidence::High,
            format!("Cycle {subject}"),
        )
    }

    #[test]
    fn ordered_results_and_no_invented_locations() {
        let critical =
            finding("critical", Severity::Critical).evidence(Evidence::line("src/main.rs", 17));
        let info = finding("info", Severity::Info).path("src/file name.rs");
        let unlocated = finding("unlocated", Severity::Warning);
        let dna = artifact(vec![info, unlocated, critical.clone(), critical]);
        let output = sarif(&dna, None);
        assert_eq!(output["version"], "2.1.0");
        assert_eq!(output["runs"][0]["tool"]["driver"]["name"], "RepoDNA");
        assert_eq!(
            output["runs"][0]["tool"]["driver"]["rules"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let results = output["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["level"], "error");
        assert_eq!(
            results[0]["locations"][0]["physicalLocation"]["region"]["startLine"],
            17
        );
        assert_eq!(results[1]["level"], "note");
        assert_eq!(
            results[1]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "src/file%20name.rs"
        );
        assert!(
            results[1]["locations"][0]["physicalLocation"]
                .get("region")
                .is_none()
        );
        assert_eq!(output, sarif(&dna, None));
    }

    #[test]
    fn filters_invalid_paths_suppressions_and_introduced_scope() {
        let active = finding("new", Severity::Warning).path("src/new.rs");
        let mut suppressed = finding("suppressed", Severity::Critical).path("src/s.rs");
        suppressed.suppressed = Some(repodna_core::finding::Suppression {
            reason: "accepted".into(),
            source: "repodna.toml".into(),
        });
        let dna = artifact(vec![
            active.clone(),
            suppressed,
            finding("traversal", Severity::Warning).path("../private.txt"),
            finding("absolute", Severity::Warning).path("/etc/passwd"),
            finding("windows", Severity::Warning).path("C:\\secret.txt"),
            finding("url", Severity::Warning).path("https://example.com/secret"),
        ]);
        assert_eq!(
            sarif(&dna, None)["runs"][0]["results"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let delta = vec![CiFindingChange {
            id: active.id.clone(),
            rule: active.rule.clone(),
            severity: active.severity,
            title: active.title.clone(),
            paths: active.paths.clone(),
        }];
        assert_eq!(
            sarif(&dna, Some(&delta))["runs"][0]["results"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            sarif(&dna, Some(&[]))["runs"][0]["results"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn falls_back_to_safe_path_when_evidence_is_absolute() {
        let unsafe_evidence = finding("evidence", Severity::Warning)
            .evidence(Evidence::line("/home/user/private.rs", 4))
            .path("src/safe.rs");
        let output = sarif(&artifact(vec![unsafe_evidence]), None);
        let loc = &output["runs"][0]["results"][0]["locations"][0]["physicalLocation"];
        assert_eq!(loc["artifactLocation"]["uri"], "src/safe.rs");
        assert!(loc.get("region").is_none());
    }
}
