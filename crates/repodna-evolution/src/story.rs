//! The Project Story: a chronological narrative in which every sentence is a fact linked to
//! commits and snapshots, or an interpretation labeled as such.

use std::collections::HashSet;

use repodna_core::evidence::Evidence;
use repodna_core::model::evolution::{
    AbandonedArea, ArchaeologyReport, Epoch, EvolutionEvent, EvolutionEventKind, Snapshot,
    StatementKind, StoryStatement,
};
use repodna_core::model::git::{DormantPeriod, ReleaseInfo};
use repodna_core::text::count;
use repodna_core::time::Timestamp;
use repodna_git::{History, contributor_id};

use crate::names::{LanguageNames, moment};

/// Statements per archaeology section.
const MAX_PER_SECTION: usize = 8;

/// What the story is built from.
#[derive(Debug, Clone, Copy)]
pub struct StoryInput<'a> {
    /// Commit history (newest first).
    pub history: &'a History,
    /// Epochs, oldest first.
    pub epochs: &'a [Epoch],
    /// Events, oldest first.
    pub events: &'a [EvolutionEvent],
    /// Snapshots, oldest first.
    pub snapshots: &'a [Snapshot],
    /// Periods without commits.
    pub dormant_periods: &'a [DormantPeriod],
    /// Releases.
    pub releases: &'a [ReleaseInfo],
    /// Areas that stopped changing.
    pub abandoned: &'a [AbandonedArea],
    /// Display names of the languages in the snapshots.
    pub language_names: &'a LanguageNames,
}

fn fact(text: String, date: Option<Timestamp>, evidence: Vec<Evidence>) -> StoryStatement {
    StoryStatement {
        text,
        kind: StatementKind::Fact,
        date,
        evidence,
    }
}

fn interpretation(
    text: String,
    date: Option<Timestamp>,
    evidence: Vec<Evidence>,
) -> StoryStatement {
    StoryStatement {
        text,
        kind: StatementKind::Interpretation,
        date,
        evidence,
    }
}

fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

fn main_languages(snapshot: &Snapshot, names: &LanguageNames) -> Vec<String> {
    snapshot
        .languages
        .iter()
        .filter(|language| language.share >= 0.1)
        .take(3)
        .map(|language| names.name(&language.id).to_owned())
        .collect()
}

fn snapshot_evidence(snapshot: &Snapshot) -> Evidence {
    Evidence::commit(snapshot.revision.clone(), Some(snapshot.date))
}

fn event_statement(event: &EvolutionEvent) -> StoryStatement {
    let text = match event.kind {
        EvolutionEventKind::ModuleIntroduced => format!(
            "{} ({}).",
            event.title.trim_end_matches('.'),
            event.date.date_string()
        ),
        _ => format!(
            "{} on {}. {}",
            event.title,
            event.date.date_string(),
            event.description
        ),
    };
    fact(text, Some(event.date), event.evidence.clone())
}

/// Builds the software-archaeology view.
pub fn archaeology(input: &StoryInput<'_>) -> ArchaeologyReport {
    let mut report = ArchaeologyReport::default();
    let events_of = |kinds: &[EvolutionEventKind]| -> Vec<&EvolutionEvent> {
        input
            .events
            .iter()
            .filter(|event| kinds.contains(&event.kind))
            .collect()
    };

    // Origin.
    if let Some(created) = events_of(&[EvolutionEventKind::RepositoryCreated]).first() {
        report.origin.push(fact(
            format!(
                "{} on {}. {}",
                created.title,
                created.date.date_string(),
                created.description
            ),
            Some(created.date),
            created.evidence.clone(),
        ));
    }
    if let Some(first) = input.snapshots.first() {
        let languages = main_languages(first, input.language_names);
        if !languages.is_empty() {
            report.origin.push(fact(
                format!(
                    "At that point the code was mostly {} (by size).",
                    list(&languages)
                ),
                Some(first.date),
                vec![snapshot_evidence(first)],
            ));
        }
    }

    // Growth.
    let commits = input.history.commits.len();
    if let (Some(first), Some(last)) = (input.history.commits.last(), input.history.commits.first())
    {
        let authors: HashSet<String> = input
            .history
            .commits
            .iter()
            .map(|commit| contributor_id(&commit.author_email, &commit.author_name))
            .collect();
        let start = Timestamp::from_unix(first.timestamp);
        let end = Timestamp::from_unix(last.timestamp);
        report.growth.push(fact(
            format!(
                "{} by {} between {} and {}.",
                count(commits as u64, "commit", "commits"),
                count(authors.len() as u64, "contributor", "contributors"),
                start.date_string(),
                end.date_string()
            ),
            Some(end),
            vec![
                Evidence::commit(first.hash.clone(), Some(start)),
                Evidence::commit(last.hash.clone(), Some(end)),
            ],
        ));
    }
    if let (Some(first), Some(last)) = (input.snapshots.first(), input.snapshots.last())
        && input.snapshots.len() >= 2
        && first.files != last.files
    {
        report.growth.push(fact(
            format!(
                "The repository went from {} ({}) to {} ({}).",
                count(first.files, "file", "files"),
                first.date.date_string(),
                count(last.files, "file", "files"),
                last.date.date_string()
            ),
            Some(last.date),
            vec![snapshot_evidence(first), snapshot_evidence(last)],
        ));
        if let Some(pair) = input
            .snapshots
            .windows(2)
            .filter(|pair| pair[1].files > pair[0].files)
            .max_by_key(|pair| pair[1].files - pair[0].files)
        {
            report.growth.push(interpretation(
                format!(
                    "Growth was fastest between {} and {}, when {} added.",
                    moment(&pair[0]),
                    moment(&pair[1]),
                    match pair[1].files - pair[0].files {
                        1 => "1 file was".to_owned(),
                        added => format!("{added} files were"),
                    }
                ),
                Some(pair[1].date),
                vec![snapshot_evidence(&pair[0]), snapshot_evidence(&pair[1])],
            ));
        }
    }

    // Expansions and rewrites.
    use EvolutionEventKind::*;
    report.expansions = events_of(&[
        ModuleIntroduced,
        MultiPackageStructure,
        TestsIntroduced,
        CiAdopted,
        ContainersAdopted,
        LanguageIntroduced,
        SignificantGrowth,
    ])
    .into_iter()
    .take(MAX_PER_SECTION)
    .map(event_statement)
    .collect();
    report.rewrites = events_of(&[
        Restructuring,
        LanguageShift,
        ModuleRemoved,
        SignificantReduction,
    ])
    .into_iter()
    .take(MAX_PER_SECTION)
    .map(event_statement)
    .collect();

    // Inactivity.
    let mut dormant: Vec<&DormantPeriod> = input.dormant_periods.iter().collect();
    dormant.sort_by(|a, b| b.days.cmp(&a.days).then_with(|| a.start.cmp(&b.start)));
    let mut inactivity: Vec<StoryStatement> = dormant
        .into_iter()
        .take(5)
        .map(|period| {
            fact(
                format!(
                    "No commits for {} days, from {} to {}.",
                    period.days,
                    period.start.date_string(),
                    period.end.date_string()
                ),
                Some(period.start),
                vec![
                    Evidence::commit(period.before_commit.clone(), Some(period.start)),
                    Evidence::commit(period.after_commit.clone(), Some(period.end)),
                ],
            )
        })
        .collect();
    inactivity.sort_by_key(|statement| statement.date);
    inactivity.extend(input.abandoned.iter().take(3).map(|area| {
        fact(
            format!(
                "{}/ has not changed since {}.",
                area.path,
                area.last_changed.date_string()
            ),
            Some(area.last_changed),
            vec![Evidence::directory(area.path.as_str())],
        )
    }));
    report.inactivity = inactivity;

    // The final phase and the current state.
    if let Some(epoch) = input.epochs.last() {
        let focus: Vec<String> = epoch
            .focus_areas
            .iter()
            .map(|area| format!("{area}/"))
            .collect();
        let mut text = format!(
            "Since {}, {} by {}",
            epoch.start.date_string(),
            count(epoch.commits, "commit", "commits"),
            count(u64::from(epoch.contributors), "contributor", "contributors")
        );
        if focus.is_empty() {
            text.push('.');
        } else {
            text.push_str(&format!(" changed mostly {}.", list(&focus)));
        }
        report
            .final_phase
            .push(fact(text, Some(epoch.start), Vec::new()));
        if let Some(area) = focus.first() {
            report.final_phase.push(interpretation(
                format!("Recent work concentrates on {area}."),
                Some(epoch.end),
                Vec::new(),
            ));
        }
    }
    if let Some(current) = input.snapshots.last() {
        let languages = main_languages(current, input.language_names);
        let mut text = format!(
            "The current revision has {}",
            count(current.files, "file", "files")
        );
        match languages.as_slice() {
            [] => {}
            [only] => text.push_str(&format!("; the main language by size is {only}")),
            _ => text.push_str(&format!(
                "; the main languages by size are {}",
                list(&languages)
            )),
        }
        text.push('.');
        report.current_state.push(fact(
            text,
            Some(current.date),
            vec![snapshot_evidence(current)],
        ));
    }
    if let Some(release) = input.releases.iter().max_by_key(|release| release.date) {
        report.current_state.push(fact(
            format!(
                "The latest release is {}, from {}.",
                release.tag,
                release.date.date_string()
            ),
            Some(release.date),
            vec![Evidence::commit(release.commit.clone(), Some(release.date))],
        ));
    }
    report
}

/// The chronological story: origin, dated milestones, growth, the final phase, and now.
pub fn story(archaeology: &ArchaeologyReport) -> Vec<StoryStatement> {
    let mut milestones: Vec<StoryStatement> = archaeology
        .expansions
        .iter()
        .chain(&archaeology.rewrites)
        .chain(&archaeology.inactivity)
        .cloned()
        .collect();
    milestones.sort_by_key(|statement| statement.date);
    archaeology
        .origin
        .iter()
        .cloned()
        .chain(milestones)
        .chain(archaeology.growth.iter().cloned())
        .chain(archaeology.final_phase.iter().cloned())
        .chain(archaeology.current_state.iter().cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::epochs::detect_epochs;
    use crate::events::detect_events;
    use crate::testutil::{commit, history};
    use repodna_core::model::evolution::{SnapshotKind, SnapshotLanguage};

    fn snapshot(label: &str, date: (i64, u32, u32), files: u64) -> Snapshot {
        Snapshot {
            revision: format!("rev-{label}"),
            label: label.to_owned(),
            kind: match label {
                "Initial commit" => SnapshotKind::Initial,
                "Current" => SnapshotKind::Current,
                _ => SnapshotKind::Sample,
            },
            date: Timestamp::from_ymd(date.0, date.1, date.2).unwrap(),
            commit_index: files,
            files,
            bytes: files * 100,
            test_files: 0,
            doc_files: 0,
            languages: vec![SnapshotLanguage {
                id: "rust".into(),
                files,
                bytes: files * 100,
                share: 1.0,
            }],
            directories: Vec::new(),
            dependencies: None,
            architecture: None,
        }
    }

    #[test]
    fn tells_a_story_of_facts_and_labeled_interpretations() {
        let history = history(vec![
            commit("c1", "2020-01-01", "ana", &["+src/lib.rs", "+Cargo.toml"]),
            commit(
                "c2",
                "2020-02-01",
                "bo",
                &["+cli/a.rs", "+cli/b.rs", "+cli/c.rs"],
            ),
            commit("c3", "2021-03-01", "ana", &["~cli/a.rs"]),
        ]);
        let epochs = detect_epochs(&history);
        let events = detect_events(&history, &[], &[], &LanguageNames::default());
        let snapshots = [
            snapshot("Initial commit", (2020, 1, 1), 2),
            snapshot("2020-02", (2020, 2, 1), 5),
            snapshot("Current", (2021, 3, 1), 5),
        ];
        let dormant = [DormantPeriod {
            start: Timestamp::from_ymd(2020, 2, 1).unwrap(),
            end: Timestamp::from_ymd(2021, 3, 1).unwrap(),
            days: 394,
            before_commit: "c2".into(),
            after_commit: "c3".into(),
        }];
        let releases = [ReleaseInfo {
            tag: "v0.1.0".into(),
            version: "0.1.0".into(),
            date: Timestamp::from_ymd(2020, 2, 2).unwrap(),
            commit: "c2".into(),
            commits_since_previous: 2,
        }];
        let input = StoryInput {
            history: &history,
            epochs: &epochs,
            events: &events,
            snapshots: &snapshots,
            dormant_periods: &dormant,
            releases: &releases,
            abandoned: &[],
            language_names: &LanguageNames::new([("rust".to_owned(), "Rust".to_owned())]),
        };
        let report = archaeology(&input);
        assert_eq!(
            report.origin[0].text,
            "Repository created on 2020-01-01. The first commit added 2 files."
        );
        assert_eq!(
            report.origin[1].text,
            "At that point the code was mostly Rust (by size)."
        );
        assert!(
            report
                .growth
                .iter()
                .any(|s| s.kind == StatementKind::Interpretation
                    && s.text.contains("between the first commit and 2020-02"))
        );
        assert_eq!(report.expansions[0].text, "cli/ appeared (2020-02-01).");
        assert_eq!(
            report.inactivity[0].text,
            "No commits for 394 days, from 2020-02-01 to 2021-03-01."
        );
        assert!(report.final_phase[0].text.contains("changed mostly cli/"));
        assert_eq!(
            report.current_state[1].text,
            "The latest release is v0.1.0, from 2020-02-02."
        );

        let story = story(&report);
        assert_eq!(story[0].text, report.origin[0].text);
        assert_eq!(story.last().unwrap().text, report.current_state[1].text);
        assert!(story.iter().all(|s| s.kind == StatementKind::Fact
            || s.text.starts_with("Growth")
            || s.text.starts_with("Recent")));
        assert_eq!(list(&["a".into(), "b".into(), "c".into()]), "a, b, and c");
    }
}
