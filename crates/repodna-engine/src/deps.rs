//! The dependency stage and its findings.

use std::collections::{BTreeMap, HashMap};

use repodna_core::confidence::Confidence;
use repodna_core::evidence::Evidence;
use repodna_core::finding::{Finding, FindingCategory};
use repodna_core::metric::round4;
use repodna_core::model::architecture::ExternalImport;
use repodna_core::model::dependencies::{
    DependencyConcentration, DependencyReport, ManifestKind, StaleSignal,
};
use repodna_core::model::git::FileHistoryRecord;
use repodna_core::severity::Severity;
use repodna_core::time::Timestamp;
use repodna_dependencies::{DependencyAnalysis, DependencyFile, analyze, is_dependency_file};

/// Packages listed in the concentration view.
const MAX_CONCENTRATION: usize = 10;

/// Share of external-import files that import one package at which concentration is
/// reported.
pub const EXTERNAL_CONCENTRATION_SHARE: f64 = 0.5;

/// Ecosystems where committing a lockfile is the convention.
const LOCKFILE_ECOSYSTEMS: &[&str] = &["npm", "composer", "rubygems", "go", "pub"];

/// Parses every manifest and lockfile among the collected contents.
pub fn run_dependencies(contents: &BTreeMap<String, String>) -> DependencyAnalysis {
    let files: Vec<DependencyFile<'_>> = contents
        .iter()
        .filter(|(path, _)| is_dependency_file(path))
        .map(|(path, content)| DependencyFile { path, content })
        .collect();
    analyze(&files)
}

/// Adds import concentration (from architecture) and stale-manifest signals (from history).
pub fn complete_report(
    report: &mut DependencyReport,
    externals: &[ExternalImport],
    file_history: &[FileHistoryRecord],
    latest: Option<Timestamp>,
    stale_days: u32,
) {
    let total: u64 = externals.iter().map(|e| u64::from(e.importers)).sum();
    if total > 0 {
        report.concentration = externals
            .iter()
            .take(MAX_CONCENTRATION)
            .map(|external| DependencyConcentration {
                name: external.name.clone(),
                importers: external.importers,
                share: round4(f64::from(external.importers) / total as f64),
            })
            .collect();
    }
    let Some(latest) = latest else {
        return;
    };
    let history: HashMap<&str, &FileHistoryRecord> = file_history
        .iter()
        .map(|record| (record.path.as_str(), record))
        .collect();
    for manifest in report
        .manifests
        .iter()
        .filter(|m| m.kind == ManifestKind::Manifest)
    {
        let Some(record) = history.get(manifest.path.as_str()) else {
            continue;
        };
        let days = record.last_changed.days_until(latest);
        if days >= i64::from(stale_days) {
            report.stale_signals.push(StaleSignal {
                manifest: manifest.path.clone(),
                last_changed: record.last_changed,
                days_unchanged: days,
                description: format!(
                    "Not changed since {}, {days} days before the latest commit.",
                    record.last_changed.date_string()
                ),
            });
        }
    }
}

/// Derives dependency findings.
pub fn dependency_findings(
    report: &DependencyReport,
    concentration_threshold: f64,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for lockfile in report
        .lockfiles
        .iter()
        .filter(|l| !l.missing_from_lock.is_empty())
    {
        let sample: Vec<&str> = lockfile
            .missing_from_lock
            .iter()
            .take(10)
            .map(String::as_str)
            .collect();
        findings.push(
            Finding::new("dependencies.lock-mismatch", &lockfile.path, FindingCategory::Dependencies, Severity::Warning, Confidence::Medium, format!("{} does not lock every declared dependency", lockfile.path))
                .summary(format!("{} declared dependencies were not found in the lockfile: {}.", lockfile.missing_from_lock.len(), sample.join(", ")))
                .rationale("A lockfile that disagrees with its manifest means installs can resolve different versions on different machines.")
                .method("Direct registry dependencies of the manifests next to the lockfile are looked up among the locked packages.")
                .evidence(Evidence::file(&lockfile.path))
                .limitation("Name normalization differs between ecosystems; aliases and workspace-local packages can appear as missing.")
                .next_step("Regenerate the lockfile with the ecosystem's install or lock command and commit it.")
                .path(lockfile.path.clone()),
        );
    }
    let mut by_ecosystem: BTreeMap<&str, (bool, Vec<&str>)> = BTreeMap::new();
    for manifest in &report.manifests {
        let entry = by_ecosystem.entry(manifest.ecosystem.as_str()).or_default();
        match manifest.kind {
            ManifestKind::Lockfile => entry.0 = true,
            ManifestKind::Manifest => entry.1.push(manifest.path.as_str()),
        }
    }
    for (ecosystem, (has_lockfile, manifests)) in by_ecosystem {
        let declares = report
            .dependencies
            .iter()
            .any(|d| d.ecosystem == ecosystem && !d.internal);
        if !has_lockfile && declares && LOCKFILE_ECOSYSTEMS.contains(&ecosystem) {
            findings.push(
                Finding::new("dependencies.no-lockfile", ecosystem, FindingCategory::Dependencies, Severity::Info, Confidence::Medium, format!("No {ecosystem} lockfile is committed"))
                    .summary(format!("{} {ecosystem} manifest(s) declare dependencies, but no lockfile was found.", manifests.len()))
                    .rationale("Without a lockfile, each install can resolve newer versions than the ones that were tested.")
                    .method("Manifests and lockfiles are recognized by file name per ecosystem.")
                    .with_evidence(manifests.iter().take(5).map(|path| Evidence::file(*path)))
                    .limitation("Libraries sometimes deliberately omit lockfiles; lockfiles may also live outside the repository.")
                    .next_step("Commit the lockfile generated by the package manager, unless the project deliberately leaves versions open."),
            );
        }
    }
    if !report.duplicates.is_empty() {
        let sample: Vec<String> = report
            .duplicates
            .iter()
            .take(5)
            .map(|d| format!("{} ({})", d.name, d.versions.join(", ")))
            .collect();
        findings.push(
            Finding::new("dependencies.duplicate-versions", "repository", FindingCategory::Dependencies, Severity::Info, Confidence::High, format!("{} packages are locked at more than one version", report.duplicates.len()))
                .summary(format!("For example: {}.", sample.join("; ")))
                .rationale("Several versions of one package increase install size and can behave differently in different parts of the code.")
                .method("Lockfile entries are grouped by package name.")
                .with_evidence(report.duplicates.iter().take(5).map(|d| Evidence::file(&d.lockfile).with_note(format!("{}: {}", d.name, d.versions.join(", ")))))
                .limitation("Multiple versions are often unavoidable when dependencies require incompatible ranges.")
                .next_step("Check whether updating the dependents lets the package manager deduplicate the versions."),
        );
    }
    for signal in report.stale_signals.iter().take(5) {
        findings.push(
            Finding::new("dependencies.stale-manifest", &signal.manifest, FindingCategory::Dependencies, Severity::Info, Confidence::Medium, format!("{} has not changed in {} days", signal.manifest, signal.days_unchanged))
                .summary(signal.description.clone())
                .rationale("Dependencies declared long ago may have received security and bug fixes since.")
                .method("The latest commit that changed the manifest, compared with the latest commit.")
                .evidence(Evidence::file(&signal.manifest))
                .limitation("A manifest can be unchanged because its dependencies are still current; no version data was consulted.")
                .next_step("Review the declared versions against the latest releases of the dependencies.")
                .path(signal.manifest.clone()),
        );
    }
    if let Some(top) = report.concentration.first()
        && top.share >= concentration_threshold
        && top.importers >= 10
    {
        findings.push(
            Finding::new("dependencies.concentration", &top.name, FindingCategory::Dependencies, Severity::Info, Confidence::Medium, format!("{} is imported by {} files", top.name, top.importers))
                .summary(format!("{:.0}% of the files importing external packages import {}.", top.share * 100.0, top.name))
                .rationale("Code that depends heavily on one package is costly to migrate if the package changes direction or is abandoned.")
                .method("Import statements resolved to external packages, counted by importing file.")
                .evidence(Evidence::metric_with_threshold("dependencies.concentration.share", top.share, concentration_threshold, "ratio"))
                .limitation("Frameworks are meant to be used everywhere; high concentration is expected for them.")
                .next_step("Consider whether the dependency is wrapped behind a small internal interface."),
        );
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyzes_manifests_and_derives_findings() {
        let contents: BTreeMap<String, String> = [
            (
                "package.json",
                r#"{"name":"app","dependencies":{"react":"^19.0.0","zod":"^3.0.0"}}"#,
            ),
            ("src/app.ts", "ignored"),
        ]
        .into_iter()
        .map(|(p, t)| (p.to_owned(), t.to_owned()))
        .collect();
        let mut analysis = run_dependencies(&contents);
        assert_eq!(analysis.report.manifests.len(), 1);
        let externals = vec![ExternalImport {
            name: "react".into(),
            ecosystem: Some("npm".into()),
            importers: 12,
            modules: Vec::new(),
            samples: Vec::new(),
        }];
        let history = vec![FileHistoryRecord {
            path: "package.json".into(),
            commits: 1,
            authors: 1,
            insertions: 1,
            deletions: 0,
            first_seen: Timestamp::from_ymd(2019, 1, 1).unwrap(),
            last_changed: Timestamp::from_ymd(2019, 1, 1).unwrap(),
            recent_commits: 0,
            previous_paths: Vec::new(),
        }];
        complete_report(
            &mut analysis.report,
            &externals,
            &history,
            Timestamp::from_ymd(2024, 1, 1),
            730,
        );
        assert_eq!(analysis.report.concentration[0].share, 1.0);
        assert_eq!(analysis.report.stale_signals.len(), 1);
        let rules: Vec<String> = dependency_findings(&analysis.report, 0.3)
            .into_iter()
            .map(|f| f.rule)
            .collect();
        assert_eq!(
            rules,
            vec![
                "dependencies.no-lockfile",
                "dependencies.stale-manifest",
                "dependencies.concentration"
            ]
        );
    }
}
