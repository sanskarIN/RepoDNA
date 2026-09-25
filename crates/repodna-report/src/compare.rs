//! Descriptive comparison of two or more repositories.
//!
//! The comparison describes differences with evidence; it does not rank repositories or
//! declare a winner. Observations name the largest and smallest values of a measure, or
//! the ratio between two repositories, and say what the measure is.

use std::collections::BTreeMap;

use repodna_core::config::ReportTheme;
use repodna_core::model::artifact::RepositoryDna;

use crate::charts::{Share, share_bar};
use crate::doc::{Blocks, Inline, Table, plain};
use crate::facts::{NAMED_LANGUAGES, facts};
use crate::html::{self, Page};
use crate::markdown;
use crate::text::{counted, number, percent, span, thousands};

/// One compared measure.
#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonRow {
    /// What is measured.
    pub measure: String,
    /// Display value per repository (in input order).
    pub values: Vec<String>,
}

/// A comparison of repositories.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    /// Repository names (disambiguated when equal).
    pub names: Vec<String>,
    /// Measures side by side.
    pub rows: Vec<ComparisonRow>,
    /// DNA fingerprint dimensions side by side.
    pub fingerprint: Vec<ComparisonRow>,
    /// Neutral, evidence-based observations.
    pub observations: Vec<String>,
    /// Language shares per repository with slots shared across repositories.
    pub languages: Vec<Vec<Share>>,
}

fn names(artifacts: &[RepositoryDna]) -> Vec<String> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for dna in artifacts {
        *counts.entry(dna.identity.name.as_str()).or_default() += 1;
    }
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    artifacts
        .iter()
        .map(|dna| {
            let name = dna.identity.name.as_str();
            if counts[name] > 1 {
                let index = seen.entry(name).or_default();
                *index += 1;
                format!("{name} ({})", index)
            } else {
                name.to_owned()
            }
        })
        .collect()
}

/// Languages with slots assigned by combined share, so a language keeps its color in every
/// repository's bar.
fn language_shares(artifacts: &[RepositoryDna]) -> Vec<Vec<Share>> {
    let per_repo: Vec<Vec<(String, f64)>> = artifacts
        .iter()
        .map(|dna| {
            facts(dna)
                .languages
                .into_iter()
                .map(|l| (l.name, l.share))
                .collect()
        })
        .collect();
    let mut combined: BTreeMap<&str, f64> = BTreeMap::new();
    for languages in &per_repo {
        for (name, share) in languages {
            *combined.entry(name.as_str()).or_default() += share;
        }
    }
    let mut ranked: Vec<(&str, f64)> = combined.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let slots: BTreeMap<&str, usize> = ranked
        .iter()
        .take(NAMED_LANGUAGES)
        .enumerate()
        .map(|(slot, (name, _))| (*name, slot))
        .collect();
    per_repo
        .iter()
        .map(|languages| {
            let mut shares: Vec<Share> = Vec::new();
            let mut other = 0.0;
            for (name, share) in languages {
                match slots.get(name.as_str()) {
                    Some(slot) => shares.push(Share {
                        name: name.clone(),
                        share: *share,
                        slot: Some(*slot),
                    }),
                    None => other += share,
                }
            }
            if other > 0.0 {
                shares.push(Share {
                    name: "Other".to_owned(),
                    share: other,
                    slot: None,
                });
            }
            shares
        })
        .collect()
}

/// A numeric measure used for observations.
struct Measure<'a> {
    label: &'a str,
    values: Vec<Option<f64>>,
    format: fn(f64) -> String,
}

fn integer(value: f64) -> String {
    thousands(value.max(0.0) as u64)
}

fn observe(names: &[String], measure: &Measure<'_>) -> Option<String> {
    let known: Vec<(usize, f64)> = measure
        .values
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.map(|v| (i, v)))
        .collect();
    if known.len() < 2 {
        return None;
    }
    let (max_index, max) = known
        .iter()
        .copied()
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))?;
    let (min_index, min) = known
        .iter()
        .copied()
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))?;
    if (max - min).abs() < f64::EPSILON {
        return Some(format!(
            "{}: the same for all compared repositories ({}).",
            measure.label,
            (measure.format)(max)
        ));
    }
    if known.len() == 2 && min > 0.0 {
        return Some(format!(
            "{}: {} has {} and {} has {} ({:.1}×).",
            measure.label,
            names[max_index],
            (measure.format)(max),
            names[min_index],
            (measure.format)(min),
            max / min
        ));
    }
    Some(format!(
        "{}: highest in {} ({}), lowest in {} ({}).",
        measure.label,
        names[max_index],
        (measure.format)(max),
        names[min_index],
        (measure.format)(min)
    ))
}

/// Compares artifacts (at least two).
pub fn compare(artifacts: &[RepositoryDna]) -> Comparison {
    let names = names(artifacts);
    let all_facts: Vec<_> = artifacts.iter().map(facts).collect();
    let row = |measure: &str, values: Vec<String>| ComparisonRow {
        measure: measure.to_owned(),
        values,
    };
    let not = || "not analyzed".to_owned();
    let rows = vec![
        row(
            "Main languages",
            all_facts
                .iter()
                .map(|f| {
                    let top: Vec<String> = f
                        .languages
                        .iter()
                        .take(3)
                        .map(|l| format!("{} {}", l.name, percent(l.share)))
                        .collect();
                    if top.is_empty() {
                        "none detected".to_owned()
                    } else {
                        top.join(", ")
                    }
                })
                .collect(),
        ),
        row(
            "Files",
            all_facts.iter().map(|f| thousands(f.files)).collect(),
        ),
        row(
            "Code lines",
            all_facts.iter().map(|f| thousands(f.code_lines)).collect(),
        ),
        row(
            "Architecture",
            artifacts
                .iter()
                .map(|dna| {
                    if dna.architecture.status.has_results() {
                        format!(
                            "{} ({}, {}, {})",
                            dna.architecture.style,
                            counted(dna.architecture.modules.len() as u64, "module", "modules"),
                            counted(
                                dna.architecture.module_edges.len() as u64,
                                "dependency",
                                "dependencies"
                            ),
                            counted(dna.architecture.cycles.len() as u64, "cycle", "cycles")
                        )
                    } else {
                        not()
                    }
                })
                .collect(),
        ),
        row(
            "Direct dependencies",
            artifacts
                .iter()
                .map(|dna| {
                    if dna.dependencies.status.has_results() {
                        let ecosystems: Vec<&str> = dna
                            .dependencies
                            .ecosystems
                            .iter()
                            .map(|e| e.ecosystem.as_str())
                            .collect();
                        format!(
                            "{} ({})",
                            thousands(dna.dependencies.direct_count),
                            ecosystems.join(", ")
                        )
                    } else {
                        not()
                    }
                })
                .collect(),
        ),
        row(
            "Commits",
            all_facts
                .iter()
                .map(|f| f.commits.map_or_else(not, thousands))
                .collect(),
        ),
        row(
            "History",
            all_facts
                .iter()
                .map(|f| f.age_days.map_or_else(not, span))
                .collect(),
        ),
        row(
            "Activity",
            all_facts
                .iter()
                .map(|f| f.activity.clone().unwrap_or_else(not))
                .collect(),
        ),
        row(
            "Contributor identities",
            artifacts
                .iter()
                .map(|dna| {
                    if dna.git.status.has_results() {
                        format!(
                            "{} (half of the commits by {})",
                            dna.git.ownership.contributors,
                            dna.git.ownership.contributors_for_half_of_commits
                        )
                    } else {
                        not()
                    }
                })
                .collect(),
        ),
        row(
            "Files with tests",
            all_facts
                .iter()
                .map(|f| f.test_files.map_or_else(not, thousands))
                .collect(),
        ),
        row(
            "Documentation checks met",
            artifacts
                .iter()
                .map(|dna| {
                    if dna.docs.status.has_results() {
                        let met = dna
                            .docs
                            .checks
                            .iter()
                            .filter(|c| {
                                c.status == repodna_core::model::project::DocCheckStatus::Present
                            })
                            .count();
                        format!("{met} of {}", dna.docs.checks.len())
                    } else {
                        not()
                    }
                })
                .collect(),
        ),
        row(
            "Average cyclomatic complexity",
            artifacts
                .iter()
                .map(|dna| {
                    if dna.code_quality.status.has_results() {
                        number(dna.code_quality.complexity.average_cyclomatic)
                    } else {
                        not()
                    }
                })
                .collect(),
        ),
        row(
            "Active findings",
            artifacts
                .iter()
                .map(|dna| {
                    let c = dna.finding_counts();
                    format!(
                        "{} critical, {} warning, {} attention, {} info",
                        c.critical, c.warning, c.attention, c.info
                    )
                })
                .collect(),
        ),
    ];
    let mut dimension_ids: Vec<(String, String)> = Vec::new();
    for dna in artifacts {
        for dimension in &dna.fingerprint.dimensions {
            if !dimension_ids.iter().any(|(id, _)| *id == dimension.id) {
                dimension_ids.push((dimension.id.clone(), dimension.label.clone()));
            }
        }
    }
    let fingerprint = dimension_ids
        .iter()
        .map(|(id, label)| ComparisonRow {
            measure: label.clone(),
            values: artifacts
                .iter()
                .map(|dna| match dna.fingerprint.dimension(id) {
                    Some(d)
                        if d.confidence != repodna_core::confidence::Confidence::Unavailable =>
                    {
                        format!("{} ({} {})", number(d.value), number(d.raw), d.unit)
                    }
                    _ => "not measured".to_owned(),
                })
                .collect(),
        })
        .collect();
    let git = |dna: &RepositoryDna| dna.git.status.has_results();
    let measures = [
        Measure {
            label: "Code lines",
            values: all_facts
                .iter()
                .map(|f| Some(f.code_lines as f64))
                .collect(),
            format: integer,
        },
        Measure {
            label: "Inferred modules",
            values: artifacts
                .iter()
                .map(|d| {
                    d.architecture
                        .status
                        .has_results()
                        .then_some(d.architecture.modules.len() as f64)
                })
                .collect(),
            format: integer,
        },
        Measure {
            label: "Direct dependencies",
            values: artifacts
                .iter()
                .map(|d| {
                    d.dependencies
                        .status
                        .has_results()
                        .then_some(d.dependencies.direct_count as f64)
                })
                .collect(),
            format: integer,
        },
        Measure {
            label: "Commits",
            values: artifacts
                .iter()
                .map(|d| git(d).then_some(d.git.commit_count as f64))
                .collect(),
            format: integer,
        },
        Measure {
            label: "Contributor identities",
            values: artifacts
                .iter()
                .map(|d| git(d).then_some(f64::from(d.git.ownership.contributors)))
                .collect(),
            format: integer,
        },
        Measure {
            label: "Commits in the last 90 days",
            values: artifacts
                .iter()
                .map(|d| git(d).then_some(d.git.activity.commits_last_90_days as f64))
                .collect(),
            format: integer,
        },
    ];
    let observations = measures
        .iter()
        .filter_map(|measure| observe(&names, measure))
        .collect();
    Comparison {
        names,
        rows,
        fingerprint,
        observations,
        languages: language_shares(artifacts),
    }
}

fn table(names: &[String], rows: &[ComparisonRow]) -> Table {
    let mut headers: Vec<&str> = vec!["Measure"];
    headers.extend(names.iter().map(String::as_str));
    let mut table = Table::new(&headers);
    for row in rows {
        let mut cells = vec![plain(row.measure.clone())];
        cells.extend(row.values.iter().map(|v| plain(v.clone())));
        table.row(cells);
    }
    table
}

/// The comparison as document blocks.
pub fn blocks(comparison: &Comparison, figures: bool) -> Blocks {
    let mut blocks = Blocks::default();
    blocks.heading(
        1,
        format!("Repository comparison: {}", comparison.names.join(" · ")),
        Some("comparison"),
    );
    blocks.text("A descriptive comparison built from each repository's RepoDNA artifact. Differences are described with the measure behind them; no repository is ranked.");
    blocks.heading(2, "Side by side", Some("side-by-side"));
    blocks.table(table(&comparison.names, &comparison.rows));
    if figures {
        for (name, shares) in comparison.names.iter().zip(&comparison.languages) {
            blocks.figure(
                share_bar(shares, &format!("Languages in {name}")),
                format!("Share of first-party code by language in {name}"),
            );
        }
    }
    if !comparison.observations.is_empty() {
        blocks.heading(2, "Observations", Some("observations"));
        blocks.list(
            comparison
                .observations
                .iter()
                .map(|o| vec![Inline::Text(o.clone())])
                .collect(),
        );
    }
    if !comparison.fingerprint.is_empty() {
        blocks.heading(2, "DNA fingerprint", Some("fingerprint"));
        blocks.note("Each dimension is normalized to 0–1 and characterizes a repository; higher is not better. The measured value and unit are in parentheses.");
        blocks.table(table(&comparison.names, &comparison.fingerprint));
    }
    blocks
}

/// The comparison as Markdown.
pub fn markdown_comparison(comparison: &Comparison) -> String {
    let mut out = markdown::render(&blocks(comparison, false));
    out.push_str("---\n\nGenerated by [RepoDNA](https://github.com/sanskarIN/RepoDNA).\n");
    out
}

/// The comparison as a self-contained HTML page.
pub fn html_comparison(comparison: &Comparison, theme: ReportTheme, branding: bool) -> String {
    let mut footer = vec![
        Inline::Text("Generated by ".to_owned()),
        Inline::Link {
            text: "RepoDNA".to_owned(),
            url: crate::PROJECT_URL.to_owned(),
        },
        Inline::Text(format!(" {}.", env!("CARGO_PKG_VERSION"))),
    ];
    if branding {
        footer.push(Inline::Text(" Made by the Sanskar.".to_owned()));
    }
    html::render(
        &blocks(comparison, true),
        &Page {
            title: format!("{} — comparison · RepoDNA", comparison.names.join(" vs ")),
            theme,
            footer,
            signature: serde_json::json!({ "generatedBy": "RepoDNA", "toolVersion": env!("CARGO_PKG_VERSION"), "compared": comparison.names }).to_string(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        },
    )
}

/// The comparison as JSON.
pub fn json_comparison(comparison: &Comparison) -> serde_json::Value {
    let rows = |rows: &[ComparisonRow]| -> Vec<serde_json::Value> {
        rows.iter()
            .map(|row| serde_json::json!({ "measure": row.measure, "values": row.values }))
            .collect()
    };
    serde_json::json!({
        "repositories": comparison.names,
        "measures": rows(&comparison.rows),
        "fingerprint": rows(&comparison.fingerprint),
        "observations": comparison.observations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::SectionStatus;
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::languages::{LanguageKind, LanguageStat, ParserCapability};
    use repodna_core::model::metadata::AnalysisMetadata;

    fn artifact(name: &str, lines: u64, language: &str, commits: u64) -> RepositoryDna {
        let mut dna = RepositoryDna::new(
            RepositoryIdentity {
                name: name.into(),
                ..RepositoryIdentity::default()
            },
            AnalysisMetadata::default(),
        );
        dna.structure.code_lines = lines;
        dna.languages.languages = vec![LanguageStat {
            id: language.to_lowercase(),
            name: language.into(),
            kind: LanguageKind::Programming,
            files: 1,
            bytes: 1,
            code_lines: lines,
            comment_lines: 0,
            blank_lines: 0,
            share: 1.0,
            capability: ParserCapability::Lexical,
        }];
        dna.git.status = SectionStatus::Analyzed;
        dna.git.commit_count = commits;
        dna
    }

    #[test]
    fn describes_differences_without_ranking() {
        let a = artifact("alpha", 1000, "Rust", 40);
        let b = artifact("beta", 2500, "Go", 40);
        let comparison = compare(&[a, b]);
        assert_eq!(comparison.names, vec!["alpha", "beta"]);
        assert!(
            comparison
                .observations
                .iter()
                .any(|o| o == "Code lines: beta has 2,500 and alpha has 1,000 (2.5×).")
        );
        assert!(
            comparison
                .observations
                .iter()
                .any(|o| o == "Commits: the same for all compared repositories (40).")
        );
        let row = comparison
            .rows
            .iter()
            .find(|r| r.measure == "Main languages")
            .unwrap();
        assert_eq!(row.values, vec!["Rust 100.0%", "Go 100.0%"]);
        let markdown = markdown_comparison(&comparison);
        assert!(markdown.contains("| Measure | alpha | beta |"));
        assert!(!markdown.to_lowercase().contains("winner"));
        assert_eq!(json_comparison(&comparison)["repositories"][1], "beta");
    }

    #[test]
    fn keeps_language_colors_across_repositories_and_names_distinct() {
        let a = artifact("same", 10, "Rust", 1);
        let b = artifact("same", 30, "Rust", 1);
        let comparison = compare(&[a, b]);
        assert_eq!(comparison.names, vec!["same (1)", "same (2)"]);
        assert_eq!(
            comparison.languages[0][0].slot,
            comparison.languages[1][0].slot
        );
        let html = html_comparison(&comparison, ReportTheme::Professional, true);
        assert!(html.contains("Repository comparison"));
        assert!(html.contains("Made by the Sanskar"));
    }
}
