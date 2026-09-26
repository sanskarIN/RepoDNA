//! Complexity and size signals for functions and files.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use repodna_core::metric::round4;
use repodna_core::model::quality::{
    ComplexityReport, DistributionBucket, FileSignal, FunctionSignal, LanguageComplexity,
};

use crate::QualityFile;

/// Functions listed in the complexity overview.
pub const TOP_FUNCTIONS: usize = 25;

/// Entries kept in each large-file, large-function, and deep-nesting list.
pub const MAX_LISTED: usize = 50;

/// How complexity is measured, for reports.
pub const COMPLEXITY_METHOD: &str = "Approximate cyclomatic complexity counted lexically: 1 plus \
the decision points (branches, loops, case labels, boolean operators, and conditional \
expressions) inside each function body. Functions are found with language-specific patterns, \
and each decision point counts toward the innermost enclosing function. Test files are excluded.";

/// Complexity distribution buckets as `(label, min, max)`.
const BUCKETS: [(&str, u32, Option<u32>); 5] = [
    ("1–5", 1, Some(5)),
    ("6–10", 6, Some(10)),
    ("11–20", 11, Some(20)),
    ("21–50", 21, Some(50)),
    ("51+", 51, None),
];

/// Every measured function in non-test files.
pub fn function_signals(files: &[QualityFile<'_>]) -> Vec<FunctionSignal> {
    let mut signals = Vec::new();
    for file in files.iter().filter(|file| !file.test) {
        let (Some(analysis), Some(language)) = (file.analysis, file.language) else {
            continue;
        };
        for function in analysis.functions() {
            signals.push(FunctionSignal {
                path: file.path.to_owned(),
                name: function.name.clone(),
                line: function.line,
                end_line: function.end_line,
                lines: function.length(),
                cyclomatic: function.complexity.unwrap_or(1),
                nesting: function.nesting.unwrap_or(0),
                language: language.to_owned(),
            });
        }
    }
    signals
}

fn median(sorted: &[u32]) -> f64 {
    match sorted.len() {
        0 => 0.0,
        len if len % 2 == 1 => f64::from(sorted[len / 2]),
        len => (f64::from(sorted[len / 2 - 1]) + f64::from(sorted[len / 2])) / 2.0,
    }
}

/// Summarizes the complexity of `functions`.
pub fn complexity_report(functions: &[FunctionSignal]) -> ComplexityReport {
    let mut values: Vec<u32> = functions.iter().map(|f| f.cyclomatic).collect();
    values.sort_unstable();
    let total: u64 = values.iter().map(|&v| u64::from(v)).sum();
    let count = values.len() as u64;
    let distribution = BUCKETS
        .iter()
        .map(|&(label, min, max)| DistributionBucket {
            label: label.to_owned(),
            min,
            max,
            count: values
                .iter()
                .filter(|&&v| v >= min && max.is_none_or(|max| v <= max))
                .count() as u64,
        })
        .collect();

    let mut top: Vec<FunctionSignal> = functions.to_vec();
    top.sort_by(|a, b| {
        b.cyclomatic
            .cmp(&a.cyclomatic)
            .then_with(|| b.lines.cmp(&a.lines))
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.line.cmp(&b.line))
    });
    top.truncate(TOP_FUNCTIONS);

    let mut per_language: BTreeMap<&str, (u64, u64, u32)> = BTreeMap::new();
    for function in functions {
        let entry = per_language.entry(function.language.as_str()).or_default();
        entry.0 += 1;
        entry.1 += u64::from(function.cyclomatic);
        entry.2 = entry.2.max(function.cyclomatic);
    }
    let mut by_language: Vec<LanguageComplexity> = per_language
        .into_iter()
        .map(|(language, (functions, total, max))| LanguageComplexity {
            language: language.to_owned(),
            functions,
            average: round4(total as f64 / functions as f64),
            max,
        })
        .collect();
    by_language.sort_by_key(|entry| Reverse(entry.functions));

    ComplexityReport {
        functions_analyzed: count,
        average_cyclomatic: if count == 0 {
            0.0
        } else {
            round4(total as f64 / count as f64)
        },
        median_cyclomatic: median(&values),
        distribution,
        top_functions: top,
        by_language,
        method: COMPLEXITY_METHOD.to_owned(),
    }
}

/// Non-test files with at least `threshold` code lines, largest first.
pub fn large_files(files: &[QualityFile<'_>], threshold: u64) -> Vec<FileSignal> {
    let mut large: Vec<FileSignal> = files
        .iter()
        .filter(|file| !file.test && file.lines.code >= threshold)
        .map(|file| FileSignal {
            path: file.path.to_owned(),
            value: file.lines.code,
            threshold,
            unit: "lines".to_owned(),
            language: file.language.map(str::to_owned),
        })
        .collect();
    large.sort_by(|a, b| b.value.cmp(&a.value).then_with(|| a.path.cmp(&b.path)));
    large.truncate(MAX_LISTED);
    large
}

/// Functions selected by `measure(function) >= threshold`, largest first.
fn select(
    functions: &[FunctionSignal],
    threshold: u32,
    measure: impl Fn(&FunctionSignal) -> u32,
) -> Vec<FunctionSignal> {
    let mut selected: Vec<FunctionSignal> = functions
        .iter()
        .filter(|function| measure(function) >= threshold)
        .cloned()
        .collect();
    selected.sort_by(|a, b| {
        measure(b)
            .cmp(&measure(a))
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.line.cmp(&b.line))
    });
    selected.truncate(MAX_LISTED);
    selected
}

/// Functions with at least `threshold` lines, longest first.
pub fn large_functions(functions: &[FunctionSignal], threshold: u32) -> Vec<FunctionSignal> {
    select(functions, threshold, |function| function.lines)
}

/// Functions nested at least `threshold` blocks deep, deepest first.
pub fn deep_nesting(functions: &[FunctionSignal], threshold: u32) -> Vec<FunctionSignal> {
    select(functions, threshold, |function| function.nesting)
}

/// Functions with cyclomatic complexity of at least `threshold`, most complex first.
pub fn complex_functions(functions: &[FunctionSignal], threshold: u32) -> Vec<FunctionSignal> {
    select(functions, threshold, |function| function.cyclomatic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::structure::LineCounts;
    use repodna_parser::{FileAnalysis, LanguageRegistry, analyze_source};

    fn analyzed(path: &str, text: &str) -> FileAnalysis {
        let spec = LanguageRegistry::builtin().detect_path(path).unwrap();
        analyze_source(spec, text)
    }

    fn file<'a>(path: &'a str, analysis: &'a FileAnalysis, test: bool) -> QualityFile<'a> {
        QualityFile {
            path,
            language: Some(analysis.language.as_str()),
            test,
            lines: analysis.lines,
            analysis: Some(analysis),
            tokens: None,
        }
    }

    #[test]
    fn summarizes_function_complexity() {
        let complex = analyzed(
            "src/a.py",
            "def simple():\n    return 1\n\ndef branchy(x):\n    if x > 1 and x < 5:\n        for i in range(x):\n            if i:\n                print(i)\n    elif x:\n        return 2\n    return 3\n",
        );
        let tests = analyzed(
            "tests/test_a.py",
            "def test_it():\n    if True:\n        pass\n",
        );
        let files = [
            file("src/a.py", &complex, false),
            file("tests/test_a.py", &tests, true),
        ];
        let functions = function_signals(&files);
        assert_eq!(functions.len(), 2);
        let report = complexity_report(&functions);
        assert_eq!(report.functions_analyzed, 2);
        assert_eq!(report.top_functions[0].name, "branchy");
        assert!(report.top_functions[0].cyclomatic >= 5);
        assert_eq!(report.distribution[0].count, 1);
        assert_eq!(report.by_language[0].language, "python");
        assert_eq!(report.median_cyclomatic, report.average_cyclomatic);
        assert_eq!(complex_functions(&functions, 5).len(), 1);
        assert_eq!(deep_nesting(&functions, 3)[0].name, "branchy");
        assert!(large_functions(&functions, 100).is_empty());
        assert_eq!(large_functions(&functions, 2)[0].name, "branchy");
    }

    #[test]
    fn lists_large_files_and_handles_empty_input() {
        let lines = LineCounts {
            total: 1_500,
            code: 1_200,
            comment: 200,
            blank: 100,
        };
        let files = [QualityFile {
            path: "src/big.rs",
            language: Some("rust"),
            test: false,
            lines,
            analysis: None,
            tokens: None,
        }];
        let large = large_files(&files, 1_000);
        assert_eq!(large.len(), 1);
        assert_eq!(large[0].value, 1_200);
        assert!(large_files(&files, 2_000).is_empty());
        let report = complexity_report(&[]);
        assert_eq!(report.functions_analyzed, 0);
        assert_eq!(report.average_cyclomatic, 0.0);
        assert_eq!(report.median_cyclomatic, 0.0);
        assert_eq!(median(&[1, 3, 5, 7]), 4.0);
    }
}
