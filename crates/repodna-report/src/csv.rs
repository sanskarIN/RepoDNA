//! CSV exports of the artifact's tabular data.
//!
//! Every field is quoted when needed and values that a spreadsheet would treat as a
//! formula are prefixed with an apostrophe, so opening an export cannot run anything.

use std::str::FromStr;

use repodna_core::model::artifact::RepositoryDna;

use crate::text::csv_field;

/// A table that can be exported as CSV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsvTable {
    /// Every discovered file.
    Files,
    /// Findings, including suppressed ones.
    Findings,
    /// Raw metrics.
    Metrics,
    /// Change hotspots.
    Hotspots,
    /// Declared dependencies.
    Dependencies,
    /// Contributor statistics.
    Contributors,
    /// Language composition.
    Languages,
}

impl CsvTable {
    /// Every table.
    pub const ALL: [CsvTable; 7] = [
        CsvTable::Files,
        CsvTable::Findings,
        CsvTable::Metrics,
        CsvTable::Hotspots,
        CsvTable::Dependencies,
        CsvTable::Contributors,
        CsvTable::Languages,
    ];

    /// Identifier used on the command line and as the file name stem.
    pub const fn id(self) -> &'static str {
        match self {
            CsvTable::Files => "files",
            CsvTable::Findings => "findings",
            CsvTable::Metrics => "metrics",
            CsvTable::Hotspots => "hotspots",
            CsvTable::Dependencies => "dependencies",
            CsvTable::Contributors => "contributors",
            CsvTable::Languages => "languages",
        }
    }
}

impl FromStr for CsvTable {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        CsvTable::ALL
            .into_iter()
            .find(|table| table.id() == value.trim().to_ascii_lowercase())
            .ok_or_else(|| {
                let valid: Vec<&str> = CsvTable::ALL.iter().map(|t| t.id()).collect();
                format!(
                    "unknown CSV table {value:?}; expected one of {}",
                    valid.join(", ")
                )
            })
    }
}

fn line(out: &mut String, fields: &[String]) {
    let cells: Vec<String> = fields.iter().map(|f| csv_field(f)).collect();
    out.push_str(&cells.join(","));
    out.push_str("\r\n");
}

fn header(out: &mut String, names: &[&str]) {
    line(
        out,
        &names.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>(),
    );
}

fn opt<T: ToString>(value: Option<T>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

/// Renders one table as CSV (RFC 4180 line endings).
pub fn csv(dna: &RepositoryDna, table: CsvTable) -> String {
    let mut out = String::new();
    match table {
        CsvTable::Files => {
            header(
                &mut out,
                &[
                    "path",
                    "category",
                    "language",
                    "bytes",
                    "lines",
                    "code_lines",
                    "comment_lines",
                    "blank_lines",
                    "module",
                    "generated",
                    "vendored",
                    "binary",
                ],
            );
            for file in &dna.structure.files {
                let lines = file.lines.unwrap_or_default();
                line(
                    &mut out,
                    &[
                        file.path.clone(),
                        file.category.label().to_owned(),
                        file.language.clone().unwrap_or_default(),
                        file.bytes.to_string(),
                        lines.total.to_string(),
                        lines.code.to_string(),
                        lines.comment.to_string(),
                        lines.blank.to_string(),
                        file.module.clone().unwrap_or_default(),
                        file.generated.to_string(),
                        file.vendored.to_string(),
                        file.binary.to_string(),
                    ],
                );
            }
        }
        CsvTable::Findings => {
            header(
                &mut out,
                &[
                    "id",
                    "rule",
                    "category",
                    "severity",
                    "confidence",
                    "title",
                    "summary",
                    "paths",
                    "suppressed",
                ],
            );
            for finding in &dna.findings {
                line(
                    &mut out,
                    &[
                        finding.id.clone(),
                        finding.rule.clone(),
                        format!("{:?}", finding.category).to_lowercase(),
                        finding.severity.label().to_owned(),
                        finding.confidence.label().to_owned(),
                        finding.title.clone(),
                        finding.summary.clone(),
                        finding.paths.join(";"),
                        finding.is_suppressed().to_string(),
                    ],
                );
            }
        }
        CsvTable::Metrics => {
            header(
                &mut out,
                &["id", "label", "value", "unit", "confidence", "definition"],
            );
            for metric in &dna.metrics.raw {
                line(
                    &mut out,
                    &[
                        metric.id.clone(),
                        metric.label.clone(),
                        metric.value.to_string(),
                        metric.unit.clone(),
                        metric.confidence.label().to_owned(),
                        metric.definition.clone(),
                    ],
                );
            }
        }
        CsvTable::Hotspots => {
            header(
                &mut out,
                &[
                    "rank",
                    "path",
                    "score",
                    "commits",
                    "authors",
                    "churn",
                    "recent_commits",
                    "code_lines",
                    "complexity",
                    "dependents",
                ],
            );
            for hotspot in &dna.git.hot_spots {
                line(
                    &mut out,
                    &[
                        hotspot.rank.to_string(),
                        hotspot.path.clone(),
                        hotspot.score.to_string(),
                        hotspot.commits.to_string(),
                        hotspot.authors.to_string(),
                        hotspot.churn.to_string(),
                        hotspot.recent_commits.to_string(),
                        hotspot.lines.to_string(),
                        hotspot.complexity.to_string(),
                        hotspot.dependents.to_string(),
                    ],
                );
            }
        }
        CsvTable::Dependencies => {
            header(
                &mut out,
                &[
                    "ecosystem",
                    "name",
                    "requirement",
                    "scope",
                    "internal",
                    "resolved",
                    "manifests",
                ],
            );
            for dependency in &dna.dependencies.dependencies {
                line(
                    &mut out,
                    &[
                        dependency.ecosystem.clone(),
                        dependency.name.clone(),
                        opt(dependency.requirement.as_ref()),
                        format!("{:?}", dependency.scope).to_lowercase(),
                        dependency.internal.to_string(),
                        dependency.resolved.join(";"),
                        dependency.manifests.join(";"),
                    ],
                );
            }
        }
        CsvTable::Contributors => {
            header(
                &mut out,
                &[
                    "id",
                    "name",
                    "commits",
                    "insertions",
                    "deletions",
                    "first_commit",
                    "last_commit",
                    "active_days",
                ],
            );
            for contributor in &dna.git.contributors {
                line(
                    &mut out,
                    &[
                        contributor.id.clone(),
                        contributor.name.clone(),
                        contributor.commits.to_string(),
                        contributor.insertions.to_string(),
                        contributor.deletions.to_string(),
                        contributor.first_commit.date_string(),
                        contributor.last_commit.date_string(),
                        contributor.active_days.to_string(),
                    ],
                );
            }
        }
        CsvTable::Languages => {
            header(
                &mut out,
                &[
                    "id",
                    "name",
                    "files",
                    "bytes",
                    "code_lines",
                    "comment_lines",
                    "blank_lines",
                    "share",
                ],
            );
            for language in &dna.languages.languages {
                line(
                    &mut out,
                    &[
                        language.id.clone(),
                        language.name.clone(),
                        language.files.to_string(),
                        language.bytes.to_string(),
                        language.code_lines.to_string(),
                        language.comment_lines.to_string(),
                        language.blank_lines.to_string(),
                        language.share.to_string(),
                    ],
                );
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::metadata::AnalysisMetadata;
    use repodna_core::model::structure::{FileCategory, FileRecord};

    #[test]
    fn exports_quoted_formula_safe_rows() {
        let mut dna =
            RepositoryDna::new(RepositoryIdentity::default(), AnalysisMetadata::default());
        dna.structure.files = vec![
            FileRecord::new("src/a,b.rs", FileCategory::Source, 10),
            FileRecord::new("=cmd.csv", FileCategory::Data, 1),
        ];
        let text = csv(&dna, CsvTable::Files);
        let lines: Vec<&str> = text.split("\r\n").collect();
        assert!(lines[0].starts_with("path,category,language,bytes"));
        assert!(lines[1].starts_with("\"src/a,b.rs\",Source code,,10,"));
        assert!(lines[2].starts_with("'=cmd.csv,Data files"));
        assert_eq!(lines.len(), 4);
        assert!(csv(&dna, CsvTable::Hotspots).starts_with("rank,path,score"));
        assert_eq!("metrics".parse::<CsvTable>().unwrap(), CsvTable::Metrics);
        assert!("nope".parse::<CsvTable>().is_err());
    }
}
