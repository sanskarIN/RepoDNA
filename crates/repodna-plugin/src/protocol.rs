//! Plugin protocol version 1.
//!
//! RepoDNA writes one JSON [`Request`] to the analyzer's standard input and reads one JSON
//! response from its standard output:
//!
//! ```json
//! {
//!   "api": 1,
//!   "findings": [{
//!     "rule": "missing-header",
//!     "severity": "info",
//!     "confidence": "high",
//!     "title": "3 source files have no SPDX license identifier",
//!     "summary": "…",
//!     "paths": ["src/a.rs"],
//!     "evidence": [{"kind": "file", "path": "src/a.rs", "line": 1}],
//!     "nextSteps": ["Add an SPDX-License-Identifier comment."]
//!   }],
//!   "metrics": [{"id": "coverage", "label": "Files with a header", "value": 0.8, "unit": "ratio"}],
//!   "notes": ["Checked 120 files."]
//! }
//! ```
//!
//! Everything in a response is validated: rule and metric identifiers are namespaced under
//! `plugin.<name>.`, paths must be repository-relative, text is stripped of control
//! characters and shortened, and counts are capped.

use repodna_core::Confidence;
use repodna_core::evidence::Evidence;
use repodna_core::finding::{Finding, FindingCategory};
use repodna_core::metric::Metric;
use repodna_core::model::artifact::RepositoryDna;
use repodna_core::paths::normalize_relative;
use repodna_core::severity::Severity;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::manifest::{PLUGIN_API, Permission, PluginManifest};

/// Most files listed in a request.
pub const MAX_REQUEST_FILES: usize = 50_000;
/// Most findings accepted from one plugin.
pub const MAX_FINDINGS: usize = 1_000;
/// Most metrics accepted from one plugin.
pub const MAX_METRICS: usize = 200;
/// Most paths and evidence items kept per finding.
const MAX_REFERENCES: usize = 50;
/// Most notes kept.
const MAX_NOTES: usize = 20;

/// The request written to the analyzer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    /// Protocol version.
    pub api: u32,
    /// RepoDNA version.
    pub repodna_version: String,
    /// The plugin being run.
    pub plugin: PluginInfo,
    /// The repository.
    pub repository: RepositoryInfo,
    /// Files discovered by the analysis (paths are repository-relative).
    pub files: Vec<FileInfo>,
    /// `true` when the file list was cut at [`MAX_REQUEST_FILES`].
    pub files_truncated: bool,
}

/// Plugin identity in a request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginInfo {
    /// Name.
    pub name: String,
    /// Version.
    pub version: String,
}

/// Repository facts in a request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryInfo {
    /// Repository name.
    pub name: String,
    /// Absolute checkout path; present only with the `repository-files` permission.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// Commit analyzed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Primary languages.
    pub primary_languages: Vec<String>,
}

/// One file in a request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    /// Repository-relative path.
    pub path: String,
    /// Language identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Classification, e.g. `source` or `test`.
    pub category: String,
    /// Size in bytes.
    pub bytes: u64,
    /// Lines of code, when counted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_lines: Option<u64>,
    /// `true` for generated files.
    pub generated: bool,
    /// `true` for vendored files.
    pub vendored: bool,
    /// `true` for binary files.
    pub binary: bool,
}

/// Builds the request for `manifest`.
pub fn build_request(manifest: &PluginManifest, dna: &RepositoryDna, root: &str) -> Request {
    let files: Vec<FileInfo> = dna
        .structure
        .files
        .iter()
        .take(MAX_REQUEST_FILES)
        .map(|file| FileInfo {
            path: file.path.clone(),
            language: file.language.clone(),
            category: serde_json::to_value(file.category)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default(),
            bytes: file.bytes,
            code_lines: file.lines.as_ref().map(|lines| lines.code),
            generated: file.generated,
            vendored: file.vendored,
            binary: file.binary,
        })
        .collect();
    Request {
        api: PLUGIN_API,
        repodna_version: env!("CARGO_PKG_VERSION").to_owned(),
        plugin: PluginInfo {
            name: manifest.name.clone(),
            version: manifest.version.clone(),
        },
        repository: RepositoryInfo {
            name: dna.identity.name.clone(),
            root: manifest
                .has(Permission::RepositoryFiles)
                .then(|| root.to_owned()),
            revision: dna.analysis_metadata.revision.clone(),
            primary_languages: dna.languages.primary.clone(),
        },
        files_truncated: dna.structure.files.len() > MAX_REQUEST_FILES,
        files,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawResponse {
    api: u32,
    #[serde(default)]
    findings: Vec<Value>,
    #[serde(default)]
    metrics: Vec<Value>,
    #[serde(default)]
    notes: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFinding {
    rule: String,
    #[serde(default = "default_severity")]
    severity: Severity,
    #[serde(default = "default_confidence")]
    confidence: Confidence,
    title: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    rationale: String,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    evidence: Vec<Value>,
    #[serde(default)]
    next_steps: Vec<String>,
    #[serde(default)]
    limitations: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMetric {
    id: String,
    #[serde(default)]
    label: String,
    value: f64,
    #[serde(default)]
    unit: String,
    #[serde(default)]
    definition: String,
}

fn default_severity() -> Severity {
    Severity::Info
}

fn default_confidence() -> Confidence {
    Confidence::Medium
}

/// A validated response.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Contribution {
    /// Findings, rules prefixed with `plugin.<name>.`.
    pub findings: Vec<Finding>,
    /// Metrics, identifiers prefixed with `plugin.<name>.`.
    pub metrics: Vec<Metric>,
    /// Notes from the plugin.
    pub notes: Vec<String>,
    /// Items that were dropped and why.
    pub rejected: Vec<String>,
}

/// Removes control characters, folds whitespace, and limits the length.
pub(crate) fn clean(text: &str, max_chars: usize) -> String {
    let folded: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if folded.chars().count() > max_chars {
        let mut cut: String = folded.chars().take(max_chars).collect();
        cut.push('…');
        cut
    } else {
        folded
    }
}

/// Returns `true` for identifiers like `missing-header` or `docs.coverage`.
fn valid_identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
        && id.as_bytes()[0].is_ascii_alphanumeric()
}

fn relative(path: &str) -> Option<String> {
    normalize_relative(path).filter(|path| !path.is_empty())
}

/// Validates one evidence item: paths must be repository-relative, and text is cleaned.
fn clean_evidence(value: Value) -> Option<Evidence> {
    let evidence: Evidence = serde_json::from_value(value).ok()?;
    let note = |note: Option<String>| note.map(|text| clean(&text, 300));
    Some(match evidence {
        Evidence::File {
            path,
            line,
            end_line,
            note: text,
        } => Evidence::File {
            path: relative(&path)?,
            line,
            end_line,
            note: note(text),
        },
        Evidence::Directory { path, note: text } => Evidence::Directory {
            path: normalize_relative(&path)?,
            note: note(text),
        },
        Evidence::Symbol { path, name, line } => Evidence::Symbol {
            path: relative(&path)?,
            name: clean(&name, 200),
            line,
        },
        Evidence::Commit {
            hash,
            date,
            summary,
        } => {
            if hash.is_empty() || hash.len() > 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            Evidence::Commit {
                hash,
                date,
                summary: note(summary),
            }
        }
        Evidence::DependencyEdge {
            from,
            to,
            path,
            line,
        } => Evidence::DependencyEdge {
            from: clean(&from, 200),
            to: clean(&to, 200),
            path: match path {
                Some(path) => Some(relative(&path)?),
                None => None,
            },
            line,
        },
        Evidence::Package {
            ecosystem,
            name,
            version,
            manifest,
        } => Evidence::Package {
            ecosystem: clean(&ecosystem, 40),
            name: clean(&name, 200),
            version: version.map(|v| clean(&v, 80)),
            manifest: match manifest {
                Some(path) => Some(relative(&path)?),
                None => None,
            },
        },
        Evidence::Metric {
            metric,
            value,
            threshold,
            unit,
        } => Evidence::Metric {
            metric: clean(&metric, 120),
            value,
            threshold,
            unit: unit.map(|u| clean(&u, 40)),
        },
        Evidence::Observation { text } => Evidence::Observation {
            text: clean(&text, 500),
        },
    })
}

fn texts(items: Vec<String>, max_chars: usize) -> Vec<String> {
    items
        .into_iter()
        .map(|text| clean(&text, max_chars))
        .filter(|text| !text.is_empty())
        .take(MAX_REFERENCES)
        .collect()
}

fn finding(plugin: &str, raw: RawFinding) -> Result<Finding, String> {
    let rule = raw.rule.trim();
    if !valid_identifier(rule) {
        return Err(format!(
            "finding rule `{}` is not a valid identifier",
            clean(rule, 80)
        ));
    }
    let title = clean(&raw.title, 200);
    if title.is_empty() {
        return Err(format!("finding `{rule}` has no title"));
    }
    let paths: Vec<String> = raw
        .paths
        .iter()
        .filter_map(|path| relative(path))
        .take(MAX_REFERENCES)
        .collect();
    let subject = paths.first().cloned().unwrap_or_else(|| title.clone());
    let mut result = Finding::new(
        format!("plugin.{plugin}.{rule}"),
        &subject,
        FindingCategory::Plugin,
        raw.severity,
        raw.confidence,
        title,
    )
    .summary(clean(&raw.summary, 2_000))
    .rationale(clean(&raw.rationale, 2_000))
    .method(format!("Reported by the `{plugin}` plugin."))
    .with_evidence(
        raw.evidence
            .into_iter()
            .filter_map(clean_evidence)
            .take(MAX_REFERENCES),
    );
    for path in paths {
        result = result.path(path);
    }
    for step in texts(raw.next_steps, 500) {
        result = result.next_step(step);
    }
    for limitation in texts(raw.limitations, 500) {
        result = result.limitation(limitation);
    }
    Ok(result)
}

fn metric(plugin: &str, raw: RawMetric) -> Result<Metric, String> {
    let id = raw.id.trim();
    if !valid_identifier(id) {
        return Err(format!(
            "metric `{}` is not a valid identifier",
            clean(id, 80)
        ));
    }
    if !raw.value.is_finite() {
        return Err(format!("metric `{id}` is not a finite number"));
    }
    let label = if raw.label.trim().is_empty() {
        id.to_owned()
    } else {
        clean(&raw.label, 200)
    };
    Ok(Metric::new(
        format!("plugin.{plugin}.{id}"),
        label,
        raw.value,
        clean(&raw.unit, 40),
    )
    .definition(clean(&raw.definition, 1_000))
    .method(format!("Reported by the `{plugin}` plugin.")))
}

/// Parses and validates the analyzer's output.
pub fn parse_response(plugin: &str, output: &str) -> Result<Contribution, String> {
    let raw: RawResponse = serde_json::from_str(output.trim())
        .map_err(|error| format!("output is not a valid response: {error}"))?;
    if raw.api != PLUGIN_API {
        return Err(format!(
            "response api {} does not match plugin api {PLUGIN_API}",
            raw.api
        ));
    }
    let mut contribution = Contribution::default();
    let (findings, metrics) = (raw.findings.len(), raw.metrics.len());
    for value in raw.findings.into_iter().take(MAX_FINDINGS) {
        match serde_json::from_value::<RawFinding>(value)
            .map_err(|error| format!("invalid finding: {error}"))
            .and_then(|raw| finding(plugin, raw))
        {
            Ok(finding) => contribution.findings.push(finding),
            Err(reason) => contribution.rejected.push(reason),
        }
    }
    if findings > MAX_FINDINGS {
        contribution.rejected.push(format!(
            "{} findings beyond the limit of {MAX_FINDINGS} were dropped",
            findings - MAX_FINDINGS
        ));
    }
    for value in raw.metrics.into_iter().take(MAX_METRICS) {
        match serde_json::from_value::<RawMetric>(value)
            .map_err(|error| format!("invalid metric: {error}"))
            .and_then(|raw| metric(plugin, raw))
        {
            Ok(metric) => contribution.metrics.push(metric),
            Err(reason) => contribution.rejected.push(reason),
        }
    }
    if metrics > MAX_METRICS {
        contribution.rejected.push(format!(
            "{} metrics beyond the limit of {MAX_METRICS} were dropped",
            metrics - MAX_METRICS
        ));
    }
    contribution.notes = raw
        .notes
        .iter()
        .filter_map(Value::as_str)
        .map(|note| clean(note, 500))
        .filter(|note| !note.is_empty())
        .take(MAX_NOTES)
        .collect();
    Ok(contribution)
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::metadata::AnalysisMetadata;
    use repodna_core::model::structure::{FileCategory, FileRecord};

    fn manifest(permissions: Vec<Permission>) -> PluginManifest {
        PluginManifest {
            name: "demo".into(),
            version: "1.0.0".into(),
            description: String::new(),
            api: PLUGIN_API,
            command: vec!["demo".into()],
            permissions,
            languages: Vec::new(),
            homepage: None,
            license: None,
        }
    }

    #[test]
    fn builds_requests_that_respect_permissions() {
        let mut dna = RepositoryDna::new(
            RepositoryIdentity {
                name: "widget".into(),
                ..RepositoryIdentity::default()
            },
            AnalysisMetadata::default(),
        );
        dna.structure.files.push(FileRecord {
            path: "src/lib.rs".into(),
            category: FileCategory::Source,
            language: Some("rust".into()),
            bytes: 10,
            lines: None,
            binary: false,
            generated: false,
            vendored: false,
            hash: None,
            module: None,
            analysis: None,
            skipped: None,
        });
        let without = build_request(&manifest(Vec::new()), &dna, "/checkout");
        assert_eq!(without.repository.root, None);
        assert_eq!(without.files[0].category, "source");
        let json = serde_json::to_string(&without).unwrap();
        assert!(!json.contains("/checkout"));
        assert!(json.contains("\"repodnaVersion\""));
        let with = build_request(
            &manifest(vec![Permission::RepositoryFiles]),
            &dna,
            "/checkout",
        );
        assert_eq!(with.repository.root.as_deref(), Some("/checkout"));
    }

    #[test]
    fn validates_responses() {
        let output = r#"{
          "api": 1,
          "findings": [
            {"rule": "missing-header", "severity": "attention", "confidence": "high",
             "title": "Missing\u001b[2J header", "summary": "Two files.",
             "paths": ["src/a.rs", "../../etc/passwd", "/abs"],
             "evidence": [{"kind": "file", "path": "src/a.rs", "line": 1},
                          {"kind": "file", "path": "../x"},
                          {"kind": "commit", "hash": "not-a-hash"},
                          {"kind": "observation", "text": "fine"}],
             "nextSteps": ["Add a header."]},
            {"rule": "Bad Rule", "title": "x"},
            {"rule": "no-title", "title": "  "},
            {"title": "missing rule"}
          ],
          "metrics": [
            {"id": "coverage", "label": "Files with a header", "value": 0.5, "unit": "ratio"},
            {"id": "BAD", "value": 1}
          ],
          "notes": ["Checked 2 files.", 7]
        }"#;
        let contribution = parse_response("demo", output).unwrap();
        assert_eq!(contribution.findings.len(), 1);
        let finding = &contribution.findings[0];
        assert_eq!(finding.rule, "plugin.demo.missing-header");
        assert_eq!(finding.category, FindingCategory::Plugin);
        assert_eq!(finding.severity, Severity::Attention);
        assert_eq!(finding.title, "Missing [2J header");
        assert_eq!(finding.paths, vec!["src/a.rs"]);
        assert_eq!(finding.evidence.len(), 2);
        assert_eq!(finding.next_steps, vec!["Add a header."]);
        assert_eq!(contribution.metrics.len(), 1);
        assert_eq!(contribution.metrics[0].id, "plugin.demo.coverage");
        assert_eq!(contribution.notes, vec!["Checked 2 files."]);
        assert_eq!(
            contribution.rejected.len(),
            4,
            "{:?}",
            contribution.rejected
        );

        assert!(parse_response("demo", "not json").is_err());
        assert!(
            parse_response("demo", r#"{"api": 2}"#)
                .unwrap_err()
                .contains("api 2")
        );
    }
}
