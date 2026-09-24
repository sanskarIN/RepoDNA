//! Test detection: test files, directories, kinds, commands, and coverage artifacts.

use std::collections::{BTreeMap, BTreeSet};

use repodna_core::evidence::Evidence;
use repodna_core::metric::round4;
use repodna_core::model::SectionStatus;
use repodna_core::model::project::{
    CommandCandidate, CommandPurpose, DetectedTool, TestKind, TestKindSignal, TestReport,
};
use repodna_core::model::structure::FileCategory;
use repodna_core::paths;

use crate::ProjectFile;

/// Maximum test directories listed.
const MAX_DIRECTORIES: usize = 50;

/// Maximum coverage artifacts listed.
const MAX_COVERAGE: usize = 20;

/// Sample files listed per test kind.
const SAMPLES: usize = 3;

/// Directory names that hold tests.
const TEST_DIRS: &[&str] = &[
    "__tests__",
    "e2e",
    "integration",
    "spec",
    "specs",
    "test",
    "testing",
    "tests",
];

/// File names of coverage reports.
const COVERAGE_FILES: &[&str] = &[
    ".coverage",
    "clover.xml",
    "cobertura-coverage.xml",
    "cobertura.xml",
    "coverage-final.json",
    "coverage.xml",
    "jacoco.xml",
    "lcov.info",
];

/// The kind of test a test file most likely holds, judged from its path.
pub fn test_kind(path: &str, language: Option<&str>) -> TestKind {
    let lower = path.to_ascii_lowercase();
    let parts: Vec<&str> = lower.split('/').collect();
    let name = paths::file_name(&lower);
    let has = |candidates: &[&str]| parts.iter().any(|part| candidates.contains(part));
    if has(&["bench", "benches", "benchmark", "benchmarks"]) || name.contains("bench") {
        TestKind::Benchmark
    } else if has(&["e2e", "end-to-end", "cypress", "playwright", "acceptance"])
        || name.contains(".e2e.")
        || name.contains("_e2e")
    {
        TestKind::EndToEnd
    } else if has(&["__snapshots__", "snapshots", "golden"]) || name.ends_with(".snap") {
        TestKind::Snapshot
    } else if has(&[
        "integration",
        "integration-tests",
        "integration_tests",
        "it",
    ]) || name.contains("integration")
        || (language == Some("rust") && has(&["tests"]))
    {
        TestKind::Integration
    } else {
        TestKind::Unit
    }
}

/// The directory that holds a test file: the path up to its first test directory, or the
/// file's own directory.
fn test_directory(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let directories = &parts[..parts.len().saturating_sub(1)];
    match directories
        .iter()
        .position(|part| TEST_DIRS.contains(&part.to_ascii_lowercase().as_str()))
    {
        Some(position) => directories[..=position].join("/"),
        None => directories.join("/"),
    }
}

fn kind_rank(kind: TestKind) -> u8 {
    match kind {
        TestKind::Unit => 0,
        TestKind::Integration => 1,
        TestKind::EndToEnd => 2,
        TestKind::Snapshot => 3,
        TestKind::Benchmark => 4,
    }
}

/// Coverage reports committed to the repository.
fn coverage_artifacts(files: &[ProjectFile<'_>]) -> Vec<String> {
    let mut found = BTreeSet::new();
    for file in files {
        let name = paths::file_name(file.path);
        if COVERAGE_FILES.contains(&name) {
            found.insert(file.path.to_owned());
            continue;
        }
        let parts: Vec<&str> = file.path.split('/').collect();
        if let Some(position) = parts[..parts.len() - 1]
            .iter()
            .position(|part| matches!(*part, "coverage" | "htmlcov"))
            && parts[position + 1..].iter().any(|part| {
                part.ends_with(".html") || part.ends_with(".info") || part.ends_with(".json")
            })
        {
            found.insert(parts[..=position].join("/"));
        }
    }
    found.into_iter().take(MAX_COVERAGE).collect()
}

/// Assembles the test section.
pub fn test_report(
    files: &[ProjectFile<'_>],
    frameworks: Vec<DetectedTool>,
    commands: &[CommandCandidate],
    ci_commands: Vec<CommandCandidate>,
) -> TestReport {
    let tests: Vec<&ProjectFile<'_>> = files
        .iter()
        .filter(|file| file.category == FileCategory::Test)
        .collect();
    let sources: Vec<&ProjectFile<'_>> = files
        .iter()
        .filter(|file| file.category == FileCategory::Source)
        .collect();
    let test_lines: u64 = tests.iter().map(|file| file.code_lines).sum();
    let source_lines: u64 = sources.iter().map(|file| file.code_lines).sum();

    let mut kinds: BTreeMap<u8, (TestKind, u64, Vec<Evidence>)> = BTreeMap::new();
    let mut directories = BTreeSet::new();
    for file in &tests {
        let kind = test_kind(file.path, file.language);
        let entry = kinds
            .entry(kind_rank(kind))
            .or_insert_with(|| (kind, 0, Vec::new()));
        entry.1 += 1;
        if entry.2.len() < SAMPLES {
            entry.2.push(Evidence::file(file.path));
        }
        directories.insert(test_directory(file.path));
    }

    let mut notes = Vec::new();
    let inline = files.iter().filter(|file| file.inline_tests).count();
    if inline > 0 {
        let plural = if inline == 1 { "" } else { "s" };
        notes.push(format!(
            "{inline} source file{plural} contain inline tests (such as Rust #[cfg(test)] modules); their code counts as source code."
        ));
    }
    if tests.is_empty() && inline == 0 {
        notes.push(
            "No test files were detected by path and naming conventions. Tests kept in unusual locations are not recognized."
                .to_owned(),
        );
    }

    TestReport {
        status: SectionStatus::Analyzed,
        notes,
        frameworks,
        test_files: tests.len() as u64,
        test_lines,
        source_files: sources.len() as u64,
        test_ratio: if test_lines + source_lines == 0 {
            0.0
        } else {
            round4(test_lines as f64 / (test_lines + source_lines) as f64)
        },
        test_directories: directories.into_iter().take(MAX_DIRECTORIES).collect(),
        kinds: kinds
            .into_values()
            .map(|(kind, files, evidence)| TestKindSignal {
                kind,
                files,
                evidence,
            })
            .collect(),
        commands: commands
            .iter()
            .filter(|command| command.purpose == CommandPurpose::Test)
            .cloned()
            .collect(),
        ci_commands,
        coverage_artifacts: coverage_artifacts(files),
        executions: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file<'a>(
        path: &'a str,
        category: FileCategory,
        language: &'static str,
        code: u64,
    ) -> ProjectFile<'a> {
        ProjectFile {
            path,
            category,
            language: Some(language),
            code_lines: code,
            total_lines: code,
            inline_tests: false,
        }
    }

    #[test]
    fn classifies_test_kinds_by_path() {
        assert_eq!(
            test_kind("src/util.test.ts", Some("typescript")),
            TestKind::Unit
        );
        assert_eq!(
            test_kind("crates/core/tests/io.rs", Some("rust")),
            TestKind::Integration
        );
        assert_eq!(
            test_kind("tests/integration/test_api.py", Some("python")),
            TestKind::Integration
        );
        assert_eq!(
            test_kind("web/e2e/login.spec.ts", Some("typescript")),
            TestKind::EndToEnd
        );
        assert_eq!(
            test_kind("src/__snapshots__/a.test.ts.snap", None),
            TestKind::Snapshot
        );
        assert_eq!(
            test_kind("benches/parse.rs", Some("rust")),
            TestKind::Benchmark
        );
        assert_eq!(test_directory("pkg/tests/unit/test_a.py"), "pkg/tests");
        assert_eq!(test_directory("src/a_test.go"), "src");
    }

    #[test]
    fn builds_the_test_report() {
        let mut files = vec![
            file("src/lib.rs", FileCategory::Source, "rust", 300),
            file("tests/cli.rs", FileCategory::Test, "rust", 100),
            file("web/src/a.test.ts", FileCategory::Test, "typescript", 50),
            file("web/e2e/flow.spec.ts", FileCategory::Test, "typescript", 50),
            file(
                "coverage/lcov-report/index.html",
                FileCategory::BuildOutput,
                "html",
                10,
            ),
            file("lcov.info", FileCategory::Data, "text", 10),
        ];
        files[0].inline_tests = true;
        let commands = vec![
            CommandCandidate {
                command: "cargo test".into(),
                purpose: CommandPurpose::Test,
                working_directory: String::new(),
                source: "Cargo.toml".into(),
                verified: false,
                evidence: Vec::new(),
            },
            CommandCandidate {
                command: "cargo build".into(),
                purpose: CommandPurpose::Build,
                working_directory: String::new(),
                source: "Cargo.toml".into(),
                verified: false,
                evidence: Vec::new(),
            },
        ];
        let report = test_report(&files, Vec::new(), &commands, Vec::new());
        assert_eq!(report.test_files, 3);
        assert_eq!(report.test_lines, 200);
        assert_eq!(report.source_files, 1);
        assert_eq!(report.test_ratio, 0.4);
        assert_eq!(report.test_directories, vec!["tests", "web/e2e", "web/src"]);
        let kinds: Vec<(TestKind, u64)> = report.kinds.iter().map(|k| (k.kind, k.files)).collect();
        assert_eq!(
            kinds,
            vec![
                (TestKind::Unit, 1),
                (TestKind::Integration, 1),
                (TestKind::EndToEnd, 1)
            ]
        );
        assert_eq!(report.commands.len(), 1);
        assert_eq!(report.coverage_artifacts, vec!["coverage", "lcov.info"]);
        assert!(report.notes[0].contains("inline tests"));

        let empty = test_report(&files[..1], Vec::new(), &[], Vec::new());
        assert_eq!(empty.test_ratio, 0.0);
        assert_eq!(empty.notes.len(), 1);
    }
}
