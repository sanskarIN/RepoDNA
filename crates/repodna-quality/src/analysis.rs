//! The quality analysis entry point.

use repodna_core::cancel::{CancellationToken, Cancelled};
use repodna_core::config::Thresholds;
use repodna_core::finding::Finding;
use repodna_core::model::SectionStatus;
use repodna_core::model::quality::{DuplicationReport, QualityReport, SimilarityReport};

use crate::QualityFile;
use crate::complexity::{
    complexity_report, deep_nesting, function_signals, large_files, large_functions,
};
use crate::deadcode::{UsageContext, dead_code_candidates};
use crate::duplication::detect_duplication;
use crate::findings::quality_findings;
use crate::maintainability::maintainability_signals;
use crate::markers::marker_report;
use crate::similarity::detect_similarity;

/// Inputs to quality analysis.
#[derive(Debug, Clone, Copy)]
pub struct QualityInput<'a> {
    /// First-party code files.
    pub files: &'a [QualityFile<'a>],
    /// Thresholds from the configuration.
    pub thresholds: &'a Thresholds,
    /// How files are used (for dead-code candidates).
    pub usage: UsageContext<'a>,
    /// Run duplicate-block detection.
    pub duplication: bool,
    /// Run similar-file detection.
    pub similarity: bool,
}

/// Results of quality analysis.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QualityOutput {
    /// The code-quality section.
    pub report: QualityReport,
    /// The similar-files section.
    pub similarity: SimilarityReport,
    /// Quality findings (hotspot findings are produced with the history analysis).
    pub findings: Vec<Finding>,
}

/// Runs quality analysis.
pub fn analyze(
    input: &QualityInput<'_>,
    cancel: &CancellationToken,
) -> Result<QualityOutput, Cancelled> {
    let files = input.files;
    let thresholds = input.thresholds;
    let mut report = QualityReport::default();
    if files.is_empty() {
        report.status = SectionStatus::Unavailable;
        report
            .notes
            .push("No first-party code files were found.".to_owned());
        return Ok(QualityOutput {
            report,
            ..QualityOutput::default()
        });
    }

    let functions = function_signals(files);
    report.complexity = complexity_report(&functions);
    report.large_files = large_files(files, thresholds.large_file_lines);
    report.large_functions = large_functions(&functions, thresholds.large_function_lines);
    report.deep_nesting = deep_nesting(&functions, thresholds.deep_nesting);
    if functions.is_empty() {
        report.notes.push(
            "No functions were measured; complexity needs a language with lexical analysis."
                .to_owned(),
        );
    }
    cancel.check()?;

    report.duplication = if input.duplication {
        let duplication = detect_duplication(files, thresholds.duplicate_min_tokens);
        if duplication.status == SectionStatus::Partial {
            report.notes.push(
                "Some files were not tokenized, so duplication results are partial.".to_owned(),
            );
        }
        duplication
    } else {
        DuplicationReport {
            status: SectionStatus::Skipped,
            ..DuplicationReport::default()
        }
    };
    cancel.check()?;

    let similarity = if input.similarity {
        detect_similarity(files, thresholds.similarity)
    } else {
        SimilarityReport {
            status: SectionStatus::Skipped,
            threshold: thresholds.similarity,
            ..SimilarityReport::default()
        }
    };
    cancel.check()?;

    report.markers = marker_report(files);
    report.dead_code_candidates = dead_code_candidates(files, &input.usage);
    report.maintainability = maintainability_signals(
        files,
        &functions,
        &report.duplication,
        &report.markers,
        thresholds,
    );
    report.status = if report.duplication.status == SectionStatus::Partial {
        SectionStatus::Partial
    } else {
        SectionStatus::Analyzed
    };
    let findings = quality_findings(&report, &similarity, thresholds);
    Ok(QualityOutput {
        report,
        similarity,
        findings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TokenStream;
    use repodna_core::time::Timestamp;
    use repodna_parser::{FileAnalysis, LanguageRegistry, analyze_and_tokenize};
    use std::collections::BTreeSet;

    struct Analyzed {
        path: &'static str,
        analysis: FileAnalysis,
        tokens: TokenStream,
    }

    fn analyzed(path: &'static str, text: &str) -> Analyzed {
        let spec = LanguageRegistry::builtin().detect_path(path).unwrap();
        let (analysis, tokens) = analyze_and_tokenize(spec, text);
        let tokens = TokenStream::for_file(&tokens, Some(&analysis));
        Analyzed {
            path,
            analysis,
            tokens,
        }
    }

    #[test]
    fn runs_every_stage_and_respects_switches() {
        let branchy = (0..20)
            .map(|i| format!("    if x == {i}:\n        y += {i}\n"))
            .collect::<String>();
        let sources = [
            analyzed(
                "app/logic.py",
                &format!("def decide(x):\n    y = 0\n{branchy}    return y\n# TODO: simplify\n"),
            ),
            analyzed(
                "app/copy.py",
                &format!("def decide(x):\n    y = 0\n{branchy}    return y\n"),
            ),
            analyzed(
                "tests/test_logic.py",
                "def test_decide():\n    assert True\n",
            ),
        ];
        let files: Vec<QualityFile<'_>> = sources
            .iter()
            .map(|a| QualityFile {
                path: a.path,
                language: Some(a.analysis.language.as_str()),
                test: a.path.starts_with("tests/"),
                lines: a.analysis.lines,
                analysis: Some(&a.analysis),
                tokens: Some(&a.tokens),
            })
            .collect();
        let thresholds = Thresholds::default();
        let entrypoints = BTreeSet::new();
        let last_changed: Vec<Option<Timestamp>> = vec![None; files.len()];
        let usage = UsageContext {
            dependents: &[1, 1, 0],
            module_declared: &[false, false, false],
            entrypoints: &entrypoints,
            imports_reliable: true,
            last_changed: &last_changed,
            reference_time: None,
        };
        let input = QualityInput {
            files: &files,
            thresholds: &thresholds,
            usage,
            duplication: true,
            similarity: true,
        };
        let output = analyze(&input, &CancellationToken::new()).unwrap();
        let report = &output.report;
        assert_eq!(report.status, SectionStatus::Analyzed);
        assert_eq!(report.complexity.functions_analyzed, 2);
        assert!(report.complexity.top_functions[0].cyclomatic >= 20);
        assert_eq!(report.duplication.clusters.len(), 1);
        assert_eq!(output.similarity.similar_files.len(), 1);
        assert_eq!(report.markers.total, 1);
        let rules: BTreeSet<&str> = output.findings.iter().map(|f| f.rule.as_str()).collect();
        for rule in [
            "quality.complex-function",
            "quality.duplicate-block",
            "quality.similar-files",
            "quality.markers",
        ] {
            assert!(rules.contains(rule), "{rule} missing from {rules:?}");
        }

        let skipped = QualityInput {
            duplication: false,
            similarity: false,
            ..input
        };
        let output = analyze(&skipped, &CancellationToken::new()).unwrap();
        assert_eq!(output.report.duplication.status, SectionStatus::Skipped);
        assert_eq!(output.similarity.status, SectionStatus::Skipped);

        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(analyze(&input, &cancel).is_err());

        let empty = QualityInput {
            files: &[],
            ..input
        };
        let output = analyze(&empty, &CancellationToken::new()).unwrap();
        assert_eq!(output.report.status, SectionStatus::Unavailable);
    }
}
