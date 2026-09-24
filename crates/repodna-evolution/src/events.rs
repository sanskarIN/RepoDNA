//! Evidence-backed historical events, replayed from commit history and snapshots.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use repodna_core::confidence::Confidence;
use repodna_core::evidence::Evidence;
use repodna_core::hash::stable_id;
use repodna_core::model::evolution::{EvolutionEvent, EvolutionEventKind, Snapshot};
use repodna_core::model::git::ReleaseInfo;
use repodna_core::paths;
use repodna_core::time::Timestamp;
use repodna_discovery::classify::is_test_path;
use repodna_git::{ChangeStatus, History, ParsedCommit, area_of};
use repodna_project::is_ci_file;

/// Areas must reach this many files to count as introduced or removed modules.
const MIN_AREA_FILES: usize = 3;

/// A commit renaming at least this many files is a restructuring.
const RESTRUCTURE_RENAMES: usize = 20;

/// Maximum events of each kind (releases and area changes can be numerous).
const MAX_PER_KIND: usize = 50;

/// Manifest file names that declare a package.
const MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "go.mod",
    "pyproject.toml",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "composer.json",
    "pubspec.yaml",
    "Package.swift",
];

fn short(hash: &str) -> &str {
    &hash[..hash.len().min(7)]
}

fn percent(share: f64) -> String {
    format!("{:.0}", share * 100.0)
}

fn is_container_file(path: &str) -> bool {
    let name = paths::file_name(path).to_ascii_lowercase();
    name.starts_with("dockerfile")
        || name == "containerfile"
        || ((name.starts_with("docker-compose") || name.starts_with("compose."))
            && (name.ends_with(".yml") || name.ends_with(".yaml")))
}

fn kind_rank(kind: EvolutionEventKind) -> u8 {
    use EvolutionEventKind::*;
    match kind {
        RepositoryCreated => 0,
        MultiPackageStructure => 1,
        ModuleIntroduced => 2,
        ModuleRemoved => 3,
        Restructuring => 4,
        TestsIntroduced => 5,
        CiAdopted => 6,
        ContainersAdopted => 7,
        FrameworkAdopted => 8,
        FrameworkRemoved => 9,
        LanguageIntroduced => 10,
        LanguageShift => 11,
        SignificantGrowth => 12,
        SignificantReduction => 13,
        Release => 14,
    }
}

/// A kind of file whose first appearance is an event: the flag recording whether it was
/// seen, the event kind, the title, and the path test.
type FirstAppearance<'a> = (
    &'a mut bool,
    EvolutionEventKind,
    &'static str,
    fn(&str) -> bool,
);

struct Builder {
    events: Vec<EvolutionEvent>,
}

impl Builder {
    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        kind: EvolutionEventKind,
        date: Timestamp,
        title: String,
        description: String,
        commit: Option<&str>,
        confidence: Confidence,
        evidence: Vec<Evidence>,
    ) {
        let id = stable_id(&[
            format!("{kind:?}"),
            commit.unwrap_or_default().to_owned(),
            title.clone(),
        ]);
        self.events.push(EvolutionEvent {
            id,
            kind,
            date,
            title,
            description,
            commit: commit.map(str::to_owned),
            confidence,
            evidence,
        });
    }
}

fn commit_evidence(commit: &ParsedCommit) -> Evidence {
    Evidence::commit(
        commit.hash.clone(),
        Some(Timestamp::from_unix(commit.timestamp)),
    )
}

/// Replays history and compares snapshots to find notable events, oldest first.
pub fn detect_events(
    history: &History,
    snapshots: &[Snapshot],
    releases: &[ReleaseInfo],
) -> Vec<EvolutionEvent> {
    let mut out = Builder { events: Vec::new() };
    let oldest_first: Vec<&ParsedCommit> = history.commits.iter().rev().collect();
    let mut files: HashSet<String> = HashSet::new();
    let mut area_counts: HashMap<String, usize> = HashMap::new();
    let mut area_peak: HashMap<String, usize> = HashMap::new();
    let mut present_areas: HashSet<String> = HashSet::new();
    let mut pending_intro: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let mut removed: Vec<(usize, String)> = Vec::new();
    let mut manifest_dirs: HashMap<&str, HashSet<String>> = HashMap::new();
    let (mut tests_seen, mut ci_seen, mut containers_seen, mut multi_seen) =
        (false, false, false, false);

    for (index, commit) in oldest_first.iter().enumerate() {
        let date = Timestamp::from_unix(commit.timestamp);
        let mut added_paths: Vec<&str> = Vec::new();
        let mut renames = 0usize;
        let mut touched_areas: BTreeSet<String> = BTreeSet::new();
        for change in &commit.changes {
            let mut remove = |path: &str, files: &mut HashSet<String>| {
                if files.remove(path) {
                    let area = area_of(path);
                    if let Some(count) = area_counts.get_mut(&area) {
                        *count = count.saturating_sub(1);
                    }
                    touched_areas.insert(area);
                }
            };
            match change.status {
                ChangeStatus::Deleted => remove(&change.path, &mut files),
                ChangeStatus::Renamed => {
                    renames += 1;
                    if let Some(old) = &change.old_path {
                        remove(old, &mut files);
                    }
                }
                _ => {}
            }
            if change.status != ChangeStatus::Deleted && files.insert(change.path.clone()) {
                let area = area_of(&change.path);
                let count = area_counts.entry(area.clone()).or_default();
                *count += 1;
                let peak = area_peak.entry(area.clone()).or_default();
                *peak = (*peak).max(*count);
                touched_areas.insert(area);
                if matches!(
                    change.status,
                    ChangeStatus::Added | ChangeStatus::Copied | ChangeStatus::Renamed
                ) {
                    added_paths.push(&change.path);
                }
            }
        }

        if index == 0 {
            let created = added_paths.len();
            let (title, description, confidence) = if history.truncated {
                (
                    "Oldest analyzed commit".to_owned(),
                    format!(
                        "History was read up to a limit; the oldest analyzed commit touched {created} files."
                    ),
                    Confidence::Medium,
                )
            } else {
                (
                    "Repository created".to_owned(),
                    format!("The first commit added {created} files."),
                    Confidence::High,
                )
            };
            out.push(
                EvolutionEventKind::RepositoryCreated,
                date,
                title,
                description,
                Some(&commit.hash),
                confidence,
                vec![commit_evidence(commit)],
            );
            tests_seen = files.iter().any(|path| is_test_path(path));
            ci_seen = files.iter().any(|path| is_ci_file(path));
            containers_seen = files.iter().any(|path| is_container_file(path));
            for path in &files {
                if MANIFESTS.contains(&paths::file_name(path)) {
                    manifest_dirs
                        .entry(
                            MANIFESTS
                                .iter()
                                .find(|m| **m == paths::file_name(path))
                                .copied()
                                .unwrap_or_default(),
                        )
                        .or_default()
                        .insert(paths::parent(path).to_owned());
                }
            }
            multi_seen = manifest_dirs.values().any(|dirs| dirs.len() >= 2);
            present_areas.extend(area_counts.keys().cloned());
            continue;
        }

        // Areas that appeared or disappeared in this commit.
        for area in touched_areas {
            if area == "(root)" {
                continue;
            }
            let count = area_counts.get(&area).copied().unwrap_or(0);
            let present = present_areas.contains(&area);
            if !present && count > 0 {
                present_areas.insert(area.clone());
                pending_intro.entry(index).or_default().push(area);
            } else if present && count == 0 {
                present_areas.remove(&area);
                if area_peak.get(&area).copied().unwrap_or(0) >= MIN_AREA_FILES {
                    removed.push((index, area));
                }
            }
        }

        if renames >= RESTRUCTURE_RENAMES {
            out.push(
                EvolutionEventKind::Restructuring,
                date,
                format!("{renames} files were moved or renamed"),
                format!("One commit renamed or moved {renames} files."),
                Some(&commit.hash),
                Confidence::High,
                vec![commit_evidence(commit)],
            );
        }
        let firsts: [FirstAppearance<'_>; 3] = [
            (
                &mut tests_seen,
                EvolutionEventKind::TestsIntroduced,
                "Tests were added",
                |p| is_test_path(p),
            ),
            (
                &mut ci_seen,
                EvolutionEventKind::CiAdopted,
                "Continuous integration was set up",
                |p| is_ci_file(p),
            ),
            (
                &mut containers_seen,
                EvolutionEventKind::ContainersAdopted,
                "Container definitions were added",
                is_container_file,
            ),
        ];
        for (seen, kind, title, matches) in firsts {
            if !*seen && let Some(path) = added_paths.iter().find(|path| matches(path)) {
                *seen = true;
                out.push(
                    kind,
                    date,
                    title.to_owned(),
                    format!("{path} was added."),
                    Some(&commit.hash),
                    Confidence::High,
                    vec![commit_evidence(commit), Evidence::file(*path)],
                );
            }
        }
        for path in &added_paths {
            let name = paths::file_name(path);
            if let Some(manifest) = MANIFESTS.iter().find(|m| **m == name) {
                let dirs = manifest_dirs.entry(manifest).or_default();
                dirs.insert(paths::parent(path).to_owned());
                if !multi_seen && dirs.len() >= 2 {
                    multi_seen = true;
                    out.push(
                        EvolutionEventKind::MultiPackageStructure,
                        date,
                        "The repository started holding several packages".to_owned(),
                        format!(
                            "A second {manifest} appeared, in {}.",
                            if paths::parent(path).is_empty() {
                                "the root"
                            } else {
                                paths::parent(path)
                            }
                        ),
                        Some(&commit.hash),
                        Confidence::High,
                        vec![commit_evidence(commit), Evidence::file(*path)],
                    );
                }
            }
        }
    }

    // Introduced areas count only when they grew to a meaningful size.
    let mut introductions = 0;
    for (index, areas) in pending_intro {
        for area in areas {
            if area_peak.get(&area).copied().unwrap_or(0) < MIN_AREA_FILES
                || introductions >= MAX_PER_KIND
            {
                continue;
            }
            introductions += 1;
            let commit = oldest_first[index];
            let peak = area_peak.get(&area).copied().unwrap_or(0);
            out.push(
                EvolutionEventKind::ModuleIntroduced,
                Timestamp::from_unix(commit.timestamp),
                format!("{area}/ appeared"),
                format!("The first files in {area}/ were added; it later held up to {peak} files."),
                Some(&commit.hash),
                Confidence::High,
                vec![commit_evidence(commit), Evidence::directory(area.as_str())],
            );
        }
    }
    for (index, area) in removed.into_iter().take(MAX_PER_KIND) {
        let commit = oldest_first[index];
        out.push(
            EvolutionEventKind::ModuleRemoved,
            Timestamp::from_unix(commit.timestamp),
            format!("{area}/ was removed"),
            format!("The last files in {area}/ were deleted or moved."),
            Some(&commit.hash),
            Confidence::High,
            vec![commit_evidence(commit), Evidence::directory(area.as_str())],
        );
    }

    let mut sorted_releases: Vec<&ReleaseInfo> = releases.iter().collect();
    sorted_releases.sort_by_key(|release| release.date);
    let skip = sorted_releases.len().saturating_sub(MAX_PER_KIND);
    for release in sorted_releases.into_iter().skip(skip) {
        out.push(
            EvolutionEventKind::Release,
            release.date,
            format!("Release {}", release.tag),
            format!(
                "Tag {} points to commit {}.",
                release.tag,
                short(&release.commit)
            ),
            Some(&release.commit),
            Confidence::High,
            vec![Evidence::commit(release.commit.clone(), Some(release.date))],
        );
    }

    snapshot_events(&mut out, snapshots);
    let mut events = out.events;
    events.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then_with(|| kind_rank(a.kind).cmp(&kind_rank(b.kind)))
    });
    events
}

/// Language and size changes between consecutive snapshots.
fn snapshot_events(out: &mut Builder, snapshots: &[Snapshot]) {
    for pair in snapshots.windows(2) {
        let (before, after) = (&pair[0], &pair[1]);
        let evidence = vec![
            Evidence::commit(before.revision.clone(), Some(before.date)),
            Evidence::commit(after.revision.clone(), Some(after.date)),
        ];
        let shares: HashMap<&str, f64> = before
            .languages
            .iter()
            .map(|l| (l.id.as_str(), l.share))
            .collect();
        for language in &after.languages {
            let previous = shares.get(language.id.as_str()).copied().unwrap_or(0.0);
            if previous < 0.05 && language.share >= 0.10 {
                out.push(
                    EvolutionEventKind::LanguageIntroduced,
                    after.date,
                    format!("{} became a notable language", language.id),
                    format!(
                        "Its share of first-party code (by bytes) grew from {}% at {} to {}% at {}.",
                        percent(previous), before.label, percent(language.share), after.label
                    ),
                    Some(&after.revision),
                    Confidence::Medium,
                    evidence.clone(),
                );
            } else if previous >= 0.05 && (language.share - previous).abs() >= 0.20 {
                out.push(
                    EvolutionEventKind::LanguageShift,
                    after.date,
                    format!("The share of {} changed", language.id),
                    format!(
                        "Its share of first-party code (by bytes) went from {}% at {} to {}% at {}.",
                        percent(previous), before.label, percent(language.share), after.label
                    ),
                    Some(&after.revision),
                    Confidence::Medium,
                    evidence.clone(),
                );
            }
        }
        let (a, b) = (before.files, after.files);
        if b >= a + 50 && b * 2 >= a * 3 {
            out.push(
                EvolutionEventKind::SignificantGrowth,
                after.date,
                format!("The repository grew to {b} files"),
                format!(
                    "It went from {a} files at {} to {b} files at {}.",
                    before.label, after.label
                ),
                Some(&after.revision),
                Confidence::High,
                evidence.clone(),
            );
        } else if a >= b + 30 && b * 10 <= a * 7 {
            out.push(
                EvolutionEventKind::SignificantReduction,
                after.date,
                format!("The repository shrank to {b} files"),
                format!(
                    "It went from {a} files at {} to {b} files at {}.",
                    before.label, after.label
                ),
                Some(&after.revision),
                Confidence::High,
                evidence,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{commit, history};
    use repodna_core::model::evolution::{SnapshotKind, SnapshotLanguage};

    fn kinds(events: &[EvolutionEvent]) -> Vec<EvolutionEventKind> {
        events.iter().map(|e| e.kind).collect()
    }

    #[test]
    fn replays_history_into_events() {
        let renames: Vec<String> = (0..20)
            .map(|i| format!("lib/f{i}.py>src/f{i}.py"))
            .collect();
        let rename_refs: Vec<&str> = renames.iter().map(String::as_str).collect();
        let adds: Vec<String> = (0..20).map(|i| format!("+lib/f{i}.py")).collect();
        let mut first: Vec<&str> = adds.iter().map(String::as_str).collect();
        first.push("+Cargo.toml");
        let commits = vec![
            commit("c1", "2020-01-01", "ana", &first),
            commit(
                "c2",
                "2020-02-01",
                "ana",
                &[
                    "+tests/test_a.py",
                    "+tools/a.sh",
                    "+tools/b.sh",
                    "+tools/c.sh",
                ],
            ),
            commit(
                "c3",
                "2020-03-01",
                "ana",
                &[
                    "+.github/workflows/ci.yml",
                    "+Dockerfile",
                    "+plugin/Cargo.toml",
                ],
            ),
            commit("c4", "2020-04-01", "ana", &rename_refs),
            commit(
                "c5",
                "2020-05-01",
                "ana",
                &["-tools/a.sh", "-tools/b.sh", "-tools/c.sh", "+x/one.txt"],
            ),
        ];
        let releases = [ReleaseInfo {
            tag: "v1.0.0".into(),
            version: "1.0.0".into(),
            date: Timestamp::from_ymd(2020, 4, 15).unwrap(),
            commit: "c4".into(),
            commits_since_previous: 4,
        }];
        let events = detect_events(&history(commits), &[], &releases);
        assert_eq!(
            kinds(&events),
            vec![
                EvolutionEventKind::RepositoryCreated,
                EvolutionEventKind::ModuleIntroduced,
                EvolutionEventKind::TestsIntroduced,
                EvolutionEventKind::MultiPackageStructure,
                EvolutionEventKind::CiAdopted,
                EvolutionEventKind::ContainersAdopted,
                EvolutionEventKind::ModuleIntroduced,
                EvolutionEventKind::ModuleRemoved,
                EvolutionEventKind::Restructuring,
                EvolutionEventKind::Release,
                EvolutionEventKind::ModuleRemoved,
            ]
        );
        assert_eq!(events[0].description, "The first commit added 21 files.");
        assert_eq!(events[1].title, "tools/ appeared");
        assert_eq!(events[6].title, "src/ appeared");
        let removed: Vec<&str> = events
            .iter()
            .filter(|e| e.kind == EvolutionEventKind::ModuleRemoved)
            .map(|e| e.title.as_str())
            .collect();
        assert_eq!(removed, vec!["lib/ was removed", "tools/ was removed"]);
    }

    fn snapshot(label: &str, index: u64, files: u64, rust: f64, python: f64) -> Snapshot {
        Snapshot {
            revision: label.to_owned(),
            label: label.to_owned(),
            kind: SnapshotKind::Sample,
            date: Timestamp::from_unix(index as i64 * 1_000),
            commit_index: index,
            files,
            bytes: files * 100,
            test_files: 0,
            doc_files: 0,
            languages: vec![
                SnapshotLanguage {
                    id: "rust".into(),
                    files: 1,
                    bytes: 1,
                    share: rust,
                },
                SnapshotLanguage {
                    id: "python".into(),
                    files: 1,
                    bytes: 1,
                    share: python,
                },
            ],
            directories: Vec::new(),
            dependencies: None,
            architecture: None,
        }
    }

    #[test]
    fn compares_snapshots() {
        let snapshots = [
            snapshot("a", 1, 100, 1.0, 0.0),
            snapshot("b", 2, 200, 0.6, 0.4),
            snapshot("c", 3, 120, 0.95, 0.05),
        ];
        let events = detect_events(&history(Vec::new()), &snapshots, &[]);
        assert_eq!(
            kinds(&events),
            vec![
                EvolutionEventKind::LanguageIntroduced,
                EvolutionEventKind::LanguageShift,
                EvolutionEventKind::SignificantGrowth,
                EvolutionEventKind::LanguageShift,
                EvolutionEventKind::LanguageShift,
                EvolutionEventKind::SignificantReduction,
            ]
        );
    }
}
