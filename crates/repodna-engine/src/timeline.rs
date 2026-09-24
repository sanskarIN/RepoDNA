//! The evolution stage: snapshots, historical architecture, evolution analysis, and recent
//! changes.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use repodna_architecture::{ArchitectureInput, SourceFile};
use repodna_core::cancel::CancellationToken;
use repodna_core::config::Config;
use repodna_core::model::dependencies::{DependencyReport, ManifestKind};
use repodna_core::model::evolution::{
    Snapshot, SnapshotArchitecture, SnapshotDirectory, SnapshotEdge,
};
use repodna_core::model::insights::RecentChanges;
use repodna_core::model::languages::ParserCapability;
use repodna_core::model::structure::FileCategory;
use repodna_core::time::SECONDS_PER_DAY;
use repodna_dependencies::{DependencyFile, analyze, is_dependency_file};
use repodna_discovery::{ClassificationOverrides, classify};
use repodna_evolution::recent::{Declaration, diff_dependencies, recent_changes};
use repodna_evolution::snapshots::{build_snapshot, plan_snapshots};
use repodna_evolution::{EvolutionInput, EvolutionOutput};
use repodna_git::{BlobLimits, GitError, GitRunner, TreeEntry, list_tree, read_blobs};
use repodna_parser::{FileAnalysis, LanguageRegistry, LanguageSpec, analyze_source};

use crate::history::GitStage;

/// Source files read per historical snapshot (in path order).
const HISTORICAL_MAX_FILES: usize = 5_000;
/// Files larger than this are not read at historical revisions.
const HISTORICAL_MAX_FILE_BYTES: u64 = 512 * 1024;
/// Bytes read per historical snapshot.
const HISTORICAL_MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
/// Module edges kept per snapshot (heaviest first).
const HISTORICAL_MAX_EDGES: usize = 500;

/// Everything the evolution stage needs.
#[derive(Debug, Clone, Copy)]
pub struct TimelineInput<'a> {
    /// Git runner.
    pub git: &'a GitRunner,
    /// Working tree root.
    pub root: &'a Path,
    /// Results of the Git stage.
    pub stage: &'a GitStage,
    /// Current modules as `(module id, member paths)`.
    pub modules: &'a [(String, Vec<String>)],
    /// Paths in the current tree.
    pub current_files: &'a HashSet<&'a str>,
    /// The current dependency section.
    pub current_dependencies: &'a DependencyReport,
    /// Configuration.
    pub config: &'a Config,
    /// Language registry.
    pub registry: &'a LanguageRegistry,
    /// Reconstruct module structure and dependencies at every snapshot (deep profile).
    pub historical_architecture: bool,
}

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

/// A first-party source file selected at a historical revision.
struct HistoricalSource<'a> {
    entry: &'a TreeEntry,
    spec: &'a LanguageSpec,
    test: bool,
}

/// Reconstructs the module structure and the module dependencies at `revision` by reading
/// its manifests and first-party source files and running the architecture analysis on them.
pub fn historical_architecture(
    git: &GitRunner,
    root: &Path,
    revision: &str,
    entries: &[TreeEntry],
    registry: &LanguageRegistry,
    cancel: &CancellationToken,
) -> Result<SnapshotArchitecture, GitError> {
    let overrides = ClassificationOverrides::default();
    let mut manifests: Vec<String> = Vec::new();
    let mut sources: Vec<HistoricalSource<'_>> = Vec::new();
    for entry in entries
        .iter()
        .filter(|entry| entry.mode != "120000" && entry.size <= HISTORICAL_MAX_FILE_BYTES)
    {
        if is_dependency_file(&entry.path) {
            manifests.push(entry.path.clone());
            continue;
        }
        let Some(spec) = registry.detect_path(&entry.path) else {
            continue;
        };
        if spec.capability() != ParserCapability::Lexical {
            continue;
        }
        let classification = classify(&entry.path, Some(spec.kind), &overrides);
        let code = matches!(
            classification.category,
            FileCategory::Source | FileCategory::Test
        ) && !classification.vendored
            && !classification.generated;
        if code && sources.len() < HISTORICAL_MAX_FILES {
            sources.push(HistoricalSource {
                entry,
                spec,
                test: classification.category == FileCategory::Test,
            });
        }
    }
    let mut paths = manifests.clone();
    paths.extend(sources.iter().map(|source| source.entry.path.clone()));
    let texts: HashMap<String, String> = read_blobs(
        git,
        root,
        revision,
        &paths,
        BlobLimits {
            max_blob_bytes: HISTORICAL_MAX_FILE_BYTES,
            max_total_bytes: HISTORICAL_MAX_TOTAL_BYTES,
        },
        cancel,
    )?
    .into_iter()
    .map(|(path, bytes)| (path, String::from_utf8_lossy(&bytes).into_owned()))
    .collect();

    let dependency_files: Vec<DependencyFile<'_>> = manifests
        .iter()
        .filter_map(|path| {
            texts
                .get(path)
                .map(|content| DependencyFile { path, content })
        })
        .collect();
    let dependencies = analyze(&dependency_files);
    let analyses: Vec<Option<FileAnalysis>> = sources
        .iter()
        .map(|source| {
            texts
                .get(&source.entry.path)
                .map(|text| analyze_source(source.spec, text))
        })
        .collect();
    let files: Vec<SourceFile<'_>> = sources
        .iter()
        .zip(&analyses)
        .map(|(source, analysis)| SourceFile {
            path: &source.entry.path,
            language: Some(source.spec.id.as_str()),
            code_lines: analysis.as_ref().map_or(0, |a| a.lines.code),
            first_party: true,
            test: source.test,
            analysis: analysis.as_ref(),
        })
        .collect();
    let declared: Vec<(String, String)> = dependencies
        .report
        .dependencies
        .iter()
        .map(|d| (d.ecosystem.clone(), d.name.clone()))
        .collect();
    let output = repodna_architecture::analyze(
        &ArchitectureInput {
            files: &files,
            packages: &dependencies.packages,
            workspace: dependencies.workspace.as_ref(),
            declared_entrypoints: &dependencies.entrypoints,
            declared_dependencies: &declared,
            max_file_edges: 0,
        },
        cancel,
    )
    .map_err(|_| GitError::Cancelled)?;

    let mut bytes: HashMap<&str, u64> = HashMap::new();
    for (source, module) in sources.iter().zip(&output.module_of) {
        if let Some(module) = module {
            *bytes.entry(module.as_str()).or_default() += source.entry.size;
        }
    }
    let path_of: HashMap<&str, &str> = output
        .report
        .modules
        .iter()
        .map(|module| (module.id.as_str(), module.path.as_str()))
        .collect();
    let modules = output
        .report
        .modules
        .iter()
        .map(|module| SnapshotDirectory {
            path: module.path.clone(),
            files: module.files,
            bytes: bytes.get(module.id.as_str()).copied().unwrap_or(0),
        })
        .collect();
    let mut edges: Vec<SnapshotEdge> = output
        .report
        .module_edges
        .iter()
        .map(|edge| SnapshotEdge {
            from: path_of
                .get(edge.from.as_str())
                .map_or_else(|| edge.from.clone(), |path| (*path).to_owned()),
            to: path_of
                .get(edge.to.as_str())
                .map_or_else(|| edge.to.clone(), |path| (*path).to_owned()),
            weight: edge.weight,
        })
        .collect();
    edges.sort_by(|a, b| {
        b.weight
            .cmp(&a.weight)
            .then_with(|| a.from.cmp(&b.from))
            .then_with(|| a.to.cmp(&b.to))
    });
    edges.truncate(HISTORICAL_MAX_EDGES);
    Ok(SnapshotArchitecture { modules, edges })
}

/// Runs the evolution stage.
pub fn run_timeline(
    input: &TimelineInput<'_>,
    cancel: &CancellationToken,
) -> Result<TimelineStage, GitError> {
    let TimelineInput {
        git,
        root,
        stage,
        modules,
        current_files,
        current_dependencies,
        config,
        registry,
        historical_architecture: with_architecture,
    } = *input;
    let mut notes = Vec::new();
    let plans = plan_snapshots(
        &stage.history,
        &stage.releases,
        usize::try_from(config.analysis.snapshots).unwrap_or(usize::MAX),
    );
    let mut snapshots: Vec<Snapshot> = Vec::with_capacity(plans.len());
    for plan in &plans {
        match list_tree(git, root, &plan.revision, cancel) {
            Ok(entries) => {
                let mut snapshot = build_snapshot(plan, &entries, registry);
                if with_architecture {
                    match historical_architecture(
                        git,
                        root,
                        &plan.revision,
                        &entries,
                        registry,
                        cancel,
                    ) {
                        Ok(architecture) => snapshot.architecture = Some(architecture),
                        Err(GitError::Cancelled) => return Err(GitError::Cancelled),
                        Err(error) => notes.push(format!(
                            "Architecture at snapshot {} could not be reconstructed: {error}",
                            plan.label
                        )),
                    }
                }
                snapshots.push(snapshot);
            }
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
            &TimelineInput {
                git: &git,
                root: repo.path(),
                stage: &stage,
                modules: &modules,
                current_files: &current_files,
                current_dependencies: &current.report,
                config: &config,
                registry: LanguageRegistry::builtin(),
                historical_architecture: false,
            },
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
        assert!(report.snapshots.iter().all(|s| s.architecture.is_none()));
    }

    #[test]
    fn reconstructs_architecture_at_each_snapshot() {
        if !git_available() {
            return;
        }
        let repo = GitRepo::new();
        repo.write("lib/util.ts", "export const one = 1;\n");
        repo.write("app/main.ts", "export const main = 1;\n");
        repo.commit("initial", "Ana", "ana@example.test", "2023-01-01T10:00:00Z");
        repo.write(
            "app/main.ts",
            "import { one } from '../lib/util';\nexport const main = one;\n",
        );
        repo.commit(
            "use util",
            "Ana",
            "ana@example.test",
            "2024-01-01T10:00:00Z",
        );
        let git = GitRunner::detect().unwrap();
        let cancel = CancellationToken::new();
        let files: BTreeSet<String> = ["app/main.ts", "lib/util.ts"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let config = Config::default();
        let stage = run_git_stage(
            &git,
            repo.path(),
            &files,
            &config,
            Timestamp::from_ymd(2024, 2, 1).unwrap(),
            &cancel,
        )
        .unwrap();
        let current_files: HashSet<&str> = files.iter().map(String::as_str).collect();
        let timeline = run_timeline(
            &TimelineInput {
                git: &git,
                root: repo.path(),
                stage: &stage,
                modules: &[],
                current_files: &current_files,
                current_dependencies: &DependencyReport::default(),
                config: &config,
                registry: LanguageRegistry::builtin(),
                historical_architecture: true,
            },
            &cancel,
        )
        .unwrap();
        assert!(timeline.notes.is_empty(), "{:?}", timeline.notes);
        let snapshots = &timeline.evolution.report.snapshots;
        assert_eq!(snapshots.len(), 2);
        let first = snapshots[0].architecture.as_ref().unwrap();
        let last = snapshots[1].architecture.as_ref().unwrap();
        let modules: Vec<&str> = last.modules.iter().map(|m| m.path.as_str()).collect();
        assert_eq!(modules, vec!["app", "lib"]);
        assert!(first.edges.is_empty());
        assert_eq!(last.edges.len(), 1);
        assert_eq!(
            (last.edges[0].from.as_str(), last.edges[0].to.as_str()),
            ("app", "lib")
        );
        assert!(last.modules.iter().all(|m| m.bytes > 0));
    }
}
