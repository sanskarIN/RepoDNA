//! Git history, contributors, the Codebase Time Machine, and architectural evolution.

use std::collections::{BTreeMap, HashMap};

use repodna_core::model::artifact::RepositoryDna;
use repodna_core::model::evolution::{
    AgeClass, EpochKind, SnapshotKind, StatementKind, StoryStatement,
};
use repodna_core::model::git::TimelineBucket;
use repodna_core::model::insights::ChangeKind;

use super::{ContentOptions, confidence, heading, not_analyzed, notes};
use crate::charts::{Period, bars, columns, heatmap, line, timeline};
use crate::doc::{Blocks, Inline, Rich, Table, code, plain, truncate};
use crate::sections::Section;
use crate::text::{bytes, percent, span, thousands};

fn git_missing(blocks: &mut Blocks, dna: &RepositoryDna) -> bool {
    let git = &dna.git;
    if git.status.has_results() {
        return false;
    }
    not_analyzed(blocks, git.status, &git.notes)
}

/// Commit buckets at a readable granularity: months, quarters, or years.
fn aggregate(timeline: &[TimelineBucket]) -> (Vec<(String, f64)>, &'static str) {
    let key = |bucket: &TimelineBucket, period: &str| -> String {
        let year = bucket.period.get(..4).unwrap_or(&bucket.period);
        let month: u32 = bucket
            .period
            .get(5..7)
            .and_then(|m| m.parse().ok())
            .unwrap_or(1);
        match period {
            "quarter" => format!("{year} Q{}", (month - 1) / 3 + 1),
            "year" => year.to_owned(),
            _ => bucket.period.clone(),
        }
    };
    let period = match timeline.len() {
        0..=48 => "month",
        49..=144 => "quarter",
        _ => "year",
    };
    let mut buckets: BTreeMap<String, f64> = BTreeMap::new();
    for bucket in timeline {
        *buckets.entry(key(bucket, period)).or_default() += bucket.commits as f64;
    }
    (buckets.into_iter().collect(), period)
}

pub(super) fn history(blocks: &mut Blocks, dna: &RepositoryDna, options: ContentOptions) {
    heading(blocks, Section::History);
    if git_missing(blocks, dna) {
        return;
    }
    let git = &dna.git;
    blocks.stats(vec![
        ("Commits".to_owned(), thousands(git.commit_count)),
        (
            "First commit".to_owned(),
            git.first_commit
                .as_ref()
                .map_or_else(|| "–".to_owned(), |c| c.timestamp.date_string()),
        ),
        (
            "Latest commit".to_owned(),
            git.last_commit
                .as_ref()
                .map_or_else(|| "–".to_owned(), |c| c.timestamp.date_string()),
        ),
        ("Branches".to_owned(), thousands(git.branches.len() as u64)),
        ("Tags".to_owned(), thousands(git.tags.len() as u64)),
        ("Releases".to_owned(), thousands(git.releases.len() as u64)),
    ]);
    let activity = &git.activity;
    blocks.rich(vec![
        Inline::Strong(activity.level.label().to_owned()),
        Inline::Text(format!(
            ". {} {} commits in the last 30 days, {} in the last 90, and {} in the last 365.",
            activity.description,
            thousands(activity.commits_last_30_days),
            thousands(activity.commits_last_90_days),
            thousands(activity.commits_last_365_days)
        )),
    ]);
    let (points, period) = aggregate(&git.timeline);
    if options.figures {
        blocks.figure(
            columns(&points, "commits", &format!("Commits per {period}")),
            format!("Commits per {period}"),
        );
        blocks.figure(
            heatmap(&git.weekday_hour, "Commits by weekday and hour"),
            "Commits by weekday and hour, in each author's local time",
        );
    }
    let mut years: BTreeMap<String, (u64, u64, u64)> = BTreeMap::new();
    for bucket in &git.timeline {
        let entry = years
            .entry(bucket.period.get(..4).unwrap_or(&bucket.period).to_owned())
            .or_default();
        entry.0 += bucket.commits;
        entry.1 += bucket.insertions;
        entry.2 += bucket.deletions;
    }
    if !years.is_empty() {
        let mut table = Table::new(&["Year", "Commits#", "Lines added#", "Lines deleted#"]);
        for (year, (commits, added, deleted)) in years {
            table.row(vec![
                plain(year),
                plain(thousands(commits)),
                plain(thousands(added)),
                plain(thousands(deleted)),
            ]);
        }
        blocks.table(table);
    }
    if !git.releases.is_empty() {
        blocks.heading(3, "Releases", None);
        let mut table = Table::new(&["Tag", "Version", "Date", "Commits since previous#"]);
        let mut releases: Vec<_> = git.releases.iter().rev().collect();
        table.omitted = truncate(&mut releases, 20);
        for release in releases {
            table.row(vec![
                code(release.tag.clone()),
                plain(release.version.clone()),
                plain(release.date.date_string()),
                plain(thousands(release.commits_since_previous)),
            ]);
        }
        blocks.table(table);
    }
    if !git.dormant_periods.is_empty() {
        blocks.heading(3, "Quiet periods", None);
        let mut table = Table::new(&["From", "To", "Days without commits#"]);
        let mut periods: Vec<_> = git.dormant_periods.iter().collect();
        periods.sort_by(|a, b| b.days.cmp(&a.days).then_with(|| a.start.cmp(&b.start)));
        table.omitted = truncate(&mut periods, 10);
        for period in periods {
            table.row(vec![
                plain(period.start.date_string()),
                plain(period.end.date_string()),
                plain(thousands(period.days.max(0) as u64)),
            ]);
        }
        blocks.table(table);
    }
    if !git.directory_activity.is_empty() {
        blocks.heading(3, "Where changes happen", None);
        let mut table = Table::new(&[
            "Directory",
            "Commits#",
            "Lines changed#",
            "Authors#",
            "Last change",
            "Share of recent changes#",
        ]);
        let mut areas: Vec<_> = git.directory_activity.iter().collect();
        areas.sort_by(|a, b| b.commits.cmp(&a.commits).then_with(|| a.path.cmp(&b.path)));
        table.omitted = truncate(&mut areas, 20);
        for area in areas {
            table.row(vec![
                code(area.path.clone()),
                plain(thousands(area.commits)),
                plain(thousands(area.churn)),
                plain(area.authors.to_string()),
                plain(area.last_changed.date_string()),
                plain(percent(area.recent_churn_share)),
            ]);
        }
        blocks.table(table);
    }
    if !git.commits.is_empty() {
        blocks.heading(3, "Latest commits", None);
        let names: HashMap<&str, &str> = git
            .contributors
            .iter()
            .map(|c| (c.id.as_str(), c.name.as_str()))
            .collect();
        let mut table = Table::new(&[
            "Date", "Commit", "Author", "Files#", "Added#", "Deleted#", "Message",
        ]);
        let mut commits: Vec<_> = git.commits.iter().collect();
        truncate(&mut commits, 20);
        for commit in commits {
            table.row(vec![
                plain(commit.timestamp.date_string()),
                code(commit.short.clone()),
                plain(
                    names
                        .get(commit.author.as_str())
                        .map_or_else(|| commit.author.clone(), |n| (*n).to_owned()),
                ),
                plain(commit.files_changed.to_string()),
                plain(thousands(commit.insertions)),
                plain(thousands(commit.deletions)),
                plain(if commit.subject.is_empty() {
                    "(not included)".to_owned()
                } else {
                    commit.subject.clone()
                }),
            ]);
        }
        blocks.table(table);
    }
    notes(blocks, &git.notes);
}

pub(super) fn contributors(blocks: &mut Blocks, dna: &RepositoryDna, options: ContentOptions) {
    heading(blocks, Section::Contributors);
    if git_missing(blocks, dna) {
        return;
    }
    let git = &dna.git;
    let ownership = &git.ownership;
    blocks.stats(vec![
        (
            "Contributor identities".to_owned(),
            thousands(u64::from(ownership.contributors)),
        ),
        (
            "Identities behind half of the commits".to_owned(),
            thousands(u64::from(ownership.contributors_for_half_of_commits)),
        ),
        (
            "Largest share of commits".to_owned(),
            percent(ownership.top_contributor_share),
        ),
    ]);
    if !ownership.note.is_empty() {
        blocks.text(ownership.note.clone());
    }
    if options.figures && git.contributors.len() > 1 {
        let items: Vec<(String, f64)> = git
            .contributors
            .iter()
            .take(10)
            .map(|c| (c.name.clone(), c.commits as f64))
            .collect();
        blocks.figure(
            bars(&items, "commits", "Commits per contributor identity"),
            "Commits per contributor identity (up to ten)",
        );
    }
    let mut table = Table::new(&[
        "Contributor",
        "Commits#",
        "Lines added#",
        "Lines deleted#",
        "First commit",
        "Latest commit",
        "Active days#",
        "Main areas",
    ]);
    let mut rows: Vec<_> = git.contributors.iter().collect();
    table.omitted = truncate(&mut rows, 25);
    for contributor in rows {
        let areas: Vec<String> = contributor
            .areas
            .iter()
            .take(3)
            .map(|a| format!("{} ({})", a.path, a.commits))
            .collect();
        table.row(vec![
            plain(contributor.name.clone()),
            plain(thousands(contributor.commits)),
            plain(thousands(contributor.insertions)),
            plain(thousands(contributor.deletions)),
            plain(contributor.first_commit.date_string()),
            plain(contributor.last_commit.date_string()),
            plain(contributor.active_days.to_string()),
            plain(areas.join(", ")),
        ]);
    }
    blocks.table(table);
    blocks.note("These statistics describe the repository's public Git history. Identities are grouped by e-mail address after .mailmap, and the addresses themselves are not stored. Commit counts are not a measure of anyone's work or value.");
}

fn snapshot_kind(kind: SnapshotKind) -> &'static str {
    match kind {
        SnapshotKind::Initial => "First commit",
        SnapshotKind::Release => "Release",
        SnapshotKind::Sample => "Sample",
        SnapshotKind::Current => "Current",
    }
}

fn age_class(class: AgeClass) -> &'static str {
    match class {
        AgeClass::Ancient => "Ancient",
        AgeClass::Established => "Established",
        AgeClass::Growing => "Growing",
        AgeClass::Recent => "Recent",
        AgeClass::New => "New",
    }
}

pub(super) fn time_machine(blocks: &mut Blocks, dna: &RepositoryDna, options: ContentOptions) {
    heading(blocks, Section::TimeMachine);
    let evolution = &dna.evolution;
    if not_analyzed(blocks, evolution.status, &evolution.notes) {
        return;
    }
    if !evolution.sampling.is_empty() {
        blocks.note(evolution.sampling.clone());
    }
    if options.figures {
        let points: Vec<(i64, f64)> = evolution
            .growth
            .iter()
            .map(|p| (p.date.unix(), p.files as f64))
            .collect();
        blocks.figure(
            line(&points, "files", "Files over time"),
            "Files at each sampled snapshot",
        );
    }
    let mut table = Table::new(&[
        "Snapshot",
        "Date",
        "Kind",
        "Files#",
        "Size#",
        "Test files#",
        "Doc files#",
        "Main languages",
        "Modules#",
    ]);
    for snapshot in &evolution.snapshots {
        let languages: Vec<String> = snapshot
            .languages
            .iter()
            .filter(|l| l.share > 0.0)
            .take(3)
            .map(|l| format!("{} {}", l.id, percent(l.share)))
            .collect();
        table.row(vec![
            plain(snapshot.label.clone()),
            plain(snapshot.date.date_string()),
            plain(snapshot_kind(snapshot.kind)),
            plain(thousands(snapshot.files)),
            plain(bytes(snapshot.bytes)),
            plain(thousands(snapshot.test_files)),
            plain(thousands(snapshot.doc_files)),
            plain(languages.join(", ")),
            plain(
                snapshot
                    .architecture
                    .as_ref()
                    .map_or_else(|| "–".to_owned(), |a| a.modules.len().to_string()),
            ),
        ]);
    }
    blocks.table(table);
    if !evolution.module_ages.is_empty() {
        blocks.heading(3, "Module ages", None);
        let mut table = Table::new(&[
            "Module",
            "First commit",
            "Last change",
            "Age",
            "Class",
            "Why",
        ]);
        for age in &evolution.module_ages {
            table.row(vec![
                plain(age.module.clone()),
                plain(age.first_commit.date_string()),
                plain(age.last_change.date_string()),
                plain(span(age.age_days)),
                plain(age_class(age.class)),
                plain(age.reason.clone()),
            ]);
        }
        blocks.table(table);
    }
    if !evolution.abandoned_areas.is_empty() {
        blocks.heading(3, "Long-unchanged areas", None);
        let mut table = Table::new(&[
            "Area",
            "Last change",
            "Days before the latest commit#",
            "Files#",
        ]);
        for area in &evolution.abandoned_areas {
            table.row(vec![
                code(area.path.clone()),
                plain(area.last_changed.date_string()),
                plain(thousands(area.days_before_latest.max(0) as u64)),
                plain(thousands(area.files)),
            ]);
        }
        blocks.table(table);
    }
    if let Some(recent) = &dna.insights.recent_changes {
        blocks.heading(3, "What changed recently", None);
        blocks.text(format!(
            "In the last {} days of history: {} commits by {} contributors changed {} files ({} lines added, {} deleted).",
            recent.window_days,
            thousands(recent.commits),
            recent.contributors,
            thousands(recent.files_changed),
            thousands(recent.insertions),
            thousands(recent.deletions)
        ));
        blocks.list(
            recent
                .directories
                .iter()
                .take(8)
                .map(|d| {
                    vec![
                        Inline::Code(format!("{}/", d.path)),
                        Inline::Text(format!(
                            ": {} commits, {} lines changed",
                            d.commits,
                            thousands(d.churn)
                        )),
                    ]
                })
                .collect(),
        );
        for (label, files) in [
            ("Added files", &recent.added_files),
            ("Removed files", &recent.removed_files),
        ] {
            if !files.is_empty() {
                let shown: Vec<String> = files.iter().take(15).cloned().collect();
                let mut rich: Rich = vec![Inline::Text(format!("{label}: "))];
                rich.push(Inline::Code(shown.join(", ")));
                if files.len() > shown.len() {
                    rich.push(Inline::Text(format!(
                        " and {} more",
                        files.len() - shown.len()
                    )));
                }
                blocks.rich(rich);
            }
        }
        if !recent.dependency_changes.is_empty() {
            let mut table = Table::new(&[
                "Package",
                "Ecosystem",
                "Change",
                "Before",
                "After",
                "Manifest",
            ]);
            for change in &recent.dependency_changes {
                table.row(vec![
                    code(change.name.clone()),
                    plain(change.ecosystem.clone()),
                    plain(match change.change {
                        ChangeKind::Added => "Added",
                        ChangeKind::Removed => "Removed",
                        ChangeKind::Changed => "Changed",
                    }),
                    plain(change.from.clone().unwrap_or_else(|| "–".to_owned())),
                    plain(change.to.clone().unwrap_or_else(|| "–".to_owned())),
                    code(change.manifest.clone()),
                ]);
            }
            blocks.table(table);
        }
    }
}

fn statement(statement: &StoryStatement) -> Rich {
    let label = match statement.kind {
        StatementKind::Fact => "fact",
        StatementKind::Interpretation => "interpretation",
    };
    let mut rich = Vec::new();
    if let Some(date) = statement.date {
        rich.push(Inline::Strong(date.date_string()));
        rich.push(Inline::Text(" ".to_owned()));
    }
    rich.push(Inline::Text(format!("{} ({label})", statement.text)));
    rich
}

fn epoch_kind(kind: EpochKind) -> (&'static str, usize) {
    match kind {
        EpochKind::Initial => ("Initial", 0),
        EpochKind::Active => ("Active", 1),
        EpochKind::Maintenance => ("Maintenance", 2),
        EpochKind::Current => ("Current", 3),
    }
}

pub(super) fn evolution(blocks: &mut Blocks, dna: &RepositoryDna, options: ContentOptions) {
    heading(blocks, Section::Evolution);
    let evolution = &dna.evolution;
    if not_analyzed(blocks, evolution.status, &evolution.notes) {
        return;
    }
    if options.figures && !evolution.epochs.is_empty() {
        let periods: Vec<Period> = evolution
            .epochs
            .iter()
            .map(|epoch| Period {
                start: epoch.start.unix(),
                end: epoch.end.unix(),
                label: format!("{} ({} commits)", epoch.label, thousands(epoch.commits)),
                slot: epoch_kind(epoch.kind).1,
            })
            .collect();
        let events: Vec<(i64, String)> = evolution
            .events
            .iter()
            .map(|event| {
                (
                    event.date.unix(),
                    format!("{}: {}", event.date.date_string(), event.title),
                )
            })
            .collect();
        let mut kinds: Vec<(usize, String)> = evolution
            .epochs
            .iter()
            .map(|epoch| {
                let (label, slot) = epoch_kind(epoch.kind);
                (slot, label.to_owned())
            })
            .collect();
        kinds.sort();
        kinds.dedup();
        blocks.figure(
            timeline(&periods, &events, &kinds, "Epochs and events"),
            "Epochs (bands) and detected events (dots) over the history",
        );
    }
    if !evolution.epochs.is_empty() {
        blocks.heading(3, "Epochs", None);
        let mut table = Table::new(&[
            "Epoch",
            "Kind",
            "From",
            "To",
            "Commits#",
            "Contributors#",
            "Focus",
        ]);
        for epoch in &evolution.epochs {
            table.row(vec![
                plain(epoch.label.clone()),
                plain(epoch_kind(epoch.kind).0),
                plain(epoch.start.date_string()),
                plain(epoch.end.date_string()),
                plain(thousands(epoch.commits)),
                plain(epoch.contributors.to_string()),
                plain(epoch.focus_areas.join(", ")),
            ]);
        }
        blocks.table(table);
    }
    if !evolution.events.is_empty() {
        blocks.heading(3, "Events", None);
        let mut table = Table::new(&["Date", "Event", "Details", "Confidence"]);
        let mut events: Vec<_> = evolution.events.iter().collect();
        table.omitted = truncate(&mut events, 60);
        for event in events {
            table.row(vec![
                plain(event.date.date_string()),
                plain(event.title.clone()),
                plain(event.description.clone()),
                plain(confidence(event.confidence)),
            ]);
        }
        blocks.table(table);
    }
    if !evolution.story.is_empty() {
        blocks.heading(3, "The story so far", None);
        blocks.list(evolution.story.iter().map(statement).collect());
    }
    let archaeology = &evolution.archaeology;
    for (title, statements) in [
        ("Origin", &archaeology.origin),
        ("Growth", &archaeology.growth),
        ("Expansions", &archaeology.expansions),
        ("Rewrites and restructuring", &archaeology.rewrites),
        ("Inactivity", &archaeology.inactivity),
        ("Latest phase", &archaeology.final_phase),
        ("Current state", &archaeology.current_state),
    ] {
        if !statements.is_empty() {
            blocks.heading(4, title, None);
            blocks.list(statements.iter().map(statement).collect());
        }
    }
    let sampled: Vec<_> = evolution
        .snapshots
        .iter()
        .filter_map(|s| s.architecture.as_ref().map(|a| (s, a)))
        .collect();
    if !sampled.is_empty() {
        blocks.heading(3, "Architecture over time", None);
        let mut table = Table::new(&[
            "Snapshot",
            "Date",
            "Modules#",
            "Module dependencies#",
            "Largest modules",
        ]);
        for (snapshot, architecture) in sampled {
            let mut modules: Vec<_> = architecture.modules.iter().collect();
            modules.sort_by(|a, b| b.files.cmp(&a.files).then_with(|| a.path.cmp(&b.path)));
            let largest: Vec<String> = modules
                .iter()
                .take(3)
                .map(|m| {
                    if m.path.is_empty() {
                        "(root)".to_owned()
                    } else {
                        m.path.clone()
                    }
                })
                .collect();
            table.row(vec![
                plain(snapshot.label.clone()),
                plain(snapshot.date.date_string()),
                plain(architecture.modules.len().to_string()),
                plain(architecture.edges.len().to_string()),
                plain(largest.join(", ")),
            ]);
        }
        blocks.table(table);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::render;
    use repodna_core::model::SectionStatus;
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::metadata::AnalysisMetadata;
    use repodna_core::time::Timestamp;

    fn bucket(period: &str, commits: u64) -> TimelineBucket {
        TimelineBucket {
            period: period.into(),
            start: Timestamp::UNIX_EPOCH,
            commits,
            authors: 1,
            insertions: 1,
            deletions: 0,
        }
    }

    #[test]
    fn aggregates_long_timelines() {
        let months: Vec<TimelineBucket> = (0..60)
            .map(|i| bucket(&format!("{}-{:02}", 2020 + i / 12, i % 12 + 1), 1))
            .collect();
        let (points, period) = aggregate(&months);
        assert_eq!(period, "quarter");
        assert_eq!(points[0], ("2020 Q1".to_owned(), 3.0));
        assert_eq!(points.len(), 20);
        let (points, period) = aggregate(&months[..12]);
        assert_eq!(period, "month");
        assert_eq!(points.len(), 12);
    }

    #[test]
    fn keeps_contributor_statistics_neutral() {
        let mut dna =
            RepositoryDna::new(RepositoryIdentity::default(), AnalysisMetadata::default());
        dna.git.status = SectionStatus::Analyzed;
        dna.git.ownership.contributors = 2;
        let markdown = render(&super::super::section(
            &dna,
            Section::Contributors,
            ContentOptions::default(),
        ));
        assert!(markdown.contains("not a measure of anyone's work or value"));
        let history = render(&super::super::section(
            &dna,
            Section::History,
            ContentOptions::default(),
        ));
        assert!(history.contains("| Commits | 0 |"));
    }
}
