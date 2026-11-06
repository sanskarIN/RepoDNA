//! Offline coverage audit for explicitly categorized repository evidence questions.
//!
//! Usage: audit-repodna-tasks TRAIN.json VALIDATION.json TEST.json TASKS.json
//! The task manifest must be curated by reviewers; this tool never infers
//! categories or manufactures relevance labels from question text.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::path::Path;

use repodna_ai::ranking::{LabeledEvidence, MAX_EXAMPLES, validate_repository_split};
use serde::{Deserialize, Serialize};

#[path = "support/bounded_input.rs"]
mod bounded_input;

const MAX_DATASET_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TASKS_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TaskKind {
    Architecture,
    Dependencies,
    BuildCommands,
    Security,
    Onboarding,
    History,
    Hotspots,
    Other,
}

impl TaskKind {
    fn key(self) -> &'static str {
        match self {
            Self::Architecture => "architecture",
            Self::Dependencies => "dependencies",
            Self::BuildCommands => "build_commands",
            Self::Security => "security",
            Self::Onboarding => "onboarding",
            Self::History => "history",
            Self::Hotspots => "hotspots",
            Self::Other => "other",
        }
    }

    fn all() -> [Self; 8] {
        [
            Self::Architecture,
            Self::Dependencies,
            Self::BuildCommands,
            Self::Security,
            Self::Onboarding,
            Self::History,
            Self::Hotspots,
            Self::Other,
        ]
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskAnnotation {
    repository: String,
    query: String,
    task: TaskKind,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskCounts {
    questions: usize,
    answerable_questions: usize,
    unanswerable_questions: usize,
    labeled_evidence: usize,
    positive_labels: usize,
    negative_labels: usize,
}

type QuestionKey = (String, String);
type TaskManifest = BTreeMap<QuestionKey, TaskKind>;

fn read_limited(path: &Path, max: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    bounded_input::read_bounded_file(path, max, "benchmark input exceeds configured byte limit")
}

fn load_labels(path: &Path) -> Result<Vec<LabeledEvidence>, Box<dyn std::error::Error>> {
    let samples: Vec<LabeledEvidence> =
        serde_json::from_slice(&read_limited(path, MAX_DATASET_BYTES)?)?;
    if samples.is_empty() || samples.len() > MAX_EXAMPLES {
        return Err("dataset must contain 1..=100000 labeled records".into());
    }
    if samples.iter().any(|s| {
        s.repository.trim().is_empty()
            || s.repository.len() > 200
            || s.query.trim().is_empty()
            || s.query.len() > 4_000
            || s.evidence.detail.trim().is_empty()
            || s.evidence.detail.len() > 8_000
    }) {
        return Err("dataset has missing or oversized fields".into());
    }
    Ok(samples)
}

fn question_key(repository: &str, query: &str) -> QuestionKey {
    (repository.trim().to_lowercase(), query.to_owned())
}

fn load_manifest(path: &Path) -> Result<TaskManifest, Box<dyn std::error::Error>> {
    let entries: Vec<TaskAnnotation> =
        serde_json::from_slice(&read_limited(path, MAX_TASKS_BYTES)?)?;
    if entries.is_empty() || entries.len() > MAX_EXAMPLES {
        return Err("task manifest must contain 1..=100000 question entries".into());
    }
    let mut tasks = TaskManifest::new();
    for entry in entries {
        if entry.repository.trim().is_empty()
            || entry.repository.len() > 200
            || entry.query.trim().is_empty()
            || entry.query.len() > 4_000
        {
            return Err("task manifest has invalid or oversized fields".into());
        }
        if tasks
            .insert(question_key(&entry.repository, &entry.query), entry.task)
            .is_some()
        {
            return Err("task manifest repeats a normalized repository/question".into());
        }
    }
    Ok(tasks)
}

fn grouped_queries(samples: &[LabeledEvidence]) -> BTreeMap<QuestionKey, (usize, usize)> {
    let mut groups = BTreeMap::new();
    for sample in samples {
        let counts = groups
            .entry(question_key(&sample.repository, &sample.query))
            .or_insert((0_usize, 0_usize));
        if sample.relevant {
            counts.0 += 1;
        } else {
            counts.1 += 1;
        }
    }
    groups
}

fn summarize(
    groups: &BTreeMap<QuestionKey, (usize, usize)>,
    tasks: &TaskManifest,
) -> Result<BTreeMap<&'static str, TaskCounts>, Box<dyn std::error::Error>> {
    let mut by_kind: BTreeMap<&'static str, TaskCounts> = TaskKind::all()
        .into_iter()
        .map(|kind| (kind.key(), TaskCounts::default()))
        .collect();
    for (key, &(positive, negative)) in groups {
        let kind = tasks
            .get(key)
            .ok_or("task manifest does not cover all questions")?;
        let counts = by_kind.get_mut(kind.key()).ok_or("unknown task category")?;
        counts.questions += 1;
        counts.labeled_evidence += positive + negative;
        counts.positive_labels += positive;
        counts.negative_labels += negative;
        if positive > 0 {
            counts.answerable_questions += 1;
        } else {
            counts.unanswerable_questions += 1;
        }
    }
    Ok(by_kind)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(train), Some(validation), Some(test), Some(tasks), None) = (
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
    ) else {
        return Err(
            "usage: audit-repodna-tasks TRAIN.json VALIDATION.json TEST.json TASKS.json".into(),
        );
    };
    let train = load_labels(Path::new(&train))?;
    let validation = load_labels(Path::new(&validation))?;
    let test = load_labels(Path::new(&test))?;
    validate_repository_split(&train, &validation).map_err(io::Error::other)?;
    validate_repository_split(&train, &test).map_err(io::Error::other)?;
    validate_repository_split(&validation, &test).map_err(io::Error::other)?;

    let manifest = load_manifest(Path::new(&tasks))?;
    let train_groups = grouped_queries(&train);
    let validation_groups = grouped_queries(&validation);
    let test_groups = grouped_queries(&test);
    let observed: BTreeSet<_> = train_groups
        .keys()
        .chain(validation_groups.keys())
        .chain(test_groups.keys())
        .cloned()
        .collect();
    let expected: BTreeSet<_> = manifest.keys().cloned().collect();
    if observed != expected {
        return Err("task manifest contains missing or unused repository/questions".into());
    }

    let training = summarize(&train_groups, &manifest)?;
    let validation = summarize(&validation_groups, &manifest)?;
    let final_test = summarize(&test_groups, &manifest)?;
    let mut warnings = Vec::new();
    for (name, counts) in [
        ("training", &training),
        ("validation", &validation),
        ("finalTest", &final_test),
    ] {
        for required in [
            TaskKind::Architecture,
            TaskKind::Dependencies,
            TaskKind::BuildCommands,
            TaskKind::Security,
            TaskKind::Onboarding,
            TaskKind::History,
        ] {
            if counts
                .get(required.key())
                .is_some_and(|entry| entry.questions == 0)
            {
                warnings.push(format!(
                    "{name}: no reviewer-tagged {} questions",
                    required.key()
                ));
            }
        }
    }
    let report = serde_json::json!({
        "schema": "repodna-task-coverage/1",
        "taskManifestChecked": true,
        "manifestQuestions": manifest.len(),
        "training": training,
        "validation": validation,
        "finalTest": final_test,
        "warnings": warnings,
        "note": "Task categories were supplied by reviewers, not inferred. Structural validity and category coverage cannot establish independent repository families, permission to use data, human label correctness, factual accuracy or generalization. No repository IDs or question text are printed."
    });
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &report)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(io::stderr().lock(), "task coverage audit failed: {error}");
        std::process::exit(1);
    }
}
