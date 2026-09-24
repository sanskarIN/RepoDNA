//! The evolution stage: snapshots, evolution analysis, and recent changes.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use repodna_core::cancel::CancellationToken;
use repodna_core::config::Config;
use repodna_core::model::dependencies::{DependencyReport, ManifestKind};
use repodna_core::model::evolution::Snapshot;
use repodna_core::model::insights::RecentChanges;
use repodna_core::time::SECONDS_PER_DAY;
use repodna_dependencies::{DependencyFile, analyze};
use repodna_evolution::recent::{Declaration, diff_dependencies, recent_changes};
use repodna_evolution::snapshots::{build_snapshot, plan_snapshots};
use repodna_evolution::{EvolutionInput, EvolutionOutput};
use repodna_git::{BlobLimits, GitError, GitRunner, list_tree, read_blobs};
use repodna_parser::LanguageRegistry;

use crate::history::GitStage;

/// Results of the evolution stage.
#[derive(Debug)]
pub struct TimelineStage {
    /// Evolution section and findings.
    pub evolution: EvolutionOutput,
    /// What changed in the recent window.
    pub recent: Option<RecentChanges>,
    /// Problems that did not stop the stage.
    pub notes: Vec<String>,
}

fn declarations(report: &DependencyReport) -> Vec<Declaration> {
    report
        .dependencies
        .iter()
        .filter(|dependency| !dependency.internal)
        .flat_map(|dependency| {
            dependency.manifests.iter().map(|manifest| Declaration {
                ecosystem: dependency.ecosystem.clone(),
                manifest: manifest.clone(),
                name: dependency.name.clone(),
                requirement: dependency.requirement.clone(),
            })
        })
        .collect()
}

/// Runs the evolution stage.
#[allow(clippy::too_many_arguments)]
pub fn run_timeline(
    git: &GitRunner,
    root: &Path,
    stage: &GitStage,
    modules: &[(String, Vec<String>)],
    current_files: &HashSet<&str>,
    current_dependencies: &DependencyReport,
    config: &Config,
    registry: &LanguageRegistry,
    cancel: &CancellationToken,
) -> Result<TimelineStage, GitError> {
    let mut notes = Vec::new();
    let plans = plan_snapshots(
        &stage.history,
        &stage.releases,
        usize::try_from(config.analysis.snapshots).unwrap_or(usize::MAX),
    );
    let mut snapshots: Vec<Snapshot> = Vec::with_capacity(plans.len());
    for plan in &plans {
        match list_tree(git, root, &plan.revision, cancel) {
            Ok(entries) => snapshots.push(build_snapshot(plan, &entries, registry)),
            Err(GitError::Cancelled) => return Err(GitError::Cancelled),
            Err(error) => notes.push(format!(
                "Snapshot {} could not be read: {error}",
                plan.label
            )),
        }
    }
    let evolution = repodna_evolution::analyze(EvolutionInput {
        history: &stage.history,
        releases: &stage.releases,
        dormant_periods: &stage.report.dormant_periods,
        file_history: &stage.report.file_history,
        snapshots,
        modules,
    });

    let mut recent = recent_changes(&stage.history, config.thresholds.recent_days, current_files);
    if let (Some(recent), Some(latest)) = (recent.as_mut(), stage.history.commits.first()) {
        let window_start =
            latest.timestamp - i64::from(config.thresholds.recent_days) * SECONDS_PER_DAY;
        let base = stage
            .history
            .commits
            .iter()
            .find(|commit| commit.timestamp <= window_start);
        if let Some(base) = base {
            let manifests: Vec<String> = current_dependencies
                .manifests
                .iter()
                .filter(|m| m.kind == ManifestKind::Manifest)
                .map(|m| m.path.clone())
                .collect();
            match read_blobs(
                git,
                root,
                &base.hash,
                &manifests,
                BlobLimits::default(),
                cancel,
            ) {
                Ok(blobs) => {
                    let texts: BTreeMap<String, String> = blobs
                        .into_iter()
                        .map(|(path, bytes)| (path, String::from_utf8_lossy(&bytes).into_owned()))
                        .collect();
                    let files: Vec<DependencyFile<'_>> = texts
                        .iter()
                        .map(|(path, content)| DependencyFile { path, content })
                        .collect();
                    let before = analyze(&files);
                    recent.dependency_changes = diff_dependencies(
                        &declarations(&before.report),
                        &declarations(current_dependencies),
                    );
                }
                Err(GitError::Cancelled) => return Err(GitError::Cancelled),
                Err(error) => notes.push(format!("Earlier manifests could not be read: {error}")),
            }
        }
    }
    Ok(TimelineStage {
        evolution,
        recent,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::run_git_stage;
    use repodna_core::time::Timestamp;
    use repodna_testkit::{GitRepo, git_available};
    use std::collections::BTreeSet;

    #[test]
    fn builds_snapshots_and_recent_dependency_changes() {
        if !git_available() {
            return;
        }
        let repo = GitRepo::new();
        repo.write(
            "package.json",
            r#"{"name":"app","dependencies":{"react":"^18.0.0"}}"#,
        );
        repo.write("src/a.ts", "export const a = 1;\n");
        repo.commit("initial", "Ana", "ana@example.test", "2023-01-01T10:00:00Z");
        repo.write(
            "package.json",
            r#"{"name":"app","dependencies":{"react":"^19.0.0","zod":"^3.0.0"}}"#,
        );
        repo.write("src/b.ts", "export const b = 2;\n");
        repo.commit("upgrade", "Bo", "bo@example.test", "2024-05-01T10:00:00Z");
        let git = GitRunner::detect().unwrap();
        let cancel = CancellationToken::new();
        let files: BTreeSet<String> = ["package.json", "src/a.ts", "src/b.ts"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let config = Config::default();
        let stage = run_git_stage(
            &git,
            repo.path(),
            &files,
            &config,
            Timestamp::from_ymd(2024, 6, 1).unwrap(),
            &cancel,
        )
        .unwrap();
        let contents: BTreeMap<String, String> = [(
            "package.json".to_owned(),
            std::fs::read_to_string(repo.path().join("package.json")).unwrap(),
        )]
        .into();
        let current = crate::deps::run_dependencies(&contents);
        let current_files: HashSet<&str> = files.iter().map(String::as_str).collect();
        let modules = vec![(
            "src".to_owned(),
            vec!["src/a.ts".to_owned(), "src/b.ts".to_owned()],
        )];
        let timeline = run_timeline(
            &git,
            repo.path(),
            &stage,
            &modules,
            &current_files,
            &current.report,
            &config,
            LanguageRegistry::builtin(),
            &cancel,
        )
        .unwrap();
        let report = &timeline.evolution.report;
        assert_eq!(report.snapshots.len(), 2);
        assert_eq!(report.snapshots[0].files, 2);
        assert_eq!(report.snapshots[1].files, 3);
        assert_eq!(report.module_ages.len(), 1);
        let recent = timeline.recent.unwrap();
        assert_eq!(recent.commits, 1);
        let changes: Vec<(&str, repodna_core::model::insights::ChangeKind)> = recent
            .dependency_changes
            .iter()
            .map(|c| (c.name.as_str(), c.change))
            .collect();
        assert_eq!(
            changes,
            vec![
                ("react", repodna_core::model::insights::ChangeKind::Changed),
                ("zod", repodna_core::model::insights::ChangeKind::Added)
            ]
        );
        assert!(timeline.notes.is_empty());
    }
}
