//! Aggregation of parsed history into the Git section of the artifact.
//!
//! All windows that affect deterministic results ("recent" churn, recent commits) are
//! measured backwards from the latest commit, so the same revision always produces the
//! same numbers. Only the activity summary, which answers "is this project active now?",
//! is measured against the analysis time, and it records that reference time.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use repodna_core::Timestamp;
use repodna_core::hash::stable_id;
use repodna_core::model::SectionStatus;
use repodna_core::model::git::{
    ActivityLevel, ActivitySummary, AreaShare, CommitRecord, CommitRef, ContributorRecord,
    DailyActivity, DirectoryActivity, DormantPeriod, FileHistoryRecord, GitReport,
    OwnershipSummary, ReleaseInfo, TagInfo, TimelineBucket,
};
use repodna_core::paths;
use repodna_core::time::SECONDS_PER_DAY;

use crate::log::{ChangeStatus, History, ParsedCommit};

/// Options for [`aggregate`].
#[derive(Debug, Clone)]
pub struct HistoryOptions {
    /// Time the activity summary is measured against (normally the analysis time).
    pub reference_time: Timestamp,
    /// Size of the recent window, in days before the latest commit.
    pub recent_days: u32,
    /// Minimum gap, in days, reported as a dormant period.
    pub dormant_days: u32,
    /// Maximum commits stored in the artifact.
    pub artifact_commits: usize,
    /// Store commit subject lines.
    pub include_messages: bool,
}

impl Default for HistoryOptions {
    fn default() -> Self {
        Self {
            reference_time: Timestamp::now(),
            recent_days: 90,
            dormant_days: 90,
            artifact_commits: 1_000,
            include_messages: true,
        }
    }
}

/// Maximum months in the timeline before it falls back to months with commits only.
const MAX_TIMELINE_MONTHS: usize = 1_200;
/// Maximum dormant periods reported.
const MAX_DORMANT_PERIODS: usize = 50;

/// A stable, pseudonymous contributor identifier derived from the normalized e-mail address
/// (or the name when no address is recorded). The address itself is never stored.
pub fn contributor_id(email: &str, name: &str) -> String {
    let email = email.trim().to_lowercase();
    if email.is_empty() {
        format!("c-{}", stable_id(&["name", name.trim()]))
    } else {
        format!("c-{}", stable_id(&["email", email.as_str()]))
    }
}

fn short(hash: &str) -> String {
    hash.chars().take(7).collect()
}

fn commit_ref(commit: &ParsedCommit) -> CommitRef {
    CommitRef {
        hash: commit.hash.clone(),
        short: short(&commit.hash),
        timestamp: Timestamp::from_unix(commit.timestamp),
    }
}

/// The directory used to group activity: at most the first two path components of the
/// file's parent directory, or `(root)` for files at the repository root.
pub fn area_of(path: &str) -> String {
    let parent = paths::parent(path);
    if parent.is_empty() {
        "(root)".to_owned()
    } else {
        paths::prefix(parent, 2).to_owned()
    }
}

#[derive(Default)]
struct FileAccumulator {
    commits: u32,
    authors: HashSet<String>,
    insertions: u64,
    deletions: u64,
    first_seen: i64,
    last_changed: i64,
    recent: u32,
    previous: Vec<String>,
}

impl FileAccumulator {
    fn starting(timestamp: i64) -> Self {
        Self {
            first_seen: timestamp,
            last_changed: timestamp,
            ..Self::default()
        }
    }
}

#[derive(Default)]
struct ContributorAccumulator {
    names: BTreeMap<String, u64>,
    commits: u64,
    insertions: u64,
    deletions: u64,
    first: i64,
    last: i64,
    days: HashSet<String>,
    areas: HashMap<String, u64>,
}

/// Aggregates history into a [`GitReport`]. Repository state (HEAD, branches, tags,
/// remotes) and hotspots are filled in by the caller.
pub fn aggregate(
    history: &History,
    current_files: &BTreeSet<String>,
    options: &HistoryOptions,
) -> GitReport {
    let mut report = GitReport {
        status: SectionStatus::Analyzed,
        recent_window_days: options.recent_days,
        commit_count: history.commits.len() as u64,
        history_truncated: history.truncated,
        ..GitReport::default()
    };
    if history.commits.is_empty() {
        report
            .notes
            .push("No commits were found; the repository has no history yet.".to_owned());
        report.activity.description = "No commits found.".to_owned();
        return report;
    }
    report.last_commit = history.commits.first().map(commit_ref);
    report.first_commit = history.commits.last().map(commit_ref);
    let latest = history
        .commits
        .iter()
        .map(|c| c.timestamp)
        .max()
        .unwrap_or(0);
    let recent_cutoff = latest - i64::from(options.recent_days) * SECONDS_PER_DAY;

    report.commits = history
        .commits
        .iter()
        .take(options.artifact_commits)
        .map(|commit| CommitRecord {
            hash: commit.hash.clone(),
            short: short(&commit.hash),
            author: contributor_id(&commit.author_email, &commit.author_name),
            timestamp: Timestamp::from_unix(commit.timestamp),
            subject: if options.include_messages {
                commit.subject.chars().take(200).collect()
            } else {
                String::new()
            },
            files_changed: u32::try_from(commit.changes.len()).unwrap_or(u32::MAX),
            insertions: commit.insertions(),
            deletions: commit.deletions(),
            merge: commit.is_merge(),
        })
        .collect();

    let mut files: HashMap<String, FileAccumulator> = HashMap::new();
    let mut contributors: HashMap<String, ContributorAccumulator> = HashMap::new();
    let mut directories: BTreeMap<String, (u64, u64, HashSet<String>, i64, u64)> = BTreeMap::new();
    let mut months: BTreeMap<String, (i64, u64, HashSet<String>, u64, u64)> = BTreeMap::new();
    let mut days: BTreeMap<String, (u32, u64)> = BTreeMap::new();
    let mut weekday_hour = vec![vec![0u32; 24]; 7];
    let mut recent_churn_total = 0u64;

    // Oldest first, so renames carry history forward to the current path.
    for commit in history.commits.iter().rev() {
        let author = contributor_id(&commit.author_email, &commit.author_name);
        let timestamp = commit.timestamp;
        let when = Timestamp::from_unix(timestamp);
        let local = Timestamp::from_unix(timestamp + i64::from(commit.offset_minutes) * 60);
        let churn = commit.insertions() + commit.deletions();
        let is_recent = timestamp >= recent_cutoff;

        let contributor =
            contributors
                .entry(author.clone())
                .or_insert_with(|| ContributorAccumulator {
                    first: timestamp,
                    last: timestamp,
                    ..ContributorAccumulator::default()
                });
        *contributor
            .names
            .entry(commit.author_name.clone())
            .or_default() += 1;
        contributor.commits += 1;
        contributor.insertions += commit.insertions();
        contributor.deletions += commit.deletions();
        contributor.first = contributor.first.min(timestamp);
        contributor.last = contributor.last.max(timestamp);
        contributor.days.insert(when.date_string());

        let month = months
            .entry(when.month_key())
            .or_insert_with(|| (when.start_of_month().unix(), 0, HashSet::new(), 0, 0));
        month.1 += 1;
        month.2.insert(author.clone());
        month.3 += commit.insertions();
        month.4 += commit.deletions();

        let day = days.entry(when.date_string()).or_default();
        day.0 += 1;
        day.1 += churn;

        let weekday = local.weekday() as usize;
        let hour = local.hour() as usize;
        weekday_hour[weekday.min(6)][hour.min(23)] += 1;

        let mut touched_areas: BTreeSet<String> = BTreeSet::new();
        for change in &commit.changes {
            let change_churn = u64::from(change.insertions) + u64::from(change.deletions);
            if is_recent {
                recent_churn_total += change_churn;
            }
            for area in activity_areas(&change.path) {
                let entry = directories
                    .entry(area.clone())
                    .or_insert_with(|| (0, 0, HashSet::new(), timestamp, 0));
                if touched_areas.insert(area) {
                    entry.0 += 1;
                }
                entry.1 += change_churn;
                entry.2.insert(author.clone());
                entry.3 = entry.3.max(timestamp);
                if is_recent {
                    entry.4 += change_churn;
                }
            }
            match change.status {
                ChangeStatus::Deleted => {
                    files.remove(&change.path);
                }
                ChangeStatus::Renamed => {
                    let old = change.old_path.clone().unwrap_or_default();
                    let mut accumulator = files
                        .remove(&old)
                        .unwrap_or_else(|| FileAccumulator::starting(timestamp));
                    accumulator.previous.push(old);
                    update(
                        &mut accumulator,
                        &author,
                        change.insertions,
                        change.deletions,
                        timestamp,
                        is_recent,
                    );
                    files.insert(change.path.clone(), accumulator);
                }
                _ => {
                    let accumulator = files
                        .entry(change.path.clone())
                        .or_insert_with(|| FileAccumulator::starting(timestamp));
                    update(
                        accumulator,
                        &author,
                        change.insertions,
                        change.deletions,
                        timestamp,
                        is_recent,
                    );
                }
            }
        }
        let contributor = contributors.entry(author).or_default();
        for area in touched_areas {
            *contributor.areas.entry(area).or_default() += 1;
        }
    }

    report.file_history = files
        .into_iter()
        .filter(|(path, _)| current_files.contains(path))
        .map(|(path, accumulator)| FileHistoryRecord {
            path,
            commits: accumulator.commits,
            authors: u32::try_from(accumulator.authors.len()).unwrap_or(u32::MAX),
            insertions: accumulator.insertions,
            deletions: accumulator.deletions,
            first_seen: Timestamp::from_unix(accumulator.first_seen),
            last_changed: Timestamp::from_unix(accumulator.last_changed),
            recent_commits: accumulator.recent,
            previous_paths: accumulator.previous,
        })
        .collect();
    report.file_history.sort_by(|a, b| a.path.cmp(&b.path));

    report.directory_activity = directories
        .into_iter()
        .map(
            |(path, (commits, churn, authors, last, recent))| DirectoryActivity {
                path,
                commits,
                churn,
                authors: u32::try_from(authors.len()).unwrap_or(u32::MAX),
                last_changed: Timestamp::from_unix(last),
                recent_churn_share: if recent_churn_total == 0 {
                    0.0
                } else {
                    round4(recent as f64 / recent_churn_total as f64)
                },
            },
        )
        .collect();

    let mut contributor_records: Vec<ContributorRecord> = contributors
        .into_iter()
        .filter(|(_, accumulator)| accumulator.commits > 0)
        .map(|(id, accumulator)| {
            let name = accumulator
                .names
                .iter()
                .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
                .map(|(name, _)| name.clone())
                .unwrap_or_default();
            let mut areas: Vec<AreaShare> = accumulator
                .areas
                .into_iter()
                .map(|(path, commits)| AreaShare { path, commits })
                .collect();
            areas.sort_by(|a, b| b.commits.cmp(&a.commits).then_with(|| a.path.cmp(&b.path)));
            areas.truncate(5);
            ContributorRecord {
                id,
                name,
                commits: accumulator.commits,
                insertions: accumulator.insertions,
                deletions: accumulator.deletions,
                first_commit: Timestamp::from_unix(accumulator.first),
                last_commit: Timestamp::from_unix(accumulator.last),
                active_days: u32::try_from(accumulator.days.len()).unwrap_or(u32::MAX),
                areas,
            }
        })
        .collect();
    contributor_records.sort_by(|a, b| {
        b.commits
            .cmp(&a.commits)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.id.cmp(&b.id))
    });
    report.ownership = ownership(&contributor_records);
    report.contributors = contributor_records;

    report.timeline = timeline(&months);
    report.daily_activity = days
        .into_iter()
        .map(|(date, (commits, churn))| DailyActivity {
            date,
            commits,
            churn,
        })
        .collect();
    report.weekday_hour = weekday_hour;
    report.activity = activity(history, options.reference_time);
    report.dormant_periods = dormant_periods(history, options.dormant_days);
    report
}

fn update(
    accumulator: &mut FileAccumulator,
    author: &str,
    insertions: u32,
    deletions: u32,
    timestamp: i64,
    recent: bool,
) {
    accumulator.commits += 1;
    accumulator.authors.insert(author.to_owned());
    accumulator.insertions += u64::from(insertions);
    accumulator.deletions += u64::from(deletions);
    accumulator.first_seen = accumulator.first_seen.min(timestamp);
    accumulator.last_changed = accumulator.last_changed.max(timestamp);
    if recent {
        accumulator.recent += 1;
    }
}

/// Directories credited with a change: the top-level directory and, when deeper, the
/// first two levels. Root-level files are credited to `(root)`.
fn activity_areas(path: &str) -> Vec<String> {
    let parent = paths::parent(path);
    if parent.is_empty() {
        return vec!["(root)".to_owned()];
    }
    let mut areas = vec![paths::prefix(parent, 1).to_owned()];
    if paths::depth(parent) >= 2 {
        areas.push(paths::prefix(parent, 2).to_owned());
    }
    areas
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn timeline(
    months: &BTreeMap<String, (i64, u64, HashSet<String>, u64, u64)>,
) -> Vec<TimelineBucket> {
    let bucket = |key: &str, start: i64, data: Option<&(i64, u64, HashSet<String>, u64, u64)>| {
        TimelineBucket {
            period: key.to_owned(),
            start: Timestamp::from_unix(start),
            commits: data.map_or(0, |d| d.1),
            authors: data.map_or(0, |d| u32::try_from(d.2.len()).unwrap_or(u32::MAX)),
            insertions: data.map_or(0, |d| d.3),
            deletions: data.map_or(0, |d| d.4),
        }
    };
    let (Some((_, first)), Some((_, last))) = (months.iter().next(), months.iter().next_back())
    else {
        return Vec::new();
    };
    let mut buckets = Vec::new();
    let mut cursor = Timestamp::from_unix(first.0);
    let end = Timestamp::from_unix(last.0);
    while cursor <= end && buckets.len() < MAX_TIMELINE_MONTHS {
        let key = cursor.month_key();
        buckets.push(bucket(&key, cursor.unix(), months.get(&key)));
        cursor = cursor.plus_months(1);
    }
    if cursor <= end {
        // Implausibly long spans (e.g. commits dated 1970): keep only months with commits.
        return months
            .iter()
            .map(|(key, data)| bucket(key, data.0, Some(data)))
            .collect();
    }
    buckets
}

fn activity(history: &History, reference: Timestamp) -> ActivitySummary {
    let reference_unix = reference.unix();
    let count_since = |days: i64| {
        history
            .commits
            .iter()
            .filter(|commit| commit.timestamp >= reference_unix - days * SECONDS_PER_DAY)
            .count() as u64
    };
    let latest = history
        .commits
        .iter()
        .map(|c| c.timestamp)
        .max()
        .unwrap_or(0);
    let (last_30, last_90, last_365) = (count_since(30), count_since(90), count_since(365));
    let level = if history.commits.is_empty() {
        ActivityLevel::None
    } else if last_30 >= 30 {
        ActivityLevel::VeryActive
    } else if last_30 >= 5 {
        ActivityLevel::Active
    } else if last_90 >= 1 {
        ActivityLevel::Moderate
    } else if last_365 >= 1 {
        ActivityLevel::Low
    } else {
        ActivityLevel::Dormant
    };
    let latest_date = Timestamp::from_unix(latest).date_string();
    let description = match level {
        ActivityLevel::VeryActive | ActivityLevel::Active => {
            format!(
                "{last_30} commits in the last 30 days; the latest commit is from {latest_date}."
            )
        }
        ActivityLevel::Moderate => {
            format!(
                "{last_90} commits in the last 90 days; the latest commit is from {latest_date}."
            )
        }
        ActivityLevel::Low => {
            format!(
                "{last_365} commits in the last 12 months; the latest commit is from {latest_date}."
            )
        }
        ActivityLevel::Dormant => format!(
            "No commits detected in the last 12 months. No activity detected after {latest_date}."
        ),
        ActivityLevel::None => "No commits found.".to_owned(),
    };
    ActivitySummary {
        level,
        reference_time: Some(reference),
        commits_last_30_days: last_30,
        commits_last_90_days: last_90,
        commits_last_365_days: last_365,
        days_since_last_commit: Some(((reference_unix - latest) / SECONDS_PER_DAY).max(0)),
        description,
    }
}

fn dormant_periods(history: &History, dormant_days: u32) -> Vec<DormantPeriod> {
    let mut points: Vec<(i64, &str)> = history
        .commits
        .iter()
        .map(|commit| (commit.timestamp, commit.hash.as_str()))
        .collect();
    points.sort();
    let threshold = i64::from(dormant_days) * SECONDS_PER_DAY;
    let mut periods: Vec<DormantPeriod> = points
        .windows(2)
        .filter(|pair| pair[1].0 - pair[0].0 >= threshold)
        .map(|pair| DormantPeriod {
            start: Timestamp::from_unix(pair[0].0),
            end: Timestamp::from_unix(pair[1].0),
            days: (pair[1].0 - pair[0].0) / SECONDS_PER_DAY,
            before_commit: pair[0].1.to_owned(),
            after_commit: pair[1].1.to_owned(),
        })
        .collect();
    if periods.len() > MAX_DORMANT_PERIODS {
        periods.sort_by(|a, b| b.days.cmp(&a.days).then_with(|| a.start.cmp(&b.start)));
        periods.truncate(MAX_DORMANT_PERIODS);
        periods.sort_by(|a, b| a.start.cmp(&b.start));
    }
    periods
}

fn ownership(contributors: &[ContributorRecord]) -> OwnershipSummary {
    let total: u64 = contributors.iter().map(|c| c.commits).sum();
    if total == 0 {
        return OwnershipSummary::default();
    }
    let mut cumulative = 0u64;
    let mut half = 0u32;
    for contributor in contributors {
        cumulative += contributor.commits;
        half += 1;
        if cumulative * 2 >= total {
            break;
        }
    }
    let count = u32::try_from(contributors.len()).unwrap_or(u32::MAX);
    OwnershipSummary {
        contributors: count,
        contributors_for_half_of_commits: half,
        top_contributor_share: round4(contributors[0].commits as f64 / total as f64),
        note: format!(
            "{half} of {count} contributor(s) authored at least half of the analyzed commits. This describes the recorded history; it is not a judgment about individuals."
        ),
    }
}

/// Parses a release-like tag such as `v1.2.3`, `2.0`, or `v1.0.0-rc.1`, returning the
/// version without its `v` prefix.
pub fn parse_version(tag: &str) -> Option<String> {
    let version = tag.strip_prefix(['v', 'V']).unwrap_or(tag);
    let core_end = version
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(version.len());
    let core = &version[..core_end];
    let rest = &version[core_end..];
    let valid_core = !core.is_empty()
        && core
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        && core.split('.').count() <= 4;
    let valid_rest = rest.is_empty()
        || ((rest.starts_with('-') || rest.starts_with('+'))
            && rest.len() > 1
            && rest[1..]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+')));
    (valid_core && valid_rest).then(|| version.to_owned())
}

/// Builds release information from version-like tags, oldest first.
pub fn releases(tags: &[TagInfo], history: &History) -> Vec<ReleaseInfo> {
    let index: HashMap<&str, usize> = history
        .commits
        .iter()
        .enumerate()
        .map(|(position, commit)| (commit.hash.as_str(), position))
        .collect();
    let mut versioned: Vec<(&TagInfo, String)> = tags
        .iter()
        .filter_map(|tag| parse_version(&tag.name).map(|version| (tag, version)))
        .collect();
    versioned.sort_by(|a, b| {
        a.0.date
            .cmp(&b.0.date)
            .then_with(|| a.0.name.cmp(&b.0.name))
    });
    let mut previous: Option<usize> = None;
    versioned
        .into_iter()
        .map(|(tag, version)| {
            let position = index.get(tag.commit.as_str()).copied();
            let commits_since_previous = match (position, previous) {
                (Some(current), Some(previous)) => previous.saturating_sub(current) as u64,
                (Some(current), None) => (history.commits.len() - current) as u64,
                _ => 0,
            };
            if position.is_some() {
                previous = position;
            }
            ReleaseInfo {
                tag: tag.name.clone(),
                version,
                date: tag.date,
                commit: tag.commit.clone(),
                commits_since_previous,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::FileChange;

    fn change(path: &str, status: ChangeStatus, insertions: u32, old: Option<&str>) -> FileChange {
        FileChange {
            path: path.into(),
            old_path: old.map(Into::into),
            status,
            insertions,
            deletions: 0,
            binary: false,
        }
    }

    fn commit(hash: char, author: &str, date: &str, changes: Vec<FileChange>) -> ParsedCommit {
        ParsedCommit {
            hash: hash.to_string().repeat(40),
            parents: vec![],
            author_name: author.into(),
            author_email: format!("{}@example.invalid", author.to_lowercase()),
            timestamp: Timestamp::parse_rfc3339(date).unwrap().unix(),
            offset_minutes: 0,
            subject: format!("commit {hash}"),
            changes,
        }
    }

    fn sample_history() -> History {
        // Newest first.
        History {
            commits: vec![
                commit(
                    'd',
                    "Ada",
                    "2024-06-10T10:00:00Z",
                    vec![change("src/core/app.rs", ChangeStatus::Modified, 5, None)],
                ),
                commit(
                    'c',
                    "Grace",
                    "2024-06-01T09:00:00Z",
                    vec![
                        change(
                            "src/core/app.rs",
                            ChangeStatus::Renamed,
                            2,
                            Some("src/main.rs"),
                        ),
                        change("old.txt", ChangeStatus::Deleted, 0, None),
                    ],
                ),
                commit(
                    'b',
                    "Ada",
                    "2024-01-15T12:00:00Z",
                    vec![
                        change("src/main.rs", ChangeStatus::Modified, 3, None),
                        change("README.md", ChangeStatus::Added, 10, None),
                    ],
                ),
                commit(
                    'a',
                    "Ada",
                    "2024-01-01T08:00:00Z",
                    vec![
                        change("src/main.rs", ChangeStatus::Added, 20, None),
                        change("old.txt", ChangeStatus::Added, 1, None),
                    ],
                ),
            ],
            truncated: false,
        }
    }

    fn options() -> HistoryOptions {
        HistoryOptions {
            reference_time: Timestamp::parse_rfc3339("2024-06-20T00:00:00Z").unwrap(),
            recent_days: 30,
            dormant_days: 90,
            artifact_commits: 3,
            include_messages: true,
        }
    }

    #[test]
    fn aggregates_contributors_and_files_following_renames() {
        let current: BTreeSet<String> = ["src/core/app.rs", "README.md"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let report = aggregate(&sample_history(), &current, &options());
        assert_eq!(report.commit_count, 4);
        assert_eq!(report.commits.len(), 3);
        assert_eq!(report.first_commit.as_ref().unwrap().hash, "a".repeat(40));
        assert_eq!(report.last_commit.as_ref().unwrap().hash, "d".repeat(40));

        let app = report
            .file_history
            .iter()
            .find(|f| f.path == "src/core/app.rs")
            .unwrap();
        assert_eq!(app.commits, 4, "history follows the rename");
        assert_eq!(app.authors, 2);
        assert_eq!(app.insertions, 30);
        assert_eq!(app.first_seen.date_string(), "2024-01-01");
        assert_eq!(app.previous_paths, vec!["src/main.rs"]);
        assert_eq!(app.recent_commits, 2);
        assert!(
            report.file_history.iter().all(|f| f.path != "old.txt"),
            "deleted files are dropped"
        );

        assert_eq!(report.contributors.len(), 2);
        assert_eq!(report.contributors[0].name, "Ada");
        assert_eq!(report.contributors[0].commits, 3);
        assert!(report.contributors[0].id.starts_with("c-"));
        assert_eq!(report.ownership.contributors_for_half_of_commits, 1);
        assert_eq!(report.ownership.top_contributor_share, 0.75);
    }

    #[test]
    fn builds_continuous_timelines_and_heatmaps() {
        let report = aggregate(&sample_history(), &BTreeSet::new(), &options());
        let periods: Vec<_> = report.timeline.iter().map(|b| b.period.as_str()).collect();
        assert_eq!(
            periods,
            vec![
                "2024-01", "2024-02", "2024-03", "2024-04", "2024-05", "2024-06"
            ]
        );
        assert_eq!(report.timeline[0].commits, 2);
        assert_eq!(report.timeline[1].commits, 0);
        assert_eq!(report.daily_activity.len(), 4);
        let heat_total: u32 = report.weekday_hour.iter().flatten().sum();
        assert_eq!(heat_total, 4);
        let src = report
            .directory_activity
            .iter()
            .find(|d| d.path == "src")
            .unwrap();
        assert_eq!(src.commits, 4);
        assert!(
            report
                .directory_activity
                .iter()
                .any(|d| d.path == "src/core")
        );
    }

    #[test]
    fn detects_dormancy_and_activity_levels() {
        let report = aggregate(&sample_history(), &BTreeSet::new(), &options());
        assert_eq!(report.dormant_periods.len(), 1);
        assert_eq!(report.dormant_periods[0].days, 137);
        assert_eq!(report.activity.level, ActivityLevel::Moderate);
        assert_eq!(report.activity.days_since_last_commit, Some(9));
        let later = HistoryOptions {
            reference_time: Timestamp::parse_rfc3339("2027-01-01T00:00:00Z").unwrap(),
            ..options()
        };
        let dormant = aggregate(&sample_history(), &BTreeSet::new(), &later);
        assert_eq!(dormant.activity.level, ActivityLevel::Dormant);
        assert!(
            dormant
                .activity
                .description
                .contains("No activity detected after 2024-06-10")
        );
    }

    #[test]
    fn empty_history_is_reported_honestly() {
        let report = aggregate(&History::default(), &BTreeSet::new(), &options());
        assert_eq!(report.commit_count, 0);
        assert_eq!(report.status, SectionStatus::Analyzed);
        assert!(!report.notes.is_empty());
    }

    #[test]
    fn messages_can_be_excluded() {
        let quiet = HistoryOptions {
            include_messages: false,
            ..options()
        };
        let report = aggregate(&sample_history(), &BTreeSet::new(), &quiet);
        assert!(report.commits.iter().all(|c| c.subject.is_empty()));
    }

    #[test]
    fn parses_release_versions() {
        assert_eq!(parse_version("v1.2.3").as_deref(), Some("1.2.3"));
        assert_eq!(parse_version("2.0").as_deref(), Some("2.0"));
        assert_eq!(parse_version("v1.0.0-rc.1").as_deref(), Some("1.0.0-rc.1"));
        assert_eq!(parse_version("release-candidate"), None);
        assert_eq!(parse_version("v1..2"), None);
        assert_eq!(parse_version("v"), None);
        assert_eq!(parse_version("1.2.3.4.5"), None);
    }

    #[test]
    fn computes_commits_between_releases() {
        let history = sample_history();
        let tag = |name: &str, hash: char, date: &str| TagInfo {
            name: name.into(),
            commit: hash.to_string().repeat(40),
            date: Timestamp::parse_rfc3339(date).unwrap(),
            annotated: false,
        };
        let tags = vec![
            tag("v1.0.0", 'b', "2024-01-15T12:00:00Z"),
            tag("v1.1.0", 'd', "2024-06-10T10:00:00Z"),
            tag("nightly", 'c', "2024-06-01T09:00:00Z"),
        ];
        let releases = releases(&tags, &history);
        assert_eq!(releases.len(), 2);
        assert_eq!(releases[0].commits_since_previous, 2);
        assert_eq!(releases[1].commits_since_previous, 2);
        assert_eq!(releases[1].version, "1.1.0");
    }

    #[test]
    fn contributor_ids_are_pseudonymous_and_case_insensitive() {
        let a = contributor_id("Ada@Example.invalid", "Ada");
        let b = contributor_id("ada@example.invalid ", "Ada Lovelace");
        assert_eq!(a, b);
        assert!(!a.contains("example"));
        assert_ne!(contributor_id("", "Ada"), contributor_id("", "Grace"));
    }
}
