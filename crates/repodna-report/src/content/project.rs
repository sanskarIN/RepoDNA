//! Tests, build information, documentation, and security signals.

use repodna_core::model::artifact::RepositoryDna;
use repodna_core::model::project::{
    CommandCandidate, CommandPurpose, DocCheckStatus, ExecutionResult, RequirementKind, TestKind,
};
use repodna_core::model::security::PatternCategory;

use super::{confidence, heading, not_analyzed, notes};
use crate::doc::{Block, Blocks, Inline, Table, code, plain, truncate};
use crate::sections::Section;
use crate::text::{percent, thousands};

fn purpose_label(purpose: CommandPurpose) -> &'static str {
    match purpose {
        CommandPurpose::Install => "Install",
        CommandPurpose::Build => "Build",
        CommandPurpose::Test => "Test",
        CommandPurpose::Run => "Run",
        CommandPurpose::Dev => "Development server",
        CommandPurpose::Lint => "Lint",
        CommandPurpose::Format => "Format",
        CommandPurpose::Bench => "Benchmark",
    }
}

fn commands_table(commands: &[CommandCandidate]) -> Table {
    let mut table = Table::new(&["Purpose", "Command", "Directory", "Detected in", "Status"]);
    let mut rows: Vec<_> = commands.iter().collect();
    table.omitted = truncate(&mut rows, 30);
    for command in rows {
        table.row(vec![
            plain(purpose_label(command.purpose)),
            code(command.command.clone()),
            plain(if command.working_directory.is_empty() {
                ".".to_owned()
            } else {
                command.working_directory.clone()
            }),
            code(command.source.clone()),
            plain(if command.verified {
                "Ran successfully"
            } else {
                "Detected, not run"
            }),
        ]);
    }
    table
}

fn executions(blocks: &mut Blocks, executions: &[ExecutionResult]) {
    if executions.is_empty() {
        return;
    }
    blocks.heading(3, "Command runs", None);
    let mut table = Table::new(&["Command", "Result", "Duration#", "Exit code#"]);
    for execution in executions {
        let result = if execution.cancelled {
            "Cancelled"
        } else if execution.timed_out {
            "Timed out"
        } else if execution.success {
            "Succeeded"
        } else {
            "Failed"
        };
        table.row(vec![
            code(execution.command.clone()),
            plain(result),
            plain(format!("{:.1} s", execution.duration_ms as f64 / 1000.0)),
            plain(
                execution
                    .exit_code
                    .map_or_else(|| "–".to_owned(), |c| c.to_string()),
            ),
        ]);
    }
    blocks.table(table);
    for execution in executions.iter().filter(|e| !e.output_tail.is_empty()) {
        blocks.rich(vec![
            Inline::Text("Last output lines of ".to_owned()),
            Inline::Code(execution.command.clone()),
            Inline::Text(" (secret-like values redacted):".to_owned()),
        ]);
        blocks
            .0
            .push(Block::Preformatted(execution.output_tail.join("\n")));
    }
}

fn test_kind(kind: TestKind) -> &'static str {
    match kind {
        TestKind::Unit => "Unit",
        TestKind::Integration => "Integration",
        TestKind::EndToEnd => "End-to-end",
        TestKind::Snapshot => "Snapshot",
        TestKind::Benchmark => "Benchmark",
    }
}

pub(super) fn tests(blocks: &mut Blocks, dna: &RepositoryDna) {
    heading(blocks, Section::Tests);
    let report = &dna.tests;
    if not_analyzed(blocks, report.status, &report.notes) {
        return;
    }
    blocks.stats(vec![
        ("Test files".to_owned(), thousands(report.test_files)),
        (
            "Source files with inline tests".to_owned(),
            thousands(report.inline_test_files),
        ),
        ("Test code lines".to_owned(), thousands(report.test_lines)),
        ("Source files".to_owned(), thousands(report.source_files)),
        ("Test code ratio".to_owned(), percent(report.test_ratio)),
    ]);
    if report.test_files == 0 && report.inline_test_files == 0 {
        blocks.text(
            "No test files were detected by path, file-name conventions, or inline test modules.",
        );
    }
    blocks.list(
        report
            .frameworks
            .iter()
            .map(|framework| {
                let mut item = vec![
                    Inline::Strong(framework.name.clone()),
                    Inline::Text(format!(
                        " test framework ({} confidence)",
                        framework.confidence.label().to_lowercase()
                    )),
                ];
                if let Some(path) = framework.evidence.first().and_then(|e| e.path()) {
                    item.push(Inline::Text(", detected from ".to_owned()));
                    item.push(Inline::Code(path.to_owned()));
                }
                item
            })
            .collect(),
    );
    if !report.kinds.is_empty() {
        let mut table = Table::new(&["Kind of test", "Files#"]);
        for kind in &report.kinds {
            table.row(vec![
                plain(test_kind(kind.kind)),
                plain(thousands(kind.files)),
            ]);
        }
        blocks.table(table);
    }
    if !report.test_directories.is_empty() {
        blocks.rich(vec![
            Inline::Text("Test directories: ".to_owned()),
            Inline::Code(report.test_directories.join(", ")),
        ]);
    }
    if !report.commands.is_empty() {
        blocks.heading(3, "How to run the tests", None);
        blocks.table(commands_table(&report.commands));
    }
    if report.ci_commands.is_empty() {
        if report.test_files > 0 {
            blocks.text("No CI configuration was found to run the tests.");
        }
    } else {
        blocks.heading(3, "Tests in CI", None);
        blocks.table(commands_table(&report.ci_commands));
    }
    if !report.coverage_artifacts.is_empty() {
        blocks.rich(vec![
            Inline::Text("Committed coverage reports: ".to_owned()),
            Inline::Code(report.coverage_artifacts.join(", ")),
        ]);
    }
    executions(blocks, &report.executions);
    notes(blocks, &report.notes);
}

fn requirement_kind(kind: RequirementKind) -> &'static str {
    match kind {
        RequirementKind::Runtime => "Runtime",
        RequirementKind::Toolchain => "Toolchain",
        RequirementKind::PackageManager => "Package manager",
        RequirementKind::Sdk => "SDK",
        RequirementKind::Container => "Container",
        RequirementKind::Platform => "Platform",
        RequirementKind::Service => "Service",
    }
}

pub(super) fn build(blocks: &mut Blocks, dna: &RepositoryDna) {
    heading(blocks, Section::Build);
    let report = &dna.builds;
    if not_analyzed(blocks, report.status, &report.notes) {
        return;
    }
    if report.systems.is_empty() {
        blocks.text("No build system was detected.");
    } else {
        let names: Vec<String> = report.systems.iter().map(|s| s.name.clone()).collect();
        blocks.text(format!(
            "Build systems and package managers: {}.",
            names.join(", ")
        ));
    }
    if !report.commands.is_empty() {
        blocks.heading(3, "Commands", None);
        blocks.table(commands_table(&report.commands));
        blocks.note("Commands are detected from manifests, task files, and README code blocks. They were not run unless command execution was explicitly enabled.");
    }
    if !report.requirements.is_empty() {
        blocks.heading(3, "Environment requirements", None);
        let mut table = Table::new(&["Kind", "Name", "Version", "Declared in"]);
        for requirement in &report.requirements {
            table.row(vec![
                plain(requirement_kind(requirement.kind)),
                plain(requirement.name.clone()),
                plain(
                    requirement
                        .version
                        .clone()
                        .unwrap_or_else(|| "–".to_owned()),
                ),
                code(requirement.source.clone()),
            ]);
        }
        blocks.table(table);
    }
    if report.ci.is_empty() {
        blocks.text("No CI/CD configuration was detected.");
    } else {
        blocks.heading(3, "CI/CD", None);
        let mut table = Table::new(&["Service", "File", "Jobs"]);
        for ci in &report.ci {
            table.row(vec![
                plain(ci.provider.clone()),
                code(ci.path.clone()),
                plain(if ci.jobs.is_empty() {
                    "–".to_owned()
                } else {
                    ci.jobs.join(", ")
                }),
            ]);
        }
        blocks.table(table);
    }
    if !report.containers.is_empty() {
        blocks.rich(vec![
            Inline::Text("Container definitions: ".to_owned()),
            Inline::Code(report.containers.join(", ")),
        ]);
    }
    if !report.configuration_files.is_empty() {
        blocks.heading(3, "Configuration files", None);
        let mut table = Table::new(&["File", "Purpose"]);
        let mut rows: Vec<_> = report.configuration_files.iter().collect();
        table.omitted = truncate(&mut rows, 30);
        for file in rows {
            table.row(vec![code(file.path.clone()), plain(file.purpose.clone())]);
        }
        blocks.table(table);
    }
    executions(blocks, &report.executions);
    notes(blocks, &report.notes);
}

pub(super) fn documentation(blocks: &mut Blocks, dna: &RepositoryDna) {
    heading(blocks, Section::Documentation);
    let report = &dna.docs;
    if not_analyzed(blocks, report.status, &report.notes) {
        return;
    }
    match &report.readme {
        Some(readme) => {
            let mut sections = Vec::new();
            if readme.has_installation {
                sections.push("installation");
            }
            if readme.has_usage {
                sections.push("usage");
            }
            blocks.rich(vec![
                Inline::Text("The README (".to_owned()),
                Inline::Code(readme.path.clone()),
                Inline::Text(format!(
                    ") has {} lines, {} words, {} headings, {} code blocks, {} links, and {} images{}.",
                    thousands(readme.lines),
                    thousands(readme.words),
                    readme.headings.len(),
                    readme.code_blocks,
                    readme.links,
                    readme.images,
                    if sections.is_empty() {
                        String::new()
                    } else {
                        format!(", with {} sections", sections.join(" and "))
                    }
                )),
            ]);
        }
        None => blocks.text("No README was found."),
    }
    if let Some(license) = &report.license {
        blocks.rich(vec![
            Inline::Text("License: ".to_owned()),
            Inline::Strong(
                license
                    .spdx
                    .clone()
                    .unwrap_or_else(|| "unrecognized text".to_owned()),
            ),
            Inline::Text(format!(
                " ({} confidence) from ",
                license.confidence.label().to_lowercase()
            )),
            Inline::Code(license.path.clone()),
            Inline::Text(".".to_owned()),
        ]);
    }
    let mut table = Table::new(&["Check", "Status", "Expected", "Evidence"]);
    for check in &report.checks {
        table.row(vec![
            plain(check.label.clone()),
            plain(match check.status {
                DocCheckStatus::Present => "Present",
                DocCheckStatus::Partial => "Partial",
                DocCheckStatus::NotDetected => "Not detected",
            }),
            plain(if check.optional {
                "Optional"
            } else {
                "Expected"
            }),
            code(
                check
                    .evidence
                    .first()
                    .map(|e| e.describe())
                    .unwrap_or_default(),
            ),
        ]);
    }
    blocks.table(table);
    blocks.text(format!(
        "{} documentation files with {} lines.",
        thousands(report.doc_files),
        thousands(report.doc_lines)
    ));
    if !report.doc_directories.is_empty() {
        blocks.rich(vec![
            Inline::Text("Documentation directories: ".to_owned()),
            Inline::Code(report.doc_directories.join(", ")),
        ]);
    }
    if !report.examples.is_empty() {
        blocks.rich(vec![
            Inline::Text("Examples: ".to_owned()),
            Inline::Code(report.examples.join(", ")),
        ]);
    }
    notes(blocks, &report.notes);
}

fn pattern_category(category: PatternCategory) -> &'static str {
    match category {
        PatternCategory::Code => "Code",
        PatternCategory::Configuration => "Configuration",
        PatternCategory::Workflow => "CI workflow",
        PatternCategory::Container => "Container",
    }
}

pub(super) fn security(blocks: &mut Blocks, dna: &RepositoryDna) {
    heading(blocks, Section::Security);
    let report = &dna.security;
    if not_analyzed(blocks, report.status, &report.notes) {
        return;
    }
    if !report.disclaimer.is_empty() {
        blocks.note(report.disclaimer.clone());
    }
    blocks.stats(vec![
        ("Files scanned".to_owned(), thousands(report.files_scanned)),
        (
            "Secret candidates".to_owned(),
            thousands(report.secrets.len() as u64),
        ),
        (
            "Risky-pattern candidates".to_owned(),
            thousands(report.patterns.len() as u64),
        ),
        (
            "Permission signals".to_owned(),
            thousands(report.permissions.len() as u64),
        ),
    ]);
    if !report.secrets.is_empty() {
        blocks.heading(3, "Secret candidates", None);
        blocks.text("Values are never stored or shown. The fingerprint is a keyed hash that lets you recognize the same value across reports of this repository; it cannot be reversed.");
        let mut table = Table::new(&[
            "Rule",
            "Location",
            "Confidence",
            "Test or example",
            "Fingerprint",
        ]);
        let mut rows: Vec<_> = report.secrets.iter().collect();
        table.omitted = truncate(&mut rows, 50);
        for secret in rows {
            table.row(vec![
                plain(secret.description.clone()),
                code(format!("{}:{}", secret.path, secret.line)),
                plain(confidence(secret.confidence)),
                plain(if secret.in_test_or_example {
                    "Yes"
                } else {
                    "No"
                }),
                code(secret.fingerprint.clone()),
            ]);
        }
        blocks.table(table);
    }
    if !report.patterns.is_empty() {
        blocks.heading(3, "Risky patterns", None);
        let mut table = Table::new(&[
            "Pattern",
            "Area",
            "Location",
            "Confidence",
            "Recommendation",
        ]);
        let mut rows: Vec<_> = report.patterns.iter().collect();
        table.omitted = truncate(&mut rows, 50);
        for pattern in rows {
            table.row(vec![
                plain(pattern.description.clone()),
                plain(pattern_category(pattern.category)),
                code(match pattern.line {
                    Some(line) => format!("{}:{line}", pattern.path),
                    None => pattern.path.clone(),
                }),
                plain(confidence(pattern.confidence)),
                plain(pattern.recommendation.clone()),
            ]);
        }
        blocks.table(table);
    }
    if !report.permissions.is_empty() {
        blocks.heading(3, "File permissions", None);
        let mut table = Table::new(&["File", "Mode", "Signal"]);
        for permission in &report.permissions {
            table.row(vec![
                code(permission.path.clone()),
                code(permission.mode.clone()),
                plain(permission.issue.clone()),
            ]);
        }
        blocks.table(table);
    }
    if report.secrets.is_empty() && report.patterns.is_empty() && report.permissions.is_empty() {
        blocks.text("No candidates were found by the rules that ran. This does not mean the code is free of vulnerabilities.");
    }
    blocks.text(format!(
        "{} detection rules were applied.",
        report.rules.len()
    ));
    if !report.advisories.note.is_empty() {
        blocks.note(report.advisories.note.clone());
    }
    notes(blocks, &report.notes);
}

#[cfg(test)]
mod tests {
    use super::super::{ContentOptions, section};
    use crate::markdown::render;
    use crate::sections::Section;
    use repodna_core::confidence::Confidence;
    use repodna_core::model::SectionStatus;
    use repodna_core::model::artifact::RepositoryDna;
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::metadata::AnalysisMetadata;
    use repodna_core::model::security::SecretCandidate;

    #[test]
    fn never_shows_secret_values_and_labels_unverified_commands() {
        let mut dna =
            RepositoryDna::new(RepositoryIdentity::default(), AnalysisMetadata::default());
        dna.security.status = SectionStatus::Analyzed;
        dna.security.secrets = vec![SecretCandidate {
            id: "s1".into(),
            rule: "aws-access-key".into(),
            description: "AWS access key ID".into(),
            path: "config/prod.env".into(),
            line: 4,
            confidence: Confidence::High,
            fingerprint: "a1b2c3d4e5f6".into(),
            in_test_or_example: false,
        }];
        let markdown = render(&section(&dna, Section::Security, ContentOptions::default()));
        assert!(
            markdown.contains(
                "| AWS access key ID | `config/prod.env:4` | High | No | `a1b2c3d4e5f6` |"
            )
        );
        assert!(markdown.contains("Values are never stored or shown."));

        dna.builds.status = SectionStatus::Analyzed;
        dna.builds.commands = vec![repodna_core::model::project::CommandCandidate {
            command: "cargo build".into(),
            purpose: repodna_core::model::project::CommandPurpose::Build,
            working_directory: String::new(),
            source: "Cargo.toml".into(),
            verified: false,
            evidence: Vec::new(),
        }];
        let build = render(&section(&dna, Section::Build, ContentOptions::default()));
        assert!(build.contains("| Build | `cargo build` | . | `Cargo.toml` | Detected, not run |"));
        assert!(build.contains("No CI/CD configuration was detected."));
    }
}
