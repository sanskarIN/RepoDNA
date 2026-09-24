//! The evolution analysis entry point and its findings.

use repodna_core::confidence::Confidence;
use repodna_core::evidence::Evidence;
use repodna_core::finding::{Finding, FindingCategory};
use repodna_core::model::SectionStatus;
use repodna_core::model::evolution::{AgeClass, EvolutionReport, Snapshot};
use repodna_core::model::git::{DormantPeriod, FileHistoryRecord, ReleaseInfo};
use repodna_core::severity::Severity;
use repodna_core::time::Timestamp;
use repodna_git::History;

use crate::ages::{abandoned_areas, module_ages};
use crate::epochs::detect_epochs;
use crate::events::detect_events;
use crate::snapshots::{SAMPLING_METHOD, growth};
use crate::story::{StoryInput, archaeology, story};

/// Dormant periods at least this long are reported as findings.
const DORMANT_FINDING_DAYS: i64 = 180;

/// Maximum findings per rule.
const MAX_PER_RULE: usize = 5;

/// Inputs to evolution analysis.
#[derive(Debug, Clone)]
pub struct EvolutionInput<'a> {
    /// Commit history (newest first).
    pub history: &'a History,
    /// Releases.
    pub releases: &'a [ReleaseInfo],
    /// Periods without commits.
    pub dormant_periods: &'a [DormantPeriod],
    /// History of current files.
    pub file_history: &'a [FileHistoryRecord],
    /// Snapshots measured by the caller, oldest first.
    pub snapshots: Vec<Snapshot>,
    /// Modules and their files, for module ages.
    pub modules: &'a [(String, Vec<String>)],
}

/// Results of evolution analysis.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EvolutionOutput {
    /// The evolution section.
    pub report: EvolutionReport,
    /// Evolution findings.
    pub findings: Vec<Finding>,
}

/// Runs evolution analysis.
pub fn analyze(input: EvolutionInput<'_>) -> EvolutionOutput {
    let history = input.history;
    let (Some(newest), Some(oldest)) = (history.commits.first(), history.commits.last()) else {
        return EvolutionOutput {
            report: EvolutionReport {
                status: SectionStatus::Unavailable,
                notes: vec!["No commit history is available.".to_owned()],
                ..EvolutionReport::default()
            },
            findings: Vec::new(),
        };
    };
    let latest = Timestamp::from_unix(newest.timestamp);
    let start = Timestamp::from_unix(oldest.timestamp);
    let mut notes = Vec::new();
    if history.truncated {
        notes.push(
            "History was read up to the configured commit limit, so the earliest events may be missing."
                .to_owned(),
        );
    }
    let epochs = detect_epochs(history);
    let events = detect_events(history, &input.snapshots, input.releases);
    let abandoned = abandoned_areas(input.file_history, latest);
    let ages = module_ages(input.modules, input.file_history, start, latest);
    let archaeology = archaeology(&StoryInput {
        history,
        epochs: &epochs,
        events: &events,
        snapshots: &input.snapshots,
        dormant_periods: input.dormant_periods,
        releases: input.releases,
        abandoned: &abandoned,
    });
    let story = story(&archaeology);

    let mut findings = Vec::new();
    for area in abandoned.iter().take(MAX_PER_RULE) {
        findings.push(
            Finding::new(
                "evolution.abandoned-area",
                &area.path,
                FindingCategory::Evolution,
                Severity::Info,
                Confidence::Medium,
                format!("{}/ has not changed in {} days", area.path, area.days_before_latest),
            )
            .summary(format!(
                "{} files in {}/ last changed on {}, while the rest of the repository kept changing.",
                area.files,
                area.path,
                area.last_changed.date_string()
            ))
            .rationale("Code that nobody touches for a long time may be finished and stable, or forgotten; either way it is worth knowing which.")
            .method("The latest change to any current file in the area, compared with the latest commit.")
            .evidence(Evidence::directory(area.path.as_str()).with_note(area.description.clone()))
            .limitation("Stable, finished code also stops changing; this is not a quality judgment.")
            .next_step("Confirm whether the area is still used and who is responsible for it.")
            .path(area.path.clone()),
        );
    }
    let mut dormant: Vec<&DormantPeriod> = input
        .dormant_periods
        .iter()
        .filter(|period| period.days >= DORMANT_FINDING_DAYS)
        .collect();
    dormant.sort_by_key(|period| std::cmp::Reverse(period.days));
    if let Some(period) = dormant.first() {
        findings.push(
            Finding::new(
                "evolution.dormant-period",
                &period.before_commit,
                FindingCategory::Evolution,
                Severity::Info,
                Confidence::High,
                format!("Development paused for {} days", period.days),
            )
            .summary(format!(
                "No commits between {} and {}.",
                period.start.date_string(),
                period.end.date_string()
            ))
            .rationale("Long pauses often mark a change of maintainers, priorities, or funding, and context may have been lost in between.")
            .method("The longest gap between consecutive commits on the analyzed branch.")
            .evidence(Evidence::commit(period.before_commit.clone(), Some(period.start)))
            .evidence(Evidence::commit(period.after_commit.clone(), Some(period.end)))
            .limitation("Work on other branches or in other repositories is not visible."),
        );
    }
    for age in ages
        .iter()
        .filter(|age| age.class == AgeClass::New)
        .take(MAX_PER_RULE)
    {
        findings.push(
            Finding::new(
                "evolution.new-module",
                &age.module,
                FindingCategory::Evolution,
                Severity::Info,
                Confidence::High,
                format!("{} is a new module", age.module),
            )
            .summary(age.reason.clone())
            .rationale("New modules are where the current direction of a project shows; they are often still changing quickly.")
            .method("The first commit that touched any of the module's current files.")
            .evidence(Evidence::directory(age.module.as_str()))
            .path(age.module.clone()),
        );
    }

    let growth = growth(&input.snapshots);
    EvolutionOutput {
        report: EvolutionReport {
            status: SectionStatus::Analyzed,
            notes,
            epochs,
            events,
            snapshots: input.snapshots,
            growth,
            abandoned_areas: abandoned,
            module_ages: ages,
            story,
            archaeology,
            sampling: SAMPLING_METHOD.to_owned(),
        },
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{commit, history};

    #[test]
    fn assembles_the_report_and_findings() {
        let history = history(vec![
            commit(
                "c1",
                "2020-01-01",
                "ana",
                &["+old/a.py", "+old/b.py", "+old/c.py", "+src/x.py"],
            ),
            commit("c2", "2021-06-01", "ana", &["~src/x.py"]),
            commit("c3", "2024-05-25", "bo", &["+fresh/a.py"]),
        ]);
        let record =
            |path: &str, first: (i64, u32, u32), last: (i64, u32, u32)| FileHistoryRecord {
                path: path.to_owned(),
                commits: 1,
                authors: 1,
                insertions: 1,
                deletions: 0,
                first_seen: Timestamp::from_ymd(first.0, first.1, first.2).unwrap(),
                last_changed: Timestamp::from_ymd(last.0, last.1, last.2).unwrap(),
                recent_commits: 0,
                previous_paths: Vec::new(),
            };
        let file_history = [
            record("old/a.py", (2020, 1, 1), (2020, 1, 1)),
            record("old/b.py", (2020, 1, 1), (2020, 1, 1)),
            record("old/c.py", (2020, 1, 1), (2020, 1, 1)),
            record("src/x.py", (2020, 1, 1), (2021, 6, 1)),
            record("fresh/a.py", (2024, 5, 25), (2024, 5, 25)),
        ];
        let dormant = [DormantPeriod {
            start: Timestamp::from_ymd(2021, 6, 1).unwrap(),
            end: Timestamp::from_ymd(2024, 5, 25).unwrap(),
            days: 1089,
            before_commit: "c2".into(),
            after_commit: "c3".into(),
        }];
        let modules = vec![("fresh".to_owned(), vec!["fresh/a.py".to_owned()])];
        let output = analyze(EvolutionInput {
            history: &history,
            releases: &[],
            dormant_periods: &dormant,
            file_history: &file_history,
            snapshots: Vec::new(),
            modules: &modules,
        });
        assert_eq!(output.report.status, SectionStatus::Analyzed);
        assert!(!output.report.epochs.is_empty());
        assert!(!output.report.story.is_empty());
        let rules: Vec<&str> = output.findings.iter().map(|f| f.rule.as_str()).collect();
        assert_eq!(
            rules,
            vec![
                "evolution.abandoned-area",
                "evolution.dormant-period",
                "evolution.new-module"
            ]
        );
        let empty = analyze(EvolutionInput {
            history: &History {
                commits: Vec::new(),
                truncated: false,
            },
            releases: &[],
            dormant_periods: &[],
            file_history: &[],
            snapshots: Vec::new(),
            modules: &[],
        });
        assert_eq!(empty.report.status, SectionStatus::Unavailable);
    }
}
