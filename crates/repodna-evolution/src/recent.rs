//! What changed recently: the window of history before the analyzed revision.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use repodna_core::model::insights::{ChangeKind, DependencyChange, DirectoryChange, RecentChanges};
use repodna_core::model::structure::FileCategory;
use repodna_core::time::{SECONDS_PER_DAY, Timestamp};
use repodna_dependencies::is_dependency_file;
use repodna_discovery::{ClassificationOverrides, classify};
use repodna_git::{ChangeStatus, History, area_of, contributor_id};

/// Directories listed in recent changes.
const MAX_DIRECTORIES: usize = 10;

/// Files listed as added or removed.
const MAX_FILES: usize = 100;

/// Dependency changes listed.
const MAX_DEPENDENCY_CHANGES: usize = 200;

/// Summarizes the commits in the `window_days` before the latest commit. `current_files`
/// are the files present at the analyzed revision.
pub fn recent_changes(
    history: &History,
    window_days: u32,
    current_files: &HashSet<&str>,
) -> Option<RecentChanges> {
    let latest = history.commits.first()?;
    let until = latest.timestamp;
    let since = until - i64::from(window_days) * SECONDS_PER_DAY;
    let overrides = ClassificationOverrides::default();
    let mut report = RecentChanges {
        window_days,
        since: Some(Timestamp::from_unix(since)),
        until: Some(Timestamp::from_unix(until)),
        ..RecentChanges::default()
    };
    let mut authors = HashSet::new();
    let mut changed: BTreeSet<&str> = BTreeSet::new();
    let mut added: BTreeSet<&str> = BTreeSet::new();
    let mut removed: BTreeSet<&str> = BTreeSet::new();
    let mut directories: HashMap<String, (HashSet<&str>, u64)> = HashMap::new();
    // Newest first, so the latest status of a path wins.
    for commit in history
        .commits
        .iter()
        .take_while(|commit| commit.timestamp > since)
    {
        report.commits += 1;
        authors.insert(contributor_id(&commit.author_email, &commit.author_name));
        for change in &commit.changes {
            report.insertions += u64::from(change.insertions);
            report.deletions += u64::from(change.deletions);
            changed.insert(&change.path);
            let directory = directories.entry(area_of(&change.path)).or_default();
            directory.0.insert(&commit.hash);
            directory.1 += u64::from(change.insertions) + u64::from(change.deletions);
            match change.status {
                ChangeStatus::Added | ChangeStatus::Copied
                    if current_files.contains(change.path.as_str()) =>
                {
                    added.insert(&change.path);
                }
                ChangeStatus::Renamed if current_files.contains(change.path.as_str()) => {
                    added.insert(&change.path);
                    if let Some(old) = &change.old_path
                        && !current_files.contains(old.as_str())
                    {
                        removed.insert(old);
                    }
                }
                ChangeStatus::Deleted if !current_files.contains(change.path.as_str()) => {
                    removed.insert(&change.path);
                }
                _ => {}
            }
        }
    }
    report.contributors = u32::try_from(authors.len()).unwrap_or(u32::MAX);
    report.files_changed = changed.len() as u64;
    for path in &changed {
        match classify(path, None, &overrides).category {
            FileCategory::Test => report.test_files_changed += 1,
            FileCategory::Documentation => report.doc_files_changed += 1,
            _ => {}
        }
    }
    report.manifests_changed = changed
        .iter()
        .filter(|path| is_dependency_file(path))
        .map(|path| (*path).to_owned())
        .collect();
    let mut directories: Vec<DirectoryChange> = directories
        .into_iter()
        .map(|(path, (commits, churn))| DirectoryChange {
            path,
            commits: commits.len() as u64,
            churn,
        })
        .collect();
    directories.sort_by(|a, b| b.churn.cmp(&a.churn).then_with(|| a.path.cmp(&b.path)));
    directories.truncate(MAX_DIRECTORIES);
    report.directories = directories;
    report.added_files = added
        .into_iter()
        .take(MAX_FILES)
        .map(str::to_owned)
        .collect();
    report.removed_files = removed
        .into_iter()
        .take(MAX_FILES)
        .map(str::to_owned)
        .collect();
    Some(report)
}

/// A dependency declaration: ecosystem, manifest, name, and requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    /// Ecosystem identifier.
    pub ecosystem: String,
    /// Manifest path.
    pub manifest: String,
    /// Package name.
    pub name: String,
    /// Declared requirement.
    pub requirement: Option<String>,
}

/// Compares dependency declarations of two revisions.
pub fn diff_dependencies(before: &[Declaration], after: &[Declaration]) -> Vec<DependencyChange> {
    let key = |d: &Declaration| {
        (
            d.ecosystem.clone(),
            d.manifest.clone(),
            d.name.to_ascii_lowercase(),
        )
    };
    let before: BTreeMap<_, &Declaration> = before.iter().map(|d| (key(d), d)).collect();
    let after: BTreeMap<_, &Declaration> = after.iter().map(|d| (key(d), d)).collect();
    let mut changes = Vec::new();
    for (k, new) in &after {
        let change = match before.get(k) {
            None => Some((ChangeKind::Added, None)),
            Some(old) if old.requirement != new.requirement => {
                Some((ChangeKind::Changed, old.requirement.clone()))
            }
            Some(_) => None,
        };
        if let Some((kind, from)) = change {
            changes.push(DependencyChange {
                ecosystem: new.ecosystem.clone(),
                name: new.name.clone(),
                change: kind,
                from,
                to: new.requirement.clone(),
                manifest: new.manifest.clone(),
            });
        }
    }
    for (k, old) in &before {
        if !after.contains_key(k) {
            changes.push(DependencyChange {
                ecosystem: old.ecosystem.clone(),
                name: old.name.clone(),
                change: ChangeKind::Removed,
                from: old.requirement.clone(),
                to: None,
                manifest: old.manifest.clone(),
            });
        }
    }
    changes.sort_by(|a, b| {
        a.manifest
            .cmp(&b.manifest)
            .then_with(|| a.name.cmp(&b.name))
    });
    changes.truncate(MAX_DEPENDENCY_CHANGES);
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{commit, history};

    #[test]
    fn summarizes_the_recent_window() {
        let history = history(vec![
            commit("c1", "2024-01-01", "ana", &["+src/old.rs", "+src/lib.rs"]),
            commit(
                "c2",
                "2024-05-01",
                "ana",
                &["~src/lib.rs", "+docs/guide.md"],
            ),
            commit(
                "c3",
                "2024-05-20",
                "bo",
                &["-src/old.rs", "+tests/cli.rs", "~Cargo.toml"],
            ),
            commit(
                "c4",
                "2024-06-01",
                "bo",
                &["src/a.rs>src/b.rs", "+tmp/x.txt", "-tmp/x.txt"],
            ),
        ]);
        let current: HashSet<&str> = [
            "src/lib.rs",
            "docs/guide.md",
            "tests/cli.rs",
            "Cargo.toml",
            "src/b.rs",
        ]
        .into();
        let recent = recent_changes(&history, 90, &current).unwrap();
        assert_eq!(recent.commits, 3);
        assert_eq!(recent.contributors, 2);
        assert_eq!(
            recent.added_files,
            vec!["docs/guide.md", "src/b.rs", "tests/cli.rs"]
        );
        assert_eq!(
            recent.removed_files,
            vec!["src/a.rs", "src/old.rs", "tmp/x.txt"]
        );
        assert_eq!(recent.manifests_changed, vec!["Cargo.toml"]);
        assert_eq!(recent.test_files_changed, 1);
        assert_eq!(recent.doc_files_changed, 1);
        assert_eq!(recent.directories[0].path, "src");
        assert!(recent_changes(&crate::testutil::history(Vec::new()), 90, &current).is_none());
    }

    #[test]
    fn diffs_dependency_declarations() {
        let declaration = |name: &str, requirement: &str| Declaration {
            ecosystem: "npm".into(),
            manifest: "package.json".into(),
            name: name.into(),
            requirement: Some(requirement.into()),
        };
        let before = [
            declaration("react", "^18.0.0"),
            declaration("lodash", "^4.0.0"),
        ];
        let after = [
            declaration("react", "^19.0.0"),
            declaration("zod", "^3.0.0"),
        ];
        let changes = diff_dependencies(&before, &after);
        let summary: Vec<(&str, ChangeKind)> = changes
            .iter()
            .map(|c| (c.name.as_str(), c.change))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("lodash", ChangeKind::Removed),
                ("react", ChangeKind::Changed),
                ("zod", ChangeKind::Added),
            ]
        );
        assert_eq!(changes[1].from.as_deref(), Some("^18.0.0"));
    }
}
