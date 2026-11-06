//! Offline, aggregate-only agreement audit for two independently reviewed
//! RepoDNA evidence annotation files.
//!
//! Usage: audit-repodna-reviewers REVIEW_A.json REVIEW_B.json [ADJUDICATED.json] [--tasks TASKS.json]
//! Reviewers must judge the exact same evidence records. This audit measures
//! consistency, not independence, factual correctness or reviewer expertise.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use std::io::{self, Write};
use std::path::Path;

use repodna_ai::ranking::{AnnotationExample, MAX_EXAMPLES, finish_annotations};

#[path = "support/bounded_input.rs"]
mod bounded_input;

const MAX_DATASET_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TASK_MANIFEST_BYTES: u64 = 2 * 1024 * 1024;

fn load(path: &Path) -> Result<Vec<AnnotationExample>, Box<dyn std::error::Error>> {
    let bytes =
        bounded_input::read_bounded_file(path, MAX_DATASET_BYTES, "review file exceeds 8 MiB")?;
    let examples: Vec<AnnotationExample> = serde_json::from_slice(&bytes)?;
    if examples.is_empty() || examples.len() > MAX_EXAMPLES {
        return Err("review file must contain 1..=100000 records".into());
    }
    if examples.iter().any(|example| {
        example.repository.trim().is_empty()
            || example.repository.len() > 200
            || example.query.trim().is_empty()
            || example.query.len() > 4_000
            || example.evidence.detail.trim().is_empty()
            || example.evidence.detail.len() > 8_000
    }) {
        return Err("review file has missing or oversized fields".into());
    }
    // Validation also rejects null judgments, duplicate evidence IDs,
    // missing evidence fields and raw source excerpts.
    finish_annotations(examples.clone()).map_err(io::Error::other)?;
    Ok(examples)
}

/// Exact metadata identity (including evidence ID), excluding the label.
/// The entire record is compared so no reviewer can silently change a fact.
fn records(
    reviews: &[AnnotationExample],
) -> Result<BTreeMap<Vec<u8>, bool>, Box<dyn std::error::Error>> {
    let mut result = BTreeMap::new();
    for review in reviews {
        let identity = serde_json::to_vec(&(&review.repository, &review.query, &review.evidence))?;
        let relevant = review.relevant.ok_or("unreviewed annotation")?;
        if result.insert(identity, relevant).is_some() {
            return Err("duplicate annotation record".into());
        }
    }
    Ok(result)
}

fn agreement(
    a: &[AnnotationExample],
    b: &[AnnotationExample],
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let first = records(a)?;
    let second = records(b)?;
    if first.len() != second.len() || !first.keys().eq(second.keys()) {
        return Err("review files contain different evidence records".into());
    }
    let mut both_relevant = 0_usize;
    let mut both_irrelevant = 0_usize;
    let mut only_a_relevant = 0_usize;
    let mut only_b_relevant = 0_usize;
    for (identity, first_label) in &first {
        let second_label = second
            .get(identity)
            .ok_or("review files contain different evidence records")?;
        match (*first_label, *second_label) {
            (true, true) => both_relevant += 1,
            (false, false) => both_irrelevant += 1,
            (true, false) => only_a_relevant += 1,
            (false, true) => only_b_relevant += 1,
        }
    }
    let count = first.len();
    let a_positive = both_relevant + only_a_relevant;
    let b_positive = both_relevant + only_b_relevant;
    let observed = (both_relevant + both_irrelevant) as f64 / count as f64;
    let a_rate = a_positive as f64 / count as f64;
    let b_rate = b_positive as f64 / count as f64;
    let chance = a_rate * b_rate + (1.0 - a_rate) * (1.0 - b_rate);
    // When both reviewers always choose the same one class, chance agreement
    // is one and kappa is undefined, even though observed agreement is 100%.
    let kappa = if chance >= 1.0 {
        None
    } else {
        Some((observed - chance) / (1.0 - chance))
    };

    Ok(serde_json::json!({
        "schema": "repodna-reviewer-agreement/1",
        "pairedAnnotations": count,
        "agreements": both_relevant + both_irrelevant,
        "disagreements": only_a_relevant + only_b_relevant,
        "bothRelevant": both_relevant,
        "bothIrrelevant": both_irrelevant,
        "onlyReviewerARelevant": only_a_relevant,
        "onlyReviewerBRelevant": only_b_relevant,
        "reviewerAPositiveRate": a_rate,
        "reviewerBPositiveRate": b_rate,
        "observedAgreement": observed,
        "expectedChanceAgreement": chance,
        "cohensKappa": kappa,
        "note": "Agreement is descriptive, not proof of annotation correctness, independent review, repository rights, or model accuracy. Undefined kappa is reported as null. No evidence text or reviewer identities are printed."
    }))
}

/// Check a third, explicitly finalized set of labels against both completed
/// independent reviews. This is a structural gate, *not* proof a human
/// adjudicator actually inspected each disputed case.
fn adjudication(
    a: &[AnnotationExample],
    b: &[AnnotationExample],
    final_review: &[AnnotationExample],
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let first = records(a)?;
    let second = records(b)?;
    let final_labels = records(final_review)?;
    if first.len() != second.len()
        || first.len() != final_labels.len()
        || !first.keys().eq(second.keys())
        || !first.keys().eq(final_labels.keys())
    {
        return Err("adjudication files contain different evidence records".into());
    }

    let mut agreed_labels_preserved = 0_usize;
    let mut disagreements_adjudicated = 0_usize;
    let mut adjudicated_for_a = 0_usize;
    let mut adjudicated_for_b = 0_usize;
    let mut final_positive_labels = 0_usize;
    for (identity, first_label) in &first {
        let second_label = second
            .get(identity)
            .ok_or("adjudication files contain different evidence records")?;
        let final_label = final_labels
            .get(identity)
            .ok_or("adjudication files contain different evidence records")?;
        if *final_label {
            final_positive_labels += 1;
        }
        if first_label == second_label {
            // An override of two agreeing reviewers needs a separate documented
            // correction protocol, not a silent change to the finalized labels.
            if first_label != final_label {
                return Err(
                    "finalized label changes a judgment on which both reviewers agreed".into(),
                );
            }
            agreed_labels_preserved += 1;
        } else {
            disagreements_adjudicated += 1;
            if first_label == final_label {
                adjudicated_for_a += 1;
            } else {
                adjudicated_for_b += 1;
            }
        }
    }

    Ok(serde_json::json!({
        "schema": "repodna-reviewer-adjudication/1",
        "structuralChecksPassed": true,
        "agreedLabelsPreserved": agreed_labels_preserved,
        "disagreementsAdjudicated": disagreements_adjudicated,
        "adjudicatedInFavorOfReviewerA": adjudicated_for_a,
        "adjudicatedInFavorOfReviewerB": adjudicated_for_b,
        "finalPositiveLabels": final_positive_labels,
        "finalNegativeLabels": final_labels.len() - final_positive_labels,
        "note": "All records are complete, paired and preserve agreed reviewer labels. This cannot prove that contested labels were independently adjudicated, that any label is correct, or that repository metadata may be published. Reviewer identities and evidence content are not emitted."
    }))
}

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
    fn name(self) -> &'static str {
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
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskAssignment {
    repository: String,
    query: String,
    task: TaskKind,
}

/// Aggregate reviewer agreement by independently curated question category.
/// The manifest must cover exactly the questions in the paired review subset.
fn task_agreement(
    a: &[AnnotationExample],
    b: &[AnnotationExample],
    path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let bytes = bounded_input::read_bounded_file(
        path,
        MAX_TASK_MANIFEST_BYTES,
        "task manifest exceeds 2 MiB",
    )?;
    let entries: Vec<TaskAssignment> = serde_json::from_slice(&bytes)?;
    if entries.is_empty() || entries.len() > MAX_EXAMPLES {
        return Err("task manifest must contain 1..=100000 questions".into());
    }
    let mut assignments = BTreeMap::new();
    for entry in entries {
        if entry.repository.trim().is_empty()
            || entry.repository.len() > 200
            || entry.query.trim().is_empty()
            || entry.query.len() > 4_000
        {
            return Err("task manifest has missing or oversized fields".into());
        }
        let key = (entry.repository.trim().to_lowercase(), entry.query);
        if assignments.insert(key, entry.task).is_some() {
            return Err("duplicate normalized task manifest question".into());
        }
    }

    // Pairing is exact for all non-label metadata; do not normalize evidence
    // identity or silently tolerate changed facts between reviewer copies.
    let other = records(b)?;
    let mut used = BTreeSet::new();
    let mut grouped: BTreeMap<TaskKind, (Vec<AnnotationExample>, Vec<AnnotationExample>)> =
        BTreeMap::new();
    for example in a {
        let key = (
            example.repository.trim().to_lowercase(),
            example.query.clone(),
        );
        let task = *assignments
            .get(&key)
            .ok_or("task manifest is missing a reviewed question")?;
        let identity =
            serde_json::to_vec(&(&example.repository, &example.query, &example.evidence))?;
        let other_label = *other
            .get(&identity)
            .ok_or("review files contain different evidence records")?;
        used.insert(key);
        let pair = grouped.entry(task).or_default();
        pair.0.push(example.clone());
        let mut corresponding = example.clone();
        corresponding.relevant = Some(other_label);
        pair.1.push(corresponding);
    }
    if used.len() != assignments.len() {
        return Err("task manifest contains questions absent from reviewed subset".into());
    }
    let mut summaries = serde_json::Map::new();
    for (kind, (first, second)) in grouped {
        let mut stats = agreement(&first, &second)?;
        if let Some(object) = stats.as_object_mut() {
            object.remove("schema");
            object.remove("note");
        }
        summaries.insert(kind.name().to_owned(), stats);
    }
    Ok(serde_json::json!({
        "schema": "repodna-reviewer-task-agreement/1",
        "manifestQuestions": assignments.len(),
        "summaries": summaries,
        "note": "Tasks are human-curated; per-task aggregates measure reviewer consistency, not correctness or model accuracy. Tiny groups can yield unstable or undefined kappa. No questions, repositories, evidence, or identities are printed."
    }))
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let usage = "usage: audit-repodna-reviewers REVIEW_A.json REVIEW_B.json [ADJUDICATED.json] [--tasks TASKS.json]";
    let (final_path, tasks_path): (Option<&Path>, Option<&Path>) = match args.as_slice() {
        [_, _] => (None, None),
        [_, _, final_path] if final_path.to_str() != Some("--tasks") => {
            (Some(Path::new(final_path)), None)
        }
        [_, _, flag, tasks_path] if flag.to_str() == Some("--tasks") => {
            (None, Some(Path::new(tasks_path)))
        }
        [_, _, final_path, flag, tasks_path]
            if flag.to_str() == Some("--tasks") && final_path.to_str() != Some("--tasks") =>
        {
            (Some(Path::new(final_path)), Some(Path::new(tasks_path)))
        }
        _ => return Err(usage.into()),
    };
    let first = load(Path::new(&args[0]))?;
    let second = load(Path::new(&args[1]))?;
    let mut report = agreement(&first, &second)?;
    if let Some(path) = final_path {
        // Load and validate the entire finalized dataset before producing any
        // output. Missing judgments and changed evidence fail closed.
        let finalized = load(path)?;
        report["adjudication"] = adjudication(&first, &second, &finalized)?;
    }
    if let Some(path) = tasks_path {
        report["taskAgreement"] = task_agreement(&first, &second, path)?;
    }
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &report)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(
            io::stderr().lock(),
            "review agreement audit failed: {error}"
        );
        std::process::exit(1);
    }
}
