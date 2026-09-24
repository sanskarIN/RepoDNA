//! Adapters from the file pass to the architecture, quality, project, and security analyzers.

use std::collections::{BTreeSet, HashMap};

use repodna_architecture::entrypoints::detect_entrypoints;
use repodna_architecture::{ArchitectureInput, ArchitectureOutput, SourceFile};
use repodna_core::cancel::{CancellationToken, Cancelled};
use repodna_core::config::{Config, Stage, StageSet};
use repodna_core::finding::Finding;
use repodna_core::hash::sha256;
use repodna_core::model::git::{GitReport, Hotspot};
use repodna_core::model::security::SecurityReport;
use repodna_core::model::structure::{Entrypoint, FileCategory};
use repodna_core::time::Timestamp;
use repodna_dependencies::DependencyAnalysis;
use repodna_project::{ProjectFile, ProjectInput, ProjectOutput};
use repodna_quality::deadcode::UsageContext;
use repodna_quality::hotspots::{HotspotInput, HotspotSettings, rank_hotspots};
use repodna_quality::{QualityFile, QualityInput, QualityOutput};
use repodna_security::{build_report, permission_signals, security_findings};

use crate::scan::ScanResult;

/// Hotspots listed in the Git section.
pub const MAX_HOTSPOTS: usize = 50;

/// Share of unresolved imports above which "nothing imports it" is not trusted.
const RELIABLE_UNRESOLVED_SHARE: f64 = 0.3;

/// Declared dependencies as `(ecosystem, name)`.
pub fn declared_dependencies(dependencies: &DependencyAnalysis) -> Vec<(String, String)> {
    dependencies
        .report
        .dependencies
        .iter()
        .map(|d| (d.ecosystem.clone(), d.name.clone()))
        .collect()
}

/// The scanned files as architecture inputs, in scan order.
pub fn source_files(scan: &ScanResult) -> Vec<SourceFile<'_>> {
    scan.files
        .iter()
        .map(|file| SourceFile {
            path: &file.record.path,
            language: file.record.language.as_deref(),
            code_lines: file.record.code_lines(),
            first_party: file.first_party_code() && file.record.language.is_some(),
            test: file.record.category == FileCategory::Test,
            analysis: file.analysis.as_ref(),
        })
        .collect()
}

/// Detects entrypoints without running the rest of the architecture analysis.
pub fn entrypoints(scan: &ScanResult, dependencies: &DependencyAnalysis) -> Vec<Entrypoint> {
    detect_entrypoints(&source_files(scan), &dependencies.entrypoints)
}

/// Runs architecture analysis over the scanned files.
pub fn run_architecture(
    scan: &ScanResult,
    dependencies: &DependencyAnalysis,
    config: &Config,
    cancel: &CancellationToken,
) -> Result<ArchitectureOutput, Cancelled> {
    let files = source_files(scan);
    let declared = declared_dependencies(dependencies);
    repodna_architecture::analyze(
        &ArchitectureInput {
            files: &files,
            packages: &dependencies.packages,
            workspace: dependencies.workspace.as_ref(),
            declared_entrypoints: &dependencies.entrypoints,
            declared_dependencies: &declared,
            max_file_edges: usize::try_from(config.analysis.max_file_edges).unwrap_or(usize::MAX),
        },
        cancel,
    )
}

/// Runs quality analysis over first-party code.
pub fn run_quality(
    scan: &ScanResult,
    architecture: Option<&ArchitectureOutput>,
    git: Option<&GitReport>,
    config: &Config,
    stages: StageSet,
    cancel: &CancellationToken,
) -> Result<QualityOutput, Cancelled> {
    let indices: Vec<usize> = (0..scan.files.len())
        .filter(|&i| scan.files[i].first_party_code())
        .collect();
    let files: Vec<QualityFile<'_>> = indices
        .iter()
        .map(|&i| {
            let file = &scan.files[i];
            QualityFile {
                path: &file.record.path,
                language: file.record.language.as_deref(),
                test: file.record.category == FileCategory::Test,
                lines: file.record.lines.unwrap_or_default(),
                analysis: file.analysis.as_ref(),
                tokens: file.tokens.as_ref(),
            }
        })
        .collect();
    let dependents: Vec<u32> = indices
        .iter()
        .map(|&i| architecture.map_or(0, |a| a.file_dependents.get(i).copied().unwrap_or(0)))
        .collect();
    let module_declared: Vec<bool> = indices
        .iter()
        .map(|&i| architecture.is_some_and(|a| a.module_declared.get(i).copied().unwrap_or(false)))
        .collect();
    let history: HashMap<&str, Timestamp> = git
        .map(|g| {
            g.file_history
                .iter()
                .map(|r| (r.path.as_str(), r.last_changed))
                .collect()
        })
        .unwrap_or_default();
    let last_changed: Vec<Option<Timestamp>> = indices
        .iter()
        .map(|&i| history.get(scan.files[i].record.path.as_str()).copied())
        .collect();
    let entrypoints: BTreeSet<String> = architecture
        .map(|a| a.entrypoints.iter().map(|e| e.path.clone()).collect())
        .unwrap_or_default();
    let imports_reliable = architecture.is_some_and(|a| {
        let attempted = a.report.resolved_imports + a.report.unresolved_imports;
        attempted > 0
            && (a.report.unresolved_imports as f64) < RELIABLE_UNRESOLVED_SHARE * attempted as f64
    });
    repodna_quality::analyze(
        &QualityInput {
            files: &files,
            thresholds: &config.thresholds,
            usage: UsageContext {
                dependents: &dependents,
                module_declared: &module_declared,
                entrypoints: &entrypoints,
                imports_reliable,
                last_changed: &last_changed,
                reference_time: git
                    .and_then(|g| g.last_commit.as_ref())
                    .map(|c| c.timestamp),
            },
            duplication: stages.contains(Stage::Duplication),
            similarity: stages.contains(Stage::Similarity),
        },
        cancel,
    )
}

/// Ranks hotspots from file history, size, complexity, and importers.
pub fn hotspots(
    scan: &ScanResult,
    git: &GitReport,
    architecture: Option<&ArchitectureOutput>,
    config: &Config,
) -> (Vec<Hotspot>, usize) {
    let index: HashMap<&str, usize> = scan
        .files
        .iter()
        .enumerate()
        .map(|(i, file)| (file.record.path.as_str(), i))
        .collect();
    let inputs: Vec<HotspotInput<'_>> = git
        .file_history
        .iter()
        .filter_map(|record| {
            let i = *index.get(record.path.as_str())?;
            let file = &scan.files[i];
            file.first_party_code().then(|| HotspotInput {
                path: &record.path,
                commits: record.commits,
                authors: record.authors,
                churn: record.insertions + record.deletions,
                recent_commits: record.recent_commits,
                lines: file.record.code_lines(),
                complexity: file
                    .record
                    .analysis
                    .as_ref()
                    .map_or(0, |a| a.cyclomatic_max),
                dependents: architecture
                    .map_or(0, |a| a.file_dependents.get(i).copied().unwrap_or(0)),
            })
        })
        .collect();
    let settings = HotspotSettings {
        min_commits: config.thresholds.hotspot_min_commits,
        recent_days: config.thresholds.recent_days,
        limit: MAX_HOTSPOTS,
    };
    let candidates = inputs
        .iter()
        .filter(|input| input.commits >= settings.min_commits && input.lines > 0)
        .count();
    (rank_hotspots(&inputs, &settings), candidates)
}

/// Runs project analysis (tests, build, CI, environment, documentation).
pub fn run_project(scan: &ScanResult, dependencies: &DependencyAnalysis) -> ProjectOutput {
    let files: Vec<ProjectFile<'_>> = scan
        .files
        .iter()
        .map(|file| ProjectFile {
            path: &file.record.path,
            category: file.record.category,
            language: file.record.language.as_deref(),
            code_lines: file.record.code_lines(),
            total_lines: file.record.lines.map_or(0, |l| l.total),
            inline_tests: file.inline_tests,
        })
        .collect();
    let declared = declared_dependencies(dependencies);
    repodna_project::analyze(&ProjectInput {
        files: &files,
        contents: &scan.contents,
        dependencies: &declared,
        requirements: &dependencies.requirements,
        descriptions: &dependencies.descriptions,
    })
}

/// The key for secret fingerprints: a digest of every path and full content hash, so it is
/// stable for the same content but cannot be derived from a report (which holds only short
/// hashes).
pub fn fingerprint_key(scan: &ScanResult) -> [u8; 32] {
    let mut material = Vec::new();
    for file in &scan.files {
        if let Some(hash) = &file.sha256 {
            material.extend_from_slice(file.record.path.as_bytes());
            material.push(0);
            material.extend_from_slice(hash);
        }
    }
    sha256(&material)
}

/// Finalizes secret fingerprints and assembles the security section.
pub fn run_security(scan: &mut ScanResult) -> (SecurityReport, Vec<Finding>) {
    let key = fingerprint_key(scan);
    let secrets = std::mem::take(&mut scan.secrets)
        .into_iter()
        .map(|pending| pending.finalize(&key))
        .collect();
    let patterns = std::mem::take(&mut scan.patterns);
    let permissions = permission_signals(
        scan.files
            .iter()
            .filter_map(|file| file.mode.map(|mode| (file.record.path.as_str(), mode))),
    );
    let report = build_report(secrets, patterns, permissions, scan.security_files_scanned);
    let findings = security_findings(&report);
    (report, findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Progress;
    use crate::scan::{ScanOptions, scan_files};
    use repodna_parser::LanguageRegistry;
    use repodna_testkit::write_tree;

    #[test]
    fn feeds_every_analyzer_from_one_scan() {
        let dir = tempfile::tempdir().unwrap();
        write_tree(
            dir.path(),
            &[
                (
                    "package.json",
                    r#"{"name":"web","dependencies":{"react":"^19.0.0"}}"#,
                ),
                (
                    "src/main.ts",
                    "import { a } from './a';\nimport React from 'react';\nconsole.log(a);\n",
                ),
                ("src/a.ts", "export const a = 1;\n"),
                (
                    "src/a.test.ts",
                    "import { a } from './a';\ntest('a', () => {});\n",
                ),
                ("README.md", "# Web\n"),
            ],
        );
        let config = Config::default();
        let stages = StageSet::of(&Stage::ALL);
        let options = ScanOptions {
            config: &config,
            stages,
            registry: LanguageRegistry::builtin(),
            progress: &Progress::default(),
            cache: None,
        };
        let cancel = CancellationToken::new();
        let mut scan = scan_files(dir.path(), &options, &cancel).unwrap();
        let dependencies = crate::deps::run_dependencies(&scan.contents);
        let architecture = run_architecture(&scan, &dependencies, &config, &cancel).unwrap();
        assert_eq!(architecture.report.resolved_imports, 2);
        assert_eq!(architecture.report.external[0].name, "react");
        let quality =
            run_quality(&scan, Some(&architecture), None, &config, stages, &cancel).unwrap();
        assert_eq!(
            quality.report.status,
            repodna_core::model::SectionStatus::Analyzed
        );
        let project = run_project(&scan, &dependencies);
        assert_eq!(project.tests.test_files, 1);
        let key = fingerprint_key(&scan);
        assert_eq!(key, fingerprint_key(&scan));
        let (security, _) = run_security(&mut scan);
        assert!(security.files_scanned >= 4);
    }
}
