//! Findings derived from quality measurements and hotspots.
//!
//! Quality metrics describe code; they rarely prove a defect. Following the published
//! severity criteria, size, complexity, duplication, and hotspot findings are `Attention`,
//! and descriptive observations are `Info`.

use repodna_core::confidence::Confidence;
use repodna_core::config::Thresholds;
use repodna_core::evidence::{Evidence, format_number};
use repodna_core::finding::{Finding, FindingCategory};
use repodna_core::model::git::Hotspot;
use repodna_core::model::quality::{FunctionSignal, MarkerKind, QualityReport, SimilarityReport};
use repodna_core::severity::Severity;
use repodna_core::text::count;

use crate::complexity::COMPLEXITY_METHOD;
use crate::duplication::DUPLICATION_METHOD;
use crate::hotspots::HOTSPOT_METHOD;
use crate::similarity::SIMILARITY_METHOD;

/// Maximum findings per rule for per-item rules.
pub const MAX_PER_RULE: usize = 20;

/// Duplicated share of code at or above which a repository-wide finding is reported.
pub const DUPLICATION_RATIO: f64 = 0.05;

/// Minimum ranked files before hotspot findings are reported.
const MIN_HOTSPOT_CANDIDATES: usize = 10;

/// Minimum hotspot score for a finding.
const HOTSPOT_SCORE: f64 = 0.6;

/// Hotspot findings reported.
const MAX_HOTSPOT_FINDINGS: usize = 5;

const LEXICAL_LIMITATION: &str = "Functions and their complexity are found lexically; macros, \
generated code, and unusual formatting can shift the numbers.";

fn percent(value: f64) -> String {
    format_number((value * 1000.0).round() / 10.0)
}

fn function_subject(function: &FunctionSignal) -> String {
    format!("{}:{}:{}", function.path, function.line, function.name)
}

fn function_evidence(function: &FunctionSignal) -> Evidence {
    Evidence::lines(&function.path, function.line, function.end_line)
        .with_note(format!("{} ({} lines)", function.name, function.lines))
}

/// Derives findings from the quality report and the similarity report.
pub fn quality_findings(
    report: &QualityReport,
    similarity: &SimilarityReport,
    thresholds: &Thresholds,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    for function in report
        .complexity
        .top_functions
        .iter()
        .filter(|f| f.cyclomatic >= thresholds.high_complexity)
        .take(MAX_PER_RULE)
    {
        findings.push(
            Finding::new(
                "quality.complex-function",
                &function_subject(function),
                FindingCategory::Complexity,
                Severity::Attention,
                Confidence::Medium,
                format!("{} has complexity {}", function.name, function.cyclomatic),
            )
            .summary(format!(
                "{} in {} has approximate cyclomatic complexity {} ({} lines, nesting depth {}).",
                function.name, function.path, function.cyclomatic, function.lines, function.nesting
            ))
            .rationale("Each decision point adds a path through the code; functions with many paths are harder to understand and to test completely.")
            .method(COMPLEXITY_METHOD)
            .evidence(function_evidence(function))
            .evidence(Evidence::metric_with_threshold(
                "quality.function.cyclomatic",
                f64::from(function.cyclomatic),
                f64::from(thresholds.high_complexity),
                "paths",
            ))
            .limitation(LEXICAL_LIMITATION)
            .next_step("Split the function along its main branches, or replace nested conditionals with early returns or lookup tables.")
            .path(function.path.clone()),
        );
    }

    for function in report.large_functions.iter().take(MAX_PER_RULE) {
        findings.push(
            Finding::new(
                "quality.long-function",
                &function_subject(function),
                FindingCategory::Complexity,
                Severity::Attention,
                Confidence::Medium,
                format!("{} is {} lines long", function.name, function.lines),
            )
            .summary(format!(
                "{} in {} spans lines {}–{}.",
                function.name, function.path, function.line, function.end_line
            ))
            .rationale("Long functions tend to do several things at once, which makes them harder to name, review, and reuse.")
            .method("Function bodies are delimited lexically (braces, indentation, or end keywords) and measured in lines.")
            .evidence(function_evidence(function))
            .evidence(Evidence::metric_with_threshold(
                "quality.function.lines",
                f64::from(function.lines),
                f64::from(thresholds.large_function_lines),
                "lines",
            ))
            .limitation(LEXICAL_LIMITATION)
            .next_step("Extract the distinct steps into well-named helper functions.")
            .path(function.path.clone()),
        );
    }

    for function in report.deep_nesting.iter().take(MAX_PER_RULE) {
        findings.push(
            Finding::new(
                "quality.deep-nesting",
                &function_subject(function),
                FindingCategory::Complexity,
                Severity::Attention,
                Confidence::Medium,
                format!("{} nests {} levels deep", function.name, function.nesting),
            )
            .summary(format!(
                "Blocks inside {} in {} are nested {} levels deep.",
                function.name, function.path, function.nesting
            ))
            .rationale("Deeply nested code forces readers to keep many conditions in mind at once.")
            .method("The deepest block nesting inside each function body, counted lexically.")
            .evidence(function_evidence(function))
            .evidence(Evidence::metric_with_threshold(
                "quality.function.nesting",
                f64::from(function.nesting),
                f64::from(thresholds.deep_nesting),
                "levels",
            ))
            .limitation(LEXICAL_LIMITATION)
            .next_step("Use guard clauses and early returns, or move inner blocks into functions.")
            .path(function.path.clone()),
        );
    }

    for file in report.large_files.iter().take(MAX_PER_RULE) {
        findings.push(
            Finding::new(
                "quality.large-file",
                &file.path,
                FindingCategory::Complexity,
                Severity::Attention,
                Confidence::High,
                format!("{} has {} code lines", file.path, file.value),
            )
            .summary(format!(
                "The file has {} code lines; the threshold is {}.",
                file.value, file.threshold
            ))
            .rationale("Large files often hold several responsibilities and attract changes from many directions.")
            .method("Code lines exclude blank and comment-only lines.")
            .evidence(Evidence::file(&file.path))
            .evidence(Evidence::metric_with_threshold(
                "quality.file.code_lines",
                file.value as f64,
                file.threshold as f64,
                "lines",
            ))
            .next_step("Look for groups of functions or types that change together and could live in their own file.")
            .path(file.path.clone()),
        );
    }

    let duplication = &report.duplication;
    for cluster in duplication.clusters.iter().take(MAX_PER_RULE) {
        let places = cluster.occurrence_count;
        let mut finding = Finding::new(
            "quality.duplicate-block",
            &cluster.id,
            FindingCategory::Duplication,
            Severity::Attention,
            Confidence::High,
            format!(
                "{} duplicated in {places} places",
                count(u64::from(cluster.lines), "line", "lines")
            ),
        )
        .summary(format!(
            "A block of {} normalized tokens ({}) appears {places} times.",
            cluster.tokens,
            count(u64::from(cluster.lines), "line", "lines")
        ))
        .rationale("Duplicated code has to be changed in several places; a fix applied to one copy is easily missed in the others.")
        .method(DUPLICATION_METHOD)
        .with_evidence(cluster.occurrences.iter().take(10).map(|occurrence| {
            Evidence::lines(&occurrence.path, occurrence.start_line, occurrence.end_line)
        }))
        .limitation("Some duplication is deliberate, such as generated-looking tables or independent examples.")
        .next_step("If the copies must stay consistent, extract the shared logic into one function or module.");
        for occurrence in cluster.occurrences.iter().take(10) {
            finding = finding.path(occurrence.path.clone());
        }
        findings.push(finding);
    }
    if duplication.status.has_results()
        && duplication.ratio >= DUPLICATION_RATIO
        && duplication.duplicated_lines >= 50
    {
        findings.push(
            Finding::new(
                "quality.duplication-ratio",
                "repository",
                FindingCategory::Duplication,
                Severity::Attention,
                Confidence::High,
                format!("{}% of analyzed code is duplicated", percent(duplication.ratio)),
            )
            .summary(format!(
                "{} of {} analyzed code lines lie in duplicated blocks.",
                duplication.duplicated_lines, duplication.analyzed_lines
            ))
            .rationale("A high share of duplicated code multiplies the cost of changes and the risk of inconsistent fixes.")
            .method(DUPLICATION_METHOD)
            .evidence(Evidence::metric_with_threshold(
                "quality.duplication.ratio",
                duplication.ratio,
                DUPLICATION_RATIO,
                "ratio",
            ))
            .next_step("Start with the largest duplicate blocks listed in the duplication view."),
        );
    }

    for pair in similarity.similar_files.iter().take(10) {
        findings.push(
            Finding::new(
                "quality.similar-files",
                &format!("{}|{}", pair.a, pair.b),
                FindingCategory::Duplication,
                Severity::Info,
                Confidence::Medium,
                format!(
                    "{} and {} are {}% similar",
                    pair.a,
                    pair.b,
                    percent(pair.similarity)
                ),
            )
            .summary("The two files share most of their token shingles.".to_owned())
            .rationale(
                "Near-copies of whole files often start as copy-and-modify and then drift apart.",
            )
            .method(SIMILARITY_METHOD)
            .evidence(Evidence::file(&pair.a))
            .evidence(Evidence::file(&pair.b))
            .evidence(Evidence::metric_with_threshold(
                "quality.similarity.jaccard_estimate",
                pair.similarity,
                similarity.threshold,
                "ratio",
            ))
            .next_step("Check whether the files should share a common implementation.")
            .path(pair.a.clone())
            .path(pair.b.clone()),
        );
    }

    let markers = &report.markers;
    if markers.total > 0 {
        let urgent: u64 = markers
            .counts
            .iter()
            .filter(|c| {
                matches!(
                    c.kind,
                    MarkerKind::Fixme | MarkerKind::Bug | MarkerKind::Hack
                )
            })
            .map(|c| c.count)
            .sum();
        let breakdown: Vec<String> = markers
            .counts
            .iter()
            .map(|c| format!("{} {}", c.count, c.kind.keyword()))
            .collect();
        findings.push(
            Finding::new(
                "quality.markers",
                "repository",
                FindingCategory::Maintainability,
                Severity::Info,
                Confidence::High,
                format!("{} TODO-style markers in comments", markers.total),
            )
            .summary(format!(
                "Markers found: {}; {urgent} of them are FIXME, BUG, or HACK.",
                breakdown.join(", ")
            ))
            .rationale("Markers record known gaps and shortcuts in the authors' own words; FIXME, BUG, and HACK usually mark work considered unfinished.")
            .method("Comments are scanned for the marker keywords TODO, FIXME, HACK, XXX, BUG, and DEPRECATED; marker text is redacted for secrets.")
            .with_evidence(markers.items.iter().take(5).map(|item| {
                Evidence::line(&item.path, item.line).with_note(item.text.clone())
            }))
            .next_step("Review the FIXME, BUG, and HACK markers first; turn the ones that matter into tracked issues."),
        );
    }

    for candidate in report.dead_code_candidates.iter().take(MAX_PER_RULE) {
        findings.push(
            Finding::new(
                "quality.dead-code",
                &candidate.path,
                FindingCategory::Maintainability,
                Severity::Info,
                candidate.confidence,
                format!("{}: {}", candidate.label, candidate.path),
            )
            .summary(candidate.reason.clone())
            .rationale("Code that nothing uses still has to be read, built, and kept compatible.")
            .method("Static usage analysis: module declarations, resolved imports and file references, and directory names.")
            .with_evidence(candidate.evidence.iter().cloned())
            .limitation("Code can be used through configuration, runtime loading, reflection, code generation, or from outside the repository.")
            .next_step("Confirm that nothing uses it before removing it; if it is used dynamically, document how.")
            .path(candidate.path.clone()),
        );
    }
    findings
}

/// Findings for the strongest hotspots. `candidates` is the number of files that were
/// ranked; with few candidates, ranks carry little meaning and no findings are produced.
pub fn hotspot_findings(hotspots: &[Hotspot], candidates: usize) -> Vec<Finding> {
    if candidates < MIN_HOTSPOT_CANDIDATES {
        return Vec::new();
    }
    hotspots
        .iter()
        .filter(|hotspot| hotspot.score >= HOTSPOT_SCORE)
        .take(MAX_HOTSPOT_FINDINGS)
        .map(|hotspot| {
            Finding::new(
                "activity.hotspot",
                &hotspot.path,
                FindingCategory::Activity,
                Severity::Attention,
                Confidence::Medium,
                format!("{} is a change hotspot (rank {})", hotspot.path, hotspot.rank),
            )
            .summary(hotspot.reasons.join("; "))
            .rationale(hotspot.interpretation.clone())
            .method(HOTSPOT_METHOD)
            .with_evidence(hotspot.evidence.iter().cloned())
            .limitation("History reflects how the repository was worked on, not the quality of the people who worked on it; large refactorings and generated changes also count as churn.")
            .next_step("Make sure the file has good tests and a clear owner; consider splitting responsibilities if it keeps growing.")
            .path(hotspot.path.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::SectionStatus;
    use repodna_core::model::quality::{
        CodeLocation, DeadCodeCandidate, DeadCodeKind, DuplicateCluster, FileSignal, MarkerCount,
        MarkerItem, SimilarFilePair,
    };

    fn function(name: &str, cyclomatic: u32, lines: u32, nesting: u32) -> FunctionSignal {
        FunctionSignal {
            path: "src/a.rs".into(),
            name: name.into(),
            line: 10,
            end_line: 10 + lines - 1,
            lines,
            cyclomatic,
            nesting,
            language: "rust".into(),
        }
    }

    #[test]
    fn produces_one_finding_per_signal() {
        let mut report = QualityReport::default();
        report.complexity.top_functions =
            vec![function("big", 22, 120, 6), function("ok", 3, 5, 1)];
        report.large_functions = vec![function("big", 22, 120, 6)];
        report.deep_nesting = vec![function("big", 22, 120, 6)];
        report.large_files = vec![FileSignal {
            path: "src/a.rs".into(),
            value: 1_500,
            threshold: 1_000,
            unit: "lines".into(),
            language: Some("rust".into()),
        }];
        report.duplication = repodna_core::model::quality::DuplicationReport {
            status: SectionStatus::Analyzed,
            clusters: vec![DuplicateCluster {
                id: "d1".into(),
                tokens: 90,
                lines: 12,
                language: "rust".into(),
                occurrence_count: 2,
                occurrences: vec![
                    CodeLocation {
                        path: "src/a.rs".into(),
                        start_line: 1,
                        end_line: 12,
                    },
                    CodeLocation {
                        path: "src/b.rs".into(),
                        start_line: 5,
                        end_line: 16,
                    },
                ],
            }],
            duplicated_lines: 120,
            analyzed_lines: 1_000,
            ratio: 0.12,
            min_tokens: 70,
            method: String::new(),
        };
        report.markers.total = 2;
        report.markers.counts = vec![
            MarkerCount {
                kind: MarkerKind::Todo,
                count: 1,
            },
            MarkerCount {
                kind: MarkerKind::Fixme,
                count: 1,
            },
        ];
        report.markers.items = vec![MarkerItem {
            path: "src/a.rs".into(),
            line: 3,
            kind: MarkerKind::Fixme,
            text: "handle overflow".into(),
        }];
        report.dead_code_candidates = vec![DeadCodeCandidate {
            path: "src/orphan.rs".into(),
            kind: DeadCodeKind::UnreferencedFile,
            label: "Not included by any module".into(),
            reason: "No mod declaration includes it.".into(),
            confidence: Confidence::Medium,
            evidence: Vec::new(),
        }];
        let similarity = SimilarityReport {
            similar_files: vec![SimilarFilePair {
                a: "src/a.rs".into(),
                b: "src/c.rs".into(),
                similarity: 0.91,
            }],
            threshold: 0.8,
            ..SimilarityReport::default()
        };
        let findings = quality_findings(&report, &similarity, &Thresholds::default());
        let rules: Vec<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
        assert_eq!(
            rules,
            vec![
                "quality.complex-function",
                "quality.long-function",
                "quality.deep-nesting",
                "quality.large-file",
                "quality.duplicate-block",
                "quality.duplication-ratio",
                "quality.similar-files",
                "quality.markers",
                "quality.dead-code",
            ]
        );
        assert_eq!(findings[0].title, "big has complexity 22");
        assert!(findings[5].title.starts_with("12%"));
        assert!(findings[6].title.ends_with("91% similar"));
        assert!(findings[7].summary.contains("1 of them are FIXME"));
        assert_eq!(findings[8].confidence, Confidence::Medium);
        assert!(findings.iter().all(|f| f.severity <= Severity::Attention));
    }

    #[test]
    fn hotspot_findings_need_enough_candidates() {
        let hotspot = Hotspot {
            path: "src/core.rs".into(),
            rank: 1,
            score: 0.9,
            commits: 40,
            authors: 3,
            churn: 900,
            recent_commits: 5,
            lines: 800,
            complexity: 25,
            dependents: 7,
            reasons: vec!["Changed in 40 commits".into()],
            interpretation: "Frequently changed and complex.".into(),
            evidence: Vec::new(),
        };
        assert!(hotspot_findings(std::slice::from_ref(&hotspot), 5).is_empty());
        let findings = hotspot_findings(std::slice::from_ref(&hotspot), 50);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, FindingCategory::Activity);
        assert_eq!(
            findings[0].title,
            "src/core.rs is a change hotspot (rank 1)"
        );
    }
}
