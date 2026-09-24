//! The project analysis entry point and its findings.

use std::collections::BTreeMap;

use repodna_core::confidence::Confidence;
use repodna_core::evidence::Evidence;
use repodna_core::finding::{Finding, FindingCategory};
use repodna_core::model::SectionStatus;
use repodna_core::model::project::{
    BuildReport, CommandPurpose, DocCheckStatus, DocsReport, EnvironmentRequirement, TestReport,
};
use repodna_core::model::structure::FileCategory;
use repodna_core::severity::Severity;

use crate::ProjectFile;
use crate::ci::{ci_configs, ci_test_commands};
use crate::commands::command_candidates;
use crate::docs::docs_report;
use crate::environment::{configuration_files, containers, environment_requirements};
use crate::testing::test_report;
use crate::tools::{ToolContext, build_systems, test_frameworks};

/// Minimum source files before missing tests are reported.
const MIN_SOURCE_FILES_FOR_TESTS: u64 = 10;

/// Minimum source files before a missing CI configuration is reported.
const MIN_SOURCE_FILES_FOR_CI: u64 = 20;

/// Test-to-code ratio below which a low ratio is reported.
const LOW_TEST_RATIO: f64 = 0.05;

/// Minimum source code lines before a low test ratio is reported.
const MIN_LINES_FOR_RATIO: u64 = 2_000;

/// Inputs to project analysis.
#[derive(Debug, Clone, Copy)]
pub struct ProjectInput<'a> {
    /// Repository files.
    pub files: &'a [ProjectFile<'a>],
    /// Contents of the files selected by [`crate::wants_content`], keyed by path.
    pub contents: &'a BTreeMap<String, String>,
    /// Declared dependencies as `(ecosystem, name)`.
    pub dependencies: &'a [(String, String)],
    /// Requirements declared by manifests.
    pub requirements: &'a [EnvironmentRequirement],
    /// Package descriptions as `(manifest, description)`.
    pub descriptions: &'a [(String, String)],
}

/// Results of project analysis.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectOutput {
    /// Test section.
    pub tests: TestReport,
    /// Build section.
    pub builds: BuildReport,
    /// Documentation section.
    pub docs: DocsReport,
    /// Findings.
    pub findings: Vec<Finding>,
}

/// Runs project analysis.
pub fn analyze(input: &ProjectInput<'_>) -> ProjectOutput {
    let context = ToolContext {
        files: input.files,
        contents: input.contents,
        dependencies: input.dependencies,
    };
    let commands = command_candidates(context);
    let tests = test_report(
        input.files,
        test_frameworks(context),
        &commands,
        ci_test_commands(input.contents),
    );
    let systems = build_systems(context);
    let mut builds = BuildReport {
        status: SectionStatus::Analyzed,
        notes: Vec::new(),
        commands: commands
            .into_iter()
            .filter(|command| command.purpose != CommandPurpose::Test)
            .collect(),
        executions: Vec::new(),
        requirements: environment_requirements(input.files, input.contents, input.requirements),
        ci: ci_configs(input.contents),
        containers: containers(input.files),
        configuration_files: configuration_files(input.files),
        systems,
    };
    if builds.systems.is_empty() {
        builds
            .notes
            .push("No build system was detected from the repository files.".to_owned());
    }
    let docs = docs_report(input.files, input.contents, input.descriptions);
    let findings = project_findings(input.files, &tests, &builds, &docs);
    ProjectOutput {
        tests,
        builds,
        docs,
        findings,
    }
}

/// Derives findings about tests, CI, and documentation.
pub fn project_findings(
    files: &[ProjectFile<'_>],
    tests: &TestReport,
    builds: &BuildReport,
    docs: &DocsReport,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let inline_tests = files.iter().any(|file| file.inline_tests);
    let source_lines: u64 = files
        .iter()
        .filter(|file| file.category == FileCategory::Source)
        .map(|file| file.code_lines)
        .sum();
    let has_tests = tests.test_files > 0 || inline_tests;

    if !has_tests && tests.source_files >= MIN_SOURCE_FILES_FOR_TESTS {
        findings.push(
            Finding::new("tests.none", "repository", FindingCategory::Tests, Severity::Attention, Confidence::Medium, "No automated tests were detected")
                .summary(format!("{} source files, but no test files or inline test modules were found.", tests.source_files))
                .rationale("Without automated tests, changes are verified by hand or not at all, which slows contributors down and lets regressions through.")
                .method("Test files are recognized by directory and file naming conventions; inline test modules by their markers.")
                .evidence(Evidence::metric("tests.files", 0.0))
                .evidence(Evidence::metric("structure.source_files", tests.source_files as f64))
                .limitation("Tests kept in unusual locations or generated at build time are not recognized.")
                .next_step("Add a test framework for the main language and a first test for the most important behavior."),
        );
    }
    if tests.test_files > 0
        && tests.test_ratio < LOW_TEST_RATIO
        && source_lines >= MIN_LINES_FOR_RATIO
    {
        findings.push(
            Finding::new("tests.low-ratio", "repository", FindingCategory::Tests, Severity::Info, Confidence::Medium, "Little test code relative to source code")
                .summary(format!("Test code is {:.1}% of all code lines in source and test files.", tests.test_ratio * 100.0))
                .rationale("A small amount of test code often means large parts of the behavior are not covered, although coverage cannot be measured this way.")
                .method("Code lines in test files divided by code lines in test and source files.")
                .evidence(Evidence::metric_with_threshold("tests.ratio", tests.test_ratio, LOW_TEST_RATIO, "ratio"))
                .limitation("The ratio does not measure coverage; inline tests count as source code.")
                .next_step("Measure coverage with the language's coverage tool to see which areas lack tests."),
        );
    }
    if !builds.ci.is_empty() && has_tests && tests.ci_commands.is_empty() {
        findings.push(
            Finding::new(
                "tests.not-in-ci",
                "repository",
                FindingCategory::Tests,
                Severity::Info,
                Confidence::Medium,
                "CI configuration does not appear to run the tests",
            )
            .summary(format!(
                "{} CI configuration(s) were found, but none contains a recognized test command.",
                builds.ci.len()
            ))
            .rationale("Tests that CI does not run can break without anyone noticing.")
            .method("CI files are searched for common test commands of each ecosystem.")
            .with_evidence(builds.ci.iter().take(5).map(|ci| Evidence::file(&ci.path)))
            .limitation(
                "Tests run through custom scripts or reusable workflows are not recognized.",
            )
            .next_step("Check that a CI job runs the test suite on every change."),
        );
    }
    if builds.ci.is_empty() && tests.source_files >= MIN_SOURCE_FILES_FOR_CI {
        findings.push(
            Finding::new("build.no-ci", "repository", FindingCategory::Build, Severity::Info, Confidence::Medium, "No CI configuration was detected")
                .summary("No configuration for a known CI/CD service was found in the repository.".to_owned())
                .rationale("Continuous integration builds and tests every change the same way, which catches problems before they are merged.")
                .method("Known CI configuration paths (GitHub Actions, GitLab CI, CircleCI, Azure Pipelines, Jenkins, and others) are checked.")
                .evidence(Evidence::observation("No CI configuration file was found"))
                .limitation("CI configured outside the repository, such as in a hosted service's settings, is not visible.")
                .next_step("Add a CI workflow that builds the project and runs its tests."),
        );
    }
    if !tests.coverage_artifacts.is_empty() {
        findings.push(
            Finding::new(
                "tests.coverage-committed",
                "repository",
                FindingCategory::Tests,
                Severity::Info,
                Confidence::High,
                "Coverage reports are committed",
            )
            .summary(format!(
                "{} coverage report(s) are part of the repository.",
                tests.coverage_artifacts.len()
            ))
            .rationale(
                "Generated coverage reports go stale with the next change and add noise to diffs.",
            )
            .method("Well-known coverage report file names and directories.")
            .with_evidence(
                tests
                    .coverage_artifacts
                    .iter()
                    .take(5)
                    .map(|path| Evidence::file(path.as_str())),
            )
            .next_step(
                "Generate coverage in CI instead, and ignore the reports in version control.",
            ),
        );
    }

    let status = |id: &str| {
        docs.checks
            .iter()
            .find(|check| check.id == id)
            .map_or(DocCheckStatus::NotDetected, |check| check.status)
    };
    match status("readme") {
        DocCheckStatus::NotDetected => findings.push(
            Finding::new("docs.readme-missing", "repository", FindingCategory::Documentation, Severity::Attention, Confidence::High, "No README was found")
                .summary("The repository has no README at its root.".to_owned())
                .rationale("The README is the first thing people read; without it they cannot tell what the project does or how to use it.")
                .method("README files are looked up at the repository root and one level below.")
                .evidence(Evidence::observation("No README file was found"))
                .next_step("Add a README that explains what the project does, how to install it, and how to use it."),
        ),
        DocCheckStatus::Partial => findings.push(
            Finding::new("docs.readme-short", "repository", FindingCategory::Documentation, Severity::Info, Confidence::High, "The README is very short or not at the root")
                .summary("A README exists, but it is either not at the repository root or has fewer than 30 words.".to_owned())
                .rationale("A short README leaves newcomers without the basics: purpose, setup, and usage.")
                .method("README location and word count.")
                .with_evidence(docs.readme.iter().map(|readme| Evidence::file(&readme.path).with_note(format!("{} words", readme.words))))
                .next_step("Describe the project's purpose, installation, and usage in the root README."),
        ),
        DocCheckStatus::Present => {
            let missing: Vec<&str> = [("installation", "installation"), ("usage", "usage")]
                .iter()
                .filter(|(id, _)| status(id) == DocCheckStatus::NotDetected)
                .map(|(_, label)| *label)
                .collect();
            if !missing.is_empty() {
                findings.push(
                    Finding::new("docs.setup-instructions", "repository", FindingCategory::Documentation, Severity::Info, Confidence::Medium, format!("The README has no {} section", missing.join(" or ")))
                        .summary(format!("No README heading mentions {}.", missing.join(" or ")))
                        .rationale("Installation and usage instructions are what newcomers look for first.")
                        .method("README headings are matched against common section names.")
                        .with_evidence(docs.readme.iter().map(|readme| Evidence::file(&readme.path)))
                        .limitation("Instructions under unusual headings or in other documents are not recognized.")
                        .next_step("Add short installation and usage sections to the README."),
                );
            }
        }
    }
    match status("license") {
        DocCheckStatus::NotDetected => findings.push(
            Finding::new("docs.license-missing", "repository", FindingCategory::Documentation, Severity::Attention, Confidence::High, "No license was found")
                .summary("Neither a license file nor a license declaration in a manifest was found.".to_owned())
                .rationale("Without a license, others have no explicit permission to use, modify, or share the code.")
                .method("License files at the repository root and license fields in common manifests.")
                .evidence(Evidence::observation("No license file was found"))
                .limitation("A license stated elsewhere, such as in file headers, is not recognized.")
                .next_step("Choose a license and add its text as a LICENSE file at the repository root."),
        ),
        DocCheckStatus::Partial => findings.push(
            Finding::new("docs.license-file-missing", "repository", FindingCategory::Documentation, Severity::Info, Confidence::High, "A license is declared but no license file exists")
                .summary("A manifest declares a license, but the repository has no license file.".to_owned())
                .rationale("Most licenses require the license text to accompany the code.")
                .method("License files at the repository root and license fields in common manifests.")
                .with_evidence(docs.license.iter().map(|license| Evidence::file(&license.path)))
                .next_step("Add the full license text as a LICENSE file."),
        ),
        DocCheckStatus::Present => {}
    }
    let optional_missing: Vec<&str> = docs
        .checks
        .iter()
        .filter(|check| {
            check.optional
                && check.status == DocCheckStatus::NotDetected
                && matches!(
                    check.id.as_str(),
                    "contributing" | "security-policy" | "changelog" | "code-of-conduct"
                )
        })
        .map(|check| check.label.as_str())
        .collect();
    if !optional_missing.is_empty() {
        findings.push(
            Finding::new("docs.optional-missing", "repository", FindingCategory::Documentation, Severity::Info, Confidence::High, "Some optional project documents were not detected")
                .summary(format!("Not detected: {}.", optional_missing.join(", ")))
                .rationale("These documents help contributors and users, but many projects legitimately omit some of them.")
                .method("Well-known file names at the root, in .github/, and in docs/, and README sections.")
                .evidence(Evidence::observation(format!("Not detected: {}", optional_missing.join(", "))))
                .limitation("Not detected does not prove absence; the information may live elsewhere.")
                .next_step("Add the documents that fit the project, starting with a security policy if it accepts vulnerability reports."),
        );
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, category: FileCategory) -> ProjectFile<'_> {
        ProjectFile {
            path,
            category,
            language: Some("python"),
            code_lines: 100,
            total_lines: 120,
            inline_tests: false,
        }
    }

    #[test]
    fn analyzes_a_small_project_end_to_end() {
        let mut files: Vec<ProjectFile<'_>> = vec![
            file("pyproject.toml", FileCategory::Manifest),
            file("README.md", FileCategory::Documentation),
            file(".github/workflows/ci.yml", FileCategory::CiCd),
        ];
        let sources: Vec<String> = (0..12).map(|i| format!("app/m{i}.py")).collect();
        files.extend(sources.iter().map(|path| file(path, FileCategory::Source)));
        let contents: BTreeMap<String, String> = [
            ("pyproject.toml", "[project]\nname = \"app\"\nlicense = \"MIT\"\n"),
            (
                "README.md",
                "# App\n\nApp does useful things for people who need them. It reads the data you already have, finds the parts that matter, and writes a short summary that a whole team can read and discuss together.\n",
            ),
            (".github/workflows/ci.yml", "jobs:\n  lint:\n    steps:\n      - run: ruff check .\n"),
        ]
        .into_iter()
        .map(|(p, t)| (p.to_owned(), t.to_owned()))
        .collect();
        let requirements = [EnvironmentRequirement {
            kind: repodna_core::model::project::RequirementKind::Runtime,
            name: "Python".into(),
            version: Some(">=3.11".into()),
            source: "pyproject.toml".into(),
        }];
        let input = ProjectInput {
            files: &files,
            contents: &contents,
            dependencies: &[],
            requirements: &requirements,
            descriptions: &[],
        };
        let output = analyze(&input);
        assert_eq!(output.tests.test_files, 0);
        assert_eq!(output.builds.ci.len(), 1);
        assert_eq!(output.builds.requirements[0].name, "Python");
        assert!(
            output
                .builds
                .commands
                .iter()
                .all(|c| c.purpose != CommandPurpose::Test)
        );
        assert!(
            output
                .tests
                .commands
                .iter()
                .any(|c| c.command == "python -m unittest")
        );
        let rules: Vec<&str> = output.findings.iter().map(|f| f.rule.as_str()).collect();
        assert_eq!(
            rules,
            vec![
                "tests.none",
                "docs.setup-instructions",
                "docs.license-file-missing",
                "docs.optional-missing",
            ]
        );
        assert_eq!(output.findings[0].severity, Severity::Attention);
    }

    #[test]
    fn reports_missing_readme_license_and_ci() {
        let sources: Vec<String> = (0..25).map(|i| format!("src/m{i}.py")).collect();
        let mut files: Vec<ProjectFile<'_>> = sources
            .iter()
            .map(|path| file(path, FileCategory::Source))
            .collect();
        files.push(file("tests/test_a.py", FileCategory::Test));
        let contents = BTreeMap::new();
        let input = ProjectInput {
            files: &files,
            contents: &contents,
            dependencies: &[],
            requirements: &[],
            descriptions: &[],
        };
        let rules: Vec<String> = analyze(&input)
            .findings
            .into_iter()
            .map(|f| f.rule)
            .collect();
        for rule in ["build.no-ci", "docs.readme-missing", "docs.license-missing"] {
            assert!(
                rules.iter().any(|r| r == rule),
                "{rule} missing from {rules:?}"
            );
        }
        assert!(!rules.iter().any(|r| r == "tests.none"));
    }
}
