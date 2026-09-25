//! Time Machine snapshots: the repository as it existed at selected revisions.

use std::collections::{BTreeMap, HashMap};

use repodna_core::metric::round4;
use repodna_core::model::evolution::{
    GrowthPoint, Snapshot, SnapshotDirectory, SnapshotKind, SnapshotLanguage,
};
use repodna_core::model::git::ReleaseInfo;
use repodna_core::model::structure::FileCategory;
use repodna_core::time::Timestamp;
use repodna_discovery::{ClassificationOverrides, classify};
use repodna_git::{History, ParsedCommit, TreeEntry};
use repodna_parser::LanguageRegistry;

/// Maximum languages recorded per snapshot.
const MAX_LANGUAGES: usize = 20;

/// Maximum top-level directories recorded per snapshot.
const MAX_DIRECTORIES: usize = 30;

/// How snapshots are chosen, for reports.
pub const SAMPLING_METHOD: &str = "Snapshots include the first analyzed commit, the current \
revision, evenly spaced release tags, and commits sampled at evenly spaced dates in between. \
Each snapshot is measured from a listing of the files at that revision, so sizes are in bytes \
rather than lines.";

/// A revision chosen for a snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotPlan {
    /// Commit hash.
    pub revision: String,
    /// Display label.
    pub label: String,
    /// Why the revision was chosen.
    pub kind: SnapshotKind,
    /// Commit timestamp.
    pub date: Timestamp,
    /// Commits up to and including this revision.
    pub commit_index: u64,
}

fn priority(kind: SnapshotKind) -> u8 {
    match kind {
        SnapshotKind::Initial | SnapshotKind::Current => 0,
        SnapshotKind::Release => 1,
        SnapshotKind::Sample => 2,
    }
}

/// Chooses up to `max` revisions (at least the first and the current one).
pub fn plan_snapshots(
    history: &History,
    releases: &[ReleaseInfo],
    max: usize,
) -> Vec<SnapshotPlan> {
    let oldest_first: Vec<&ParsedCommit> = history.commits.iter().rev().collect();
    let (Some(first), Some(last)) = (oldest_first.first(), oldest_first.last()) else {
        return Vec::new();
    };
    let max = max.max(2);
    let index_of: HashMap<&str, usize> = oldest_first
        .iter()
        .enumerate()
        .map(|(index, commit)| (commit.hash.as_str(), index))
        .collect();
    let plan = |index: usize, label: String, kind: SnapshotKind| SnapshotPlan {
        revision: oldest_first[index].hash.clone(),
        label,
        kind,
        date: Timestamp::from_unix(oldest_first[index].timestamp),
        commit_index: index as u64 + 1,
    };

    let initial_label = if history.truncated {
        "Oldest analyzed commit"
    } else {
        "Initial commit"
    };
    let mut plans = vec![
        plan(0, initial_label.to_owned(), SnapshotKind::Initial),
        plan(
            oldest_first.len() - 1,
            "Current".to_owned(),
            SnapshotKind::Current,
        ),
    ];

    // Evenly spaced releases, at most half of the remaining slots.
    let mut known_releases: Vec<(&ReleaseInfo, usize)> = releases
        .iter()
        .filter_map(|release| Some((release, *index_of.get(release.commit.as_str())?)))
        .collect();
    known_releases.sort_by_key(|(_, index)| *index);
    let release_slots = (max - 2) / 2;
    for position in spaced(known_releases.len(), release_slots) {
        let (release, index) = known_releases[position];
        plans.push(plan(index, release.tag.clone(), SnapshotKind::Release));
    }

    // Commits at evenly spaced dates fill the remaining slots.
    let sample_slots = max.saturating_sub(2 + release_slots.min(known_releases.len()));
    let (start, end) = (first.timestamp, last.timestamp);
    for step in 1..=sample_slots {
        let target = start + (end - start) * step as i64 / (sample_slots as i64 + 1);
        let index = oldest_first.partition_point(|commit| commit.timestamp <= target);
        if index == 0 {
            continue;
        }
        let label = sample_label(
            Timestamp::from_unix(oldest_first[index - 1].timestamp),
            end - start,
        );
        plans.push(plan(index - 1, label, SnapshotKind::Sample));
    }

    // One snapshot per revision, keeping the most meaningful reason.
    plans.sort_by_key(|p| (p.commit_index, priority(p.kind)));
    plans.dedup_by(|later, earlier| later.revision == earlier.revision);
    plans.truncate(max);
    plans
}

/// A label for a sampled snapshot, just precise enough to tell the samples of a history
/// `span` seconds long apart: the month for long histories, the day, or the minute (UTC).
fn sample_label(at: Timestamp, span: i64) -> String {
    const DAY: i64 = 86_400;
    if span >= 180 * DAY {
        at.month_key()
    } else if span >= 7 * DAY {
        at.date_string()
    } else {
        let civil = at.civil();
        format!(
            "{} {:02}:{:02} UTC",
            at.date_string(),
            civil.hour,
            civil.minute
        )
    }
}

/// `count` indices spread evenly over `0..len`, always including the last one.
fn spaced(len: usize, count: usize) -> Vec<usize> {
    if len == 0 || count == 0 {
        return Vec::new();
    }
    if count >= len {
        return (0..len).collect();
    }
    let mut indices: Vec<usize> = (1..=count).map(|i| i * len / count - 1).collect();
    indices.dedup();
    indices
}

/// Measures a snapshot from the tree listing of its revision.
pub fn build_snapshot(
    plan: &SnapshotPlan,
    entries: &[TreeEntry],
    registry: &LanguageRegistry,
) -> Snapshot {
    let overrides = ClassificationOverrides::default();
    let mut files = 0u64;
    let mut bytes = 0u64;
    let mut test_files = 0u64;
    let mut doc_files = 0u64;
    let mut languages: BTreeMap<String, (u64, u64, bool)> = BTreeMap::new();
    let mut directories: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for entry in entries.iter().filter(|entry| entry.mode != "120000") {
        files += 1;
        bytes += entry.size;
        let spec = registry.detect_path(&entry.path);
        let classification = classify(&entry.path, spec.map(|s| s.kind), &overrides);
        match classification.category {
            FileCategory::Test => test_files += 1,
            FileCategory::Documentation => doc_files += 1,
            _ => {}
        }
        let first_party = matches!(
            classification.category,
            FileCategory::Source | FileCategory::Test
        ) && !classification.vendored
            && !classification.generated;
        if let Some(spec) = spec.filter(|_| first_party) {
            let entry_stats =
                languages
                    .entry(spec.id.clone())
                    .or_insert((0, 0, spec.kind.counts_toward_share()));
            entry_stats.0 += 1;
            entry_stats.1 += entry.size;
        }
        let top = match entry.path.split_once('/') {
            Some((dir, _)) => dir.to_owned(),
            None => String::new(),
        };
        let dir = directories.entry(top).or_default();
        dir.0 += 1;
        dir.1 += entry.size;
    }
    let share_total: u64 = languages
        .values()
        .filter(|(_, _, counts)| *counts)
        .map(|(_, bytes, _)| bytes)
        .sum();
    let mut languages: Vec<SnapshotLanguage> = languages
        .into_iter()
        .map(|(id, (files, bytes, counts))| SnapshotLanguage {
            id,
            files,
            bytes,
            share: if counts && share_total > 0 {
                round4(bytes as f64 / share_total as f64)
            } else {
                0.0
            },
        })
        .collect();
    languages.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.id.cmp(&b.id)));
    languages.truncate(MAX_LANGUAGES);
    let mut directories: Vec<SnapshotDirectory> = directories
        .into_iter()
        .map(|(path, (files, bytes))| SnapshotDirectory { path, files, bytes })
        .collect();
    directories.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.path.cmp(&b.path)));
    directories.truncate(MAX_DIRECTORIES);
    Snapshot {
        revision: plan.revision.clone(),
        label: plan.label.clone(),
        kind: plan.kind,
        date: plan.date,
        commit_index: plan.commit_index,
        files,
        bytes,
        test_files,
        doc_files,
        languages,
        directories,
        dependencies: None,
        architecture: None,
    }
}

/// The growth curve: size and commit count at each snapshot.
pub fn growth(snapshots: &[Snapshot]) -> Vec<GrowthPoint> {
    snapshots
        .iter()
        .map(|snapshot| GrowthPoint {
            date: snapshot.date,
            files: snapshot.files,
            bytes: snapshot.bytes,
            commits: snapshot.commit_index,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{commit, history};

    fn release(tag: &str, commit: &str) -> ReleaseInfo {
        ReleaseInfo {
            tag: tag.to_owned(),
            version: tag.trim_start_matches('v').to_owned(),
            date: Timestamp::from_unix(0),
            commit: commit.to_owned(),
            commits_since_previous: 0,
        }
    }

    #[test]
    fn plans_initial_current_release_and_sample_snapshots() {
        let commits: Vec<_> = (0..24)
            .map(|month| {
                let date = format!("{}-{:02}-15", 2020 + month / 12, month % 12 + 1);
                commit(&format!("c{month:02}"), &date, "ana", &["~src/lib.rs"])
            })
            .collect();
        let history = history(commits);
        let releases = [
            release("v1.0.0", "c05"),
            release("v2.0.0", "c17"),
            release("v9", "unknown"),
        ];
        let plans = plan_snapshots(&history, &releases, 6);
        let summary: Vec<(&str, SnapshotKind, &str)> = plans
            .iter()
            .map(|p| (p.revision.as_str(), p.kind, p.label.as_str()))
            .collect();
        assert_eq!(summary.len(), 6);
        assert_eq!(summary[0], ("c00", SnapshotKind::Initial, "Initial commit"));
        assert_eq!(summary[5], ("c23", SnapshotKind::Current, "Current"));
        assert!(summary.contains(&("c05", SnapshotKind::Release, "v1.0.0")));
        assert!(summary.contains(&("c17", SnapshotKind::Release, "v2.0.0")));
        assert!(
            plans
                .windows(2)
                .all(|pair| pair[0].commit_index < pair[1].commit_index)
        );
        assert!(
            plans
                .iter()
                .any(|p| p.kind == SnapshotKind::Sample && p.label.starts_with("202"))
        );
        assert!(
            plan_snapshots(
                &History {
                    commits: Vec::new(),
                    truncated: false
                },
                &[],
                6
            )
            .is_empty()
        );
    }

    #[test]
    fn measures_snapshots_from_tree_listings() {
        let plan = SnapshotPlan {
            revision: "abc".into(),
            label: "Current".into(),
            kind: SnapshotKind::Current,
            date: Timestamp::from_unix(0),
            commit_index: 10,
        };
        let entry = |path: &str, size: u64| TreeEntry {
            path: path.to_owned(),
            size,
            mode: "100644".to_owned(),
        };
        let entries = [
            entry("src/lib.rs", 3_000),
            entry("src/util.py", 1_000),
            entry("tests/cli.rs", 500),
            entry("README.md", 400),
            entry("vendor/lib/x.rs", 9_000),
            entry("data.json", 100),
            TreeEntry {
                path: "link".into(),
                size: 5,
                mode: "120000".into(),
            },
        ];
        let snapshot = build_snapshot(&plan, &entries, LanguageRegistry::builtin());
        assert_eq!(snapshot.files, 6);
        assert_eq!(snapshot.bytes, 14_000);
        assert_eq!(snapshot.test_files, 1);
        assert_eq!(snapshot.doc_files, 1);
        let languages: Vec<(&str, u64, f64)> = snapshot
            .languages
            .iter()
            .map(|l| (l.id.as_str(), l.bytes, l.share))
            .collect();
        assert_eq!(
            languages,
            vec![("rust", 3_500, 0.7778), ("python", 1_000, 0.2222)]
        );
        assert_eq!(snapshot.directories[0].path, "vendor");
        let points = growth(&[snapshot]);
        assert_eq!(points[0].commits, 10);
        assert_eq!(spaced(10, 3), vec![2, 5, 9]);
        assert_eq!(spaced(2, 5), vec![0, 1]);
    }

    #[test]
    fn sample_labels_match_the_length_of_history() {
        let at = Timestamp::parse_rfc3339("2026-09-24T11:10:21Z").unwrap();
        assert_eq!(sample_label(at, 400 * 86_400), "2026-09");
        assert_eq!(sample_label(at, 30 * 86_400), "2026-09-24");
        assert_eq!(sample_label(at, 3_600), "2026-09-24 11:10 UTC");
    }
}
