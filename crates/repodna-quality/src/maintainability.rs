//! Repository-wide maintainability signals: ratios that describe the code as a whole.

use repodna_core::config::Thresholds;
use repodna_core::metric::round4;
use repodna_core::model::quality::{
    DuplicationReport, FunctionSignal, MaintainabilitySignal, MarkerReport,
};

use crate::QualityFile;

fn signal(
    id: &str,
    label: &str,
    value: f64,
    unit: &str,
    description: &str,
) -> MaintainabilitySignal {
    MaintainabilitySignal {
        id: id.to_owned(),
        label: label.to_owned(),
        value: round4(value),
        unit: unit.to_owned(),
        description: description.to_owned(),
    }
}

/// Computes the signals whose inputs are available; a signal without data is omitted
/// rather than reported as zero.
pub fn maintainability_signals(
    files: &[QualityFile<'_>],
    functions: &[FunctionSignal],
    duplication: &DuplicationReport,
    markers: &MarkerReport,
    thresholds: &Thresholds,
) -> Vec<MaintainabilitySignal> {
    let mut signals = Vec::new();
    let source = || files.iter().filter(|file| !file.test);
    let code: u64 = source().map(|file| file.lines.code).sum();
    let comments: u64 = source().map(|file| file.lines.comment).sum();
    let test_code: u64 = files
        .iter()
        .filter(|file| file.test)
        .map(|file| file.lines.code)
        .sum();
    let all_code = code + test_code;

    if code + comments > 0 {
        signals.push(signal(
            "comment-ratio",
            "Comment density",
            comments as f64 / (code + comments) as f64,
            "ratio",
            "Share of non-blank lines in non-test code that are comments.",
        ));
    }
    if !functions.is_empty() {
        let count = functions.len() as f64;
        let total_lines: u64 = functions.iter().map(|f| u64::from(f.lines)).sum();
        signals.push(signal(
            "average-function-length",
            "Average function length",
            total_lines as f64 / count,
            "lines",
            "Mean length of measured functions in non-test code.",
        ));
        let complex = functions
            .iter()
            .filter(|f| f.cyclomatic >= thresholds.high_complexity)
            .count();
        signals.push(signal(
            "complex-function-share",
            "Complex functions",
            complex as f64 / count,
            "ratio",
            &format!(
                "Share of functions with approximate cyclomatic complexity of at least {}.",
                thresholds.high_complexity
            ),
        ));
    }
    let measured_files: Vec<&QualityFile<'_>> =
        source().filter(|file| file.lines.code > 0).collect();
    if !measured_files.is_empty() {
        let large = measured_files
            .iter()
            .filter(|file| file.lines.code >= thresholds.large_file_lines)
            .count();
        signals.push(signal(
            "large-file-share",
            "Large files",
            large as f64 / measured_files.len() as f64,
            "ratio",
            &format!(
                "Share of non-test code files with at least {} code lines.",
                thresholds.large_file_lines
            ),
        ));
    }
    if duplication.status.has_results() && duplication.analyzed_lines > 0 {
        signals.push(signal(
            "duplication-ratio",
            "Duplicated code",
            duplication.ratio,
            "ratio",
            "Share of analyzed non-test code lines inside duplicated blocks.",
        ));
    }
    if all_code > 0 {
        signals.push(signal(
            "marker-density",
            "Marker density",
            markers.total as f64 * 1_000.0 / all_code as f64,
            "per 1k lines",
            "TODO, FIXME, and similar markers per 1,000 code lines.",
        ));
    }
    if code > 0 {
        signals.push(signal(
            "test-code-ratio",
            "Test code ratio",
            test_code as f64 / code as f64,
            "ratio",
            "Code lines in test files per code line in non-test files.",
        ));
    }
    signals
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::SectionStatus;
    use repodna_core::model::structure::LineCounts;

    fn file(path: &str, test: bool, code: u64, comment: u64) -> QualityFile<'_> {
        QualityFile {
            path,
            language: Some("rust"),
            test,
            lines: LineCounts {
                total: code + comment,
                code,
                comment,
                blank: 0,
            },
            analysis: None,
            tokens: None,
        }
    }

    #[test]
    fn computes_available_signals_only() {
        let files = [
            file("src/a.rs", false, 1_200, 300),
            file("src/b.rs", false, 300, 0),
            file("tests/a.rs", true, 750, 0),
        ];
        let duplication = DuplicationReport {
            status: SectionStatus::Analyzed,
            analyzed_lines: 1_500,
            duplicated_lines: 150,
            ratio: 0.1,
            ..DuplicationReport::default()
        };
        let markers = MarkerReport {
            total: 9,
            ..MarkerReport::default()
        };
        let signals =
            maintainability_signals(&files, &[], &duplication, &markers, &Thresholds::default());
        let values: Vec<(&str, f64)> = signals.iter().map(|s| (s.id.as_str(), s.value)).collect();
        assert_eq!(
            values,
            vec![
                ("comment-ratio", 0.1667),
                ("large-file-share", 0.5),
                ("duplication-ratio", 0.1),
                ("marker-density", 4.0),
                ("test-code-ratio", 0.5),
            ]
        );
        assert!(
            maintainability_signals(
                &[],
                &[],
                &DuplicationReport::default(),
                &MarkerReport::default(),
                &Thresholds::default()
            )
            .is_empty()
        );
    }
}
