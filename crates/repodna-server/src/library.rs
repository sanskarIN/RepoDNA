//! Stored analyses and generated reports in the shapes the web interface reads.
//!
//! The HTTP API and the desktop app answer with the same JSON, so the interface behaves the
//! same in a browser and in the desktop app.

use std::fmt;

use repodna_core::config::{PrivacyPreset, ReportTheme};
use repodna_core::model::artifact::RepositoryDna;
use repodna_report::{ReportOptions, apply_privacy, html_report, json_report, markdown_report};
use repodna_store::{RepositoryRecord, ScanRecord, Store, StoreError};
use serde_json::{Value, json};

/// Why a stored analysis could not be loaded.
#[derive(Debug)]
pub enum LookupError {
    /// No stored repository matches the query.
    NoRepository(String),
    /// The repository has no analysis with the requested identifier.
    NoScan,
    /// Local storage failed.
    Storage(StoreError),
}

impl fmt::Display for LookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRepository(query) => write!(f, "no stored repository matches `{query}`"),
            Self::NoScan => f.write_str("no stored analysis matches"),
            Self::Storage(error) => write!(f, "local storage failed: {error}"),
        }
    }
}

impl std::error::Error for LookupError {}

impl From<StoreError> for LookupError {
    fn from(error: StoreError) -> Self {
        Self::Storage(error)
    }
}

/// A stored repository as the interface lists it.
pub fn repository_json(repository: &RepositoryRecord) -> Value {
    json!({
        "id": repository.id,
        "name": repository.name,
        "location": repository.location,
        "kind": repository.kind,
        "scans": repository.scans,
        "firstScannedAt": repository.first_scanned_at,
        "lastScannedAt": repository.last_scanned_at,
    })
}

/// A stored analysis as the interface lists it.
pub fn scan_json(scan: &ScanRecord) -> Value {
    json!({
        "id": scan.id,
        "generatedAt": scan.generated_at,
        "profile": scan.profile,
        "revision": scan.revision,
        "dnaHash": scan.dna_hash,
        "toolVersion": scan.tool_version,
        "files": scan.files,
        "codeLines": scan.code_lines,
        "commits": scan.commits,
        "contributors": scan.contributors,
        "findings": {
            "critical": scan.critical,
            "warning": scan.warning,
            "attention": scan.attention,
            "info": scan.info,
            "suppressed": scan.suppressed,
        },
    })
}

/// Every stored repository.
pub fn repositories(store: &Store) -> Result<Value, LookupError> {
    Ok(json!(
        store
            .repositories()?
            .iter()
            .map(repository_json)
            .collect::<Vec<_>>()
    ))
}

/// A stored repository with its most recent analyses (up to 100).
pub fn repository_detail(store: &Store, query: &str) -> Result<Value, LookupError> {
    let repository = find_repository(store, query)?;
    let scans = store.scans(&repository.id, 100)?;
    Ok(json!({
        "repository": repository_json(&repository),
        "scans": scans.iter().map(scan_json).collect::<Vec<_>>(),
    }))
}

/// Finds a stored repository by identifier, name, or location.
pub fn find_repository(store: &Store, query: &str) -> Result<RepositoryRecord, LookupError> {
    store
        .find_repository(query)?
        .ok_or_else(|| LookupError::NoRepository(query.to_owned()))
}

/// The analysis `scan` of a repository, or its latest one.
pub fn find_scan(
    store: &Store,
    repository: &RepositoryRecord,
    scan: Option<&str>,
) -> Result<ScanRecord, LookupError> {
    let found = match scan {
        Some(id) => store
            .scan(id)?
            .filter(|scan| scan.repository_id == repository.id),
        None => store.scans(&repository.id, 1)?.into_iter().next(),
    };
    found.ok_or(LookupError::NoScan)
}

/// Loads a stored analysis with a privacy preset applied.
pub fn load(
    store: &Store,
    query: &str,
    scan: Option<&str>,
    preset: PrivacyPreset,
) -> Result<RepositoryDna, LookupError> {
    let repository = find_repository(store, query)?;
    let scan = find_scan(store, &repository, scan)?;
    let dna = store.load(&scan)?;
    Ok(apply_privacy(&dna, preset))
}

/// Parses a privacy preset name (`local`, `share`, or `public`).
pub fn parse_privacy(name: &str) -> Result<PrivacyPreset, String> {
    match name {
        "local" => Ok(PrivacyPreset::Local),
        "share" => Ok(PrivacyPreset::Share),
        "public" => Ok(PrivacyPreset::Public),
        other => Err(format!("unknown privacy preset `{other}`")),
    }
}

/// Parses a report theme name.
pub fn parse_theme(name: &str) -> Result<ReportTheme, String> {
    match name {
        "professional" => Ok(ReportTheme::Professional),
        "minimal" => Ok(ReportTheme::Minimal),
        "technical" => Ok(ReportTheme::Technical),
        "dark" => Ok(ReportTheme::Dark),
        other => Err(format!("unknown theme `{other}`")),
    }
}

/// A generated report, ready to serve or save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// Media type for HTTP responses.
    pub content_type: &'static str,
    /// File extension without the dot.
    pub extension: &'static str,
    /// The report.
    pub text: String,
}

/// Renders a report of an analysis that already has `preset` applied, in `format`
/// (`html`, `markdown`, or `json`).
pub fn report(
    dna: &RepositoryDna,
    format: &str,
    theme: ReportTheme,
    preset: PrivacyPreset,
) -> Result<Document, String> {
    let options = ReportOptions {
        theme,
        privacy: preset,
        ..ReportOptions::default()
    };
    match format {
        "html" => Ok(Document {
            content_type: "text/html; charset=utf-8",
            extension: "html",
            text: html_report(dna, &options),
        }),
        "markdown" => Ok(Document {
            content_type: "text/markdown; charset=utf-8",
            extension: "md",
            text: markdown_report(dna, &options),
        }),
        "json" => json_report(dna, preset, true)
            .map(|text| Document {
                content_type: "application/json; charset=utf-8",
                extension: "json",
                text,
            })
            .map_err(|error| error.to_string()),
        other => Err(format!("unknown report format `{other}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::metadata::AnalysisMetadata;

    #[test]
    fn names_parse_and_unknown_ones_are_explained() {
        assert_eq!(parse_privacy("share"), Ok(PrivacyPreset::Share));
        assert_eq!(parse_theme("dark"), Ok(ReportTheme::Dark));
        assert_eq!(
            parse_privacy("secret"),
            Err("unknown privacy preset `secret`".to_owned())
        );
        assert_eq!(parse_theme("neon"), Err("unknown theme `neon`".to_owned()));
        assert_eq!(
            LookupError::NoRepository("demo".into()).to_string(),
            "no stored repository matches `demo`"
        );
    }

    #[test]
    fn reports_render_in_every_format() {
        let dna = RepositoryDna::new(RepositoryIdentity::default(), AnalysisMetadata::default());
        for (format, extension) in [("html", "html"), ("markdown", "md"), ("json", "json")] {
            let document = report(&dna, format, ReportTheme::Minimal, PrivacyPreset::Share)
                .unwrap_or_else(|error| panic!("{format}: {error}"));
            assert_eq!(document.extension, extension);
            assert!(!document.text.is_empty());
        }
        assert!(report(&dna, "pdf", ReportTheme::Minimal, PrivacyPreset::Local).is_err());
    }
}
