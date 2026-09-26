//! Development epochs: contiguous periods separated by long gaps or calendar years.

use std::collections::{BTreeMap, HashSet};

use repodna_core::model::evolution::{Epoch, EpochKind};
use repodna_core::time::{SECONDS_PER_DAY, Timestamp};
use repodna_git::{History, ParsedCommit, area_of, contributor_id};

/// A pause of at least this many days starts a new epoch.
pub const EPOCH_GAP_DAYS: i64 = 90;

/// Periods longer than this are split at calendar-year boundaries.
const SPLIT_LONGER_THAN_DAYS: i64 = 540;

/// Year parts shorter than this are merged into a neighbor.
const MIN_PART_DAYS: i64 = 90;

/// Maximum epochs; the smallest neighbors are merged beyond this.
pub const MAX_EPOCHS: usize = 12;

/// A period counts as active when its commit rate reaches this share of the median rate.
const ACTIVE_SHARE_OF_MEDIAN: f64 = 0.8;

/// Focus areas listed per epoch.
const FOCUS_AREAS: usize = 3;

/// How epochs are formed, for reports.
pub const EPOCH_METHOD: &str = "Commits are grouped into periods separated by pauses of at least \
90 days; periods longer than about 18 months are split at calendar years. The first period is \
labeled initial and the last current; the others are labeled active when their commit rate is at \
least 80% of the median rate, and maintenance otherwise.";

fn span_days(commits: &[&ParsedCommit]) -> i64 {
    match (commits.first(), commits.last()) {
        (Some(first), Some(last)) => (last.timestamp - first.timestamp) / SECONDS_PER_DAY,
        _ => 0,
    }
}

/// Splits a long period at calendar years, merging short parts into their neighbors.
fn split_by_year(segment: Vec<&ParsedCommit>) -> Vec<Vec<&ParsedCommit>> {
    if span_days(&segment) <= SPLIT_LONGER_THAN_DAYS {
        return vec![segment];
    }
    let mut parts: Vec<Vec<&ParsedCommit>> = Vec::new();
    let mut current_year = None;
    for commit in segment {
        let year = Timestamp::from_unix(commit.timestamp).year();
        if current_year != Some(year) {
            parts.push(Vec::new());
            current_year = Some(year);
        }
        if let Some(part) = parts.last_mut() {
            part.push(commit);
        }
    }
    let mut merged: Vec<Vec<&ParsedCommit>> = Vec::new();
    for part in parts {
        match merged.last_mut() {
            Some(previous)
                if span_days(&part) < MIN_PART_DAYS || span_days(previous) < MIN_PART_DAYS =>
            {
                previous.extend(part);
            }
            _ => merged.push(part),
        }
    }
    merged
}

/// Detects development epochs, oldest first.
pub fn detect_epochs(history: &History) -> Vec<Epoch> {
    let oldest_first: Vec<&ParsedCommit> = history.commits.iter().rev().collect();
    if oldest_first.is_empty() {
        return Vec::new();
    }
    let mut segments: Vec<Vec<&ParsedCommit>> = vec![Vec::new()];
    let mut previous: Option<i64> = None;
    for commit in oldest_first {
        if let Some(previous) = previous
            && (commit.timestamp - previous) / SECONDS_PER_DAY >= EPOCH_GAP_DAYS
        {
            segments.push(Vec::new());
        }
        previous = Some(commit.timestamp);
        if let Some(segment) = segments.last_mut() {
            segment.push(commit);
        }
    }
    let mut periods: Vec<Vec<&ParsedCommit>> =
        segments.into_iter().flat_map(split_by_year).collect();
    while periods.len() > MAX_EPOCHS {
        let smallest = (0..periods.len() - 1)
            .min_by_key(|&i| periods[i].len() + periods[i + 1].len())
            .unwrap_or(0);
        let next = periods.remove(smallest + 1);
        periods[smallest].extend(next);
    }

    let rates: Vec<f64> = periods
        .iter()
        .map(|period| period.len() as f64 * 30.0 / (span_days(period).max(1) as f64))
        .collect();
    let mut sorted_rates = rates.clone();
    sorted_rates.sort_by(f64::total_cmp);
    let median = sorted_rates[sorted_rates.len() / 2];
    let count = periods.len();
    periods
        .iter()
        .enumerate()
        .map(|(index, commits)| {
            let kind = if index + 1 == count {
                EpochKind::Current
            } else if index == 0 {
                EpochKind::Initial
            } else if rates[index] >= ACTIVE_SHARE_OF_MEDIAN * median {
                EpochKind::Active
            } else {
                EpochKind::Maintenance
            };
            let label = match kind {
                EpochKind::Initial => "Initial development",
                EpochKind::Active => "Active development",
                EpochKind::Maintenance => "Maintenance",
                EpochKind::Current => "Current phase",
            };
            let mut areas: BTreeMap<String, u64> = BTreeMap::new();
            let mut authors = HashSet::new();
            let (mut insertions, mut deletions) = (0u64, 0u64);
            for commit in commits {
                authors.insert(contributor_id(&commit.author_email, &commit.author_name));
                for change in &commit.changes {
                    insertions += u64::from(change.insertions);
                    deletions += u64::from(change.deletions);
                    *areas.entry(area_of(&change.path)).or_default() +=
                        1 + u64::from(change.insertions) + u64::from(change.deletions);
                }
            }
            let mut focus: Vec<(String, u64)> = areas.into_iter().collect();
            focus.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            Epoch {
                index: u32::try_from(index).unwrap_or(u32::MAX),
                label: label.to_owned(),
                kind,
                start: Timestamp::from_unix(commits[0].timestamp),
                end: Timestamp::from_unix(commits[commits.len() - 1].timestamp),
                commits: commits.len() as u64,
                contributors: u32::try_from(authors.len()).unwrap_or(u32::MAX),
                insertions,
                deletions,
                focus_areas: focus
                    .into_iter()
                    .take(FOCUS_AREAS)
                    .map(|(area, _)| area)
                    .collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{commit, history};

    #[test]
    fn splits_at_long_gaps_and_labels_activity() {
        let mut commits = Vec::new();
        for day in 0..10 {
            commits.push(commit(
                &format!("a{day}"),
                &format!("2020-01-{:02}", day * 3 + 1),
                "ana",
                &["~src/core/lib.rs"],
            ));
        }
        for day in 0..60 {
            let date = format!("2020-{:02}-{:02}", 10 + day / 28, day % 28 + 1);
            let author = if day % 2 == 0 { "ana" } else { "bo" };
            commits.push(commit(
                &format!("b{day}"),
                &date,
                author,
                &["~web/app.ts", "~web/ui.ts"],
            ));
        }
        for day in 0..3 {
            commits.push(commit(
                &format!("c{day}"),
                &format!("2021-06-{:02}", day + 1),
                "cy",
                &["~docs/guide.md"],
            ));
        }
        let epochs = detect_epochs(&history(commits));
        let summary: Vec<(EpochKind, u64, u32, &str)> = epochs
            .iter()
            .map(|e| (e.kind, e.commits, e.contributors, e.focus_areas[0].as_str()))
            .collect();
        assert_eq!(
            summary,
            vec![
                (EpochKind::Initial, 10, 1, "src/core"),
                (EpochKind::Active, 60, 2, "web"),
                (EpochKind::Current, 3, 1, "docs"),
            ]
        );
        assert_eq!(epochs[1].label, "Active development");
        assert_eq!(epochs[0].start.date_string(), "2020-01-01");
    }

    #[test]
    fn splits_long_continuous_periods_by_year() {
        let commits: Vec<_> = (0..36)
            .map(|month| {
                let date = format!("{}-{:02}-10", 2019 + month / 12, month % 12 + 1);
                commit(&format!("m{month}"), &date, "ana", &["~lib/a.rb"])
            })
            .collect();
        let epochs = detect_epochs(&history(commits));
        assert_eq!(epochs.len(), 3);
        assert_eq!(epochs.iter().map(|e| e.commits).sum::<u64>(), 36);
        assert_eq!(epochs[1].kind, EpochKind::Active);
        assert!(detect_epochs(&history(Vec::new())).is_empty());
        let single = detect_epochs(&history(vec![commit(
            "x",
            "2024-01-01",
            "ana",
            &["+a.txt"],
        )]));
        assert_eq!(single[0].kind, EpochKind::Current);
    }
}
