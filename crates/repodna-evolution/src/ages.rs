//! Module ages and areas that stopped changing.

use std::collections::{BTreeMap, HashMap};

use repodna_core::model::evolution::{AbandonedArea, AgeClass, ModuleAge};
use repodna_core::model::git::FileHistoryRecord;
use repodna_core::time::Timestamp;
use repodna_git::area_of;

/// Days without change after which an area counts as abandoned.
pub const ABANDONED_DAYS: i64 = 365;

/// Minimum files for an abandoned area to be listed.
const MIN_ABANDONED_FILES: u64 = 3;

/// Maximum abandoned areas listed.
const MAX_ABANDONED: usize = 30;

/// Share of a module's commits in the recent window at which it counts as growing.
const GROWING_RECENT_SHARE: f64 = 0.3;

/// Classifies the age of each module from the history of its current files.
///
/// `modules` pairs module identifiers with their file paths. `history_start` is the first
/// analyzed commit and `latest` the latest commit.
pub fn module_ages(
    modules: &[(String, Vec<String>)],
    file_history: &[FileHistoryRecord],
    history_start: Timestamp,
    latest: Timestamp,
) -> Vec<ModuleAge> {
    let by_path: HashMap<&str, &FileHistoryRecord> = file_history
        .iter()
        .map(|record| (record.path.as_str(), record))
        .collect();
    let span = history_start.days_until(latest).max(1);
    let mut ages = Vec::new();
    for (module, paths) in modules {
        let records: Vec<&FileHistoryRecord> = paths
            .iter()
            .filter_map(|path| by_path.get(path.as_str()).copied())
            .collect();
        let (Some(first), Some(last)) = (
            records.iter().map(|r| r.first_seen).min(),
            records.iter().map(|r| r.last_changed).max(),
        ) else {
            continue;
        };
        let commits: u64 = records.iter().map(|r| u64::from(r.commits)).sum();
        let recent: u64 = records.iter().map(|r| u64::from(r.recent_commits)).sum();
        let recent_share = if commits == 0 {
            0.0
        } else {
            recent as f64 / commits as f64
        };
        let age_days = first.days_until(latest);
        let offset = history_start.days_until(first);
        let since = format!(
            "First changed on {}, {age_days} days before the latest commit",
            first.date_string()
        );
        let (class, reason) = if age_days <= 30 {
            (AgeClass::New, format!("{since}."))
        } else if age_days <= 365 {
            (AgeClass::Recent, format!("{since}."))
        } else if age_days > 90 && recent_share >= GROWING_RECENT_SHARE {
            (
                AgeClass::Growing,
                format!(
                    "{since}; {:.0}% of its commits are recent.",
                    recent_share * 100.0
                ),
            )
        } else if offset * 5 <= span && age_days > 730 {
            (
                AgeClass::Ancient,
                format!("{since}, in the first fifth of the history."),
            )
        } else {
            (
                AgeClass::Established,
                format!("{since}; most of its commits are older."),
            )
        };
        ages.push(ModuleAge {
            module: module.clone(),
            first_commit: first,
            last_change: last,
            age_days,
            class,
            reason,
        });
    }
    ages.sort_by(|a, b| {
        a.first_commit
            .cmp(&b.first_commit)
            .then_with(|| a.module.cmp(&b.module))
    });
    ages
}

/// Areas whose files have not changed for [`ABANDONED_DAYS`] while the repository continued.
pub fn abandoned_areas(
    file_history: &[FileHistoryRecord],
    latest: Timestamp,
) -> Vec<AbandonedArea> {
    let mut areas: BTreeMap<String, (Timestamp, u64)> = BTreeMap::new();
    for record in file_history {
        let entry = areas
            .entry(area_of(&record.path))
            .or_insert((record.last_changed, 0));
        entry.0 = entry.0.max(record.last_changed);
        entry.1 += 1;
    }
    let mut abandoned: Vec<AbandonedArea> = areas
        .into_iter()
        .filter(|(area, _)| area != "(root)")
        .filter_map(|(path, (last_changed, files))| {
            let days = last_changed.days_until(latest);
            (days >= ABANDONED_DAYS && files >= MIN_ABANDONED_FILES).then(|| AbandonedArea {
                description: format!("No changes detected since {}.", last_changed.date_string()),
                path,
                last_changed,
                days_before_latest: days,
                files,
            })
        })
        .collect();
    abandoned.sort_by(|a, b| {
        b.days_before_latest
            .cmp(&a.days_before_latest)
            .then_with(|| a.path.cmp(&b.path))
    });
    abandoned.truncate(MAX_ABANDONED);
    abandoned
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(
        path: &str,
        first: (i64, u32, u32),
        last: (i64, u32, u32),
        commits: u32,
        recent: u32,
    ) -> FileHistoryRecord {
        FileHistoryRecord {
            path: path.to_owned(),
            commits,
            authors: 1,
            insertions: 10,
            deletions: 1,
            first_seen: Timestamp::from_ymd(first.0, first.1, first.2).unwrap(),
            last_changed: Timestamp::from_ymd(last.0, last.1, last.2).unwrap(),
            recent_commits: recent,
            previous_paths: Vec::new(),
        }
    }

    #[test]
    fn classifies_module_ages() {
        let history = [
            record("core/a.rs", (2018, 1, 1), (2024, 1, 1), 50, 2),
            record("api/a.rs", (2021, 1, 1), (2024, 5, 1), 20, 10),
            record("cli/a.rs", (2021, 6, 1), (2022, 1, 1), 20, 0),
            record("web/a.ts", (2024, 3, 1), (2024, 5, 1), 5, 5),
            record("new/a.ts", (2024, 5, 20), (2024, 5, 30), 1, 1),
        ];
        let modules: Vec<(String, Vec<String>)> = ["core", "api", "cli", "web", "new", "empty"]
            .iter()
            .map(|m| {
                (
                    (*m).to_owned(),
                    vec![format!("{m}/a.rs"), format!("{m}/a.ts")],
                )
            })
            .collect();
        let ages = module_ages(
            &modules,
            &history,
            Timestamp::from_ymd(2018, 1, 1).unwrap(),
            Timestamp::from_ymd(2024, 6, 1).unwrap(),
        );
        let classes: Vec<(&str, AgeClass)> =
            ages.iter().map(|a| (a.module.as_str(), a.class)).collect();
        assert_eq!(
            classes,
            vec![
                ("core", AgeClass::Ancient),
                ("api", AgeClass::Growing),
                ("cli", AgeClass::Established),
                ("web", AgeClass::Recent),
                ("new", AgeClass::New),
            ]
        );
        assert!(ages[0].reason.starts_with("First changed on 2018-01-01"));
    }

    #[test]
    fn finds_areas_that_stopped_changing() {
        let history = [
            record("legacy/a.py", (2019, 1, 1), (2020, 1, 1), 3, 0),
            record("legacy/b.py", (2019, 1, 1), (2020, 2, 1), 3, 0),
            record("legacy/c.py", (2019, 1, 1), (2020, 3, 1), 3, 0),
            record("small/a.py", (2019, 1, 1), (2019, 1, 1), 1, 0),
            record("src/a.py", (2019, 1, 1), (2024, 5, 1), 30, 5),
            record("src/b.py", (2019, 1, 1), (2024, 5, 1), 30, 5),
            record("src/c.py", (2019, 1, 1), (2024, 5, 1), 30, 5),
        ];
        let abandoned = abandoned_areas(&history, Timestamp::from_ymd(2024, 6, 1).unwrap());
        assert_eq!(abandoned.len(), 1);
        assert_eq!(abandoned[0].path, "legacy");
        assert_eq!(abandoned[0].files, 3);
        assert_eq!(
            abandoned[0].description,
            "No changes detected since 2020-03-01."
        );
    }
}
