//! Offline, aggregate-only agreement audit for two independently reviewed
//! RepoDNA evidence annotation files.
//!
//! Usage: audit-repodna-reviewers REVIEW_A.json REVIEW_B.json [ADJUDICATED.json] [--tasks TASKS.json] [--source FINAL_DATASET.json]
//! Reviewers must judge the exact same evidence records. This audit measures
//! consistency, not independence, factual correctness or reviewer expertise.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use std::io::{self, Write};
use std::path::Path;

use repodna_ai::ranking::{
    AnnotationExample, MAX_EXAMPLES, finish_annotations, training_data_fingerprint,
};

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

/// Count evidence judgments by normalized repository and exact question.
fn question_counts(reviews: &[AnnotationExample]) -> BTreeMap<(String, String), usize> {
    let mut counts = BTreeMap::new();
    for review in reviews {
        *counts
            .entry((
                review.repository.trim().to_lowercase(),
                review.query.clone(),
            ))
            .or_insert(0) += 1;
    }
    counts
}

/// Verify that the independently double-reviewed subset is drawn from a fixed,
/// fully labeled source dataset. When adjudication is provided, require every
/// finalized reviewed judgment to agree with the corresponding source label.
/// Neither structural membership nor a matching label proves human review.
fn source_coverage(
    first: &[AnnotationExample],
    final_review: Option<&[AnnotationExample]>,
    source_path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let source = load(source_path)?;
    let full = records(&source)?;
    let subset = records(first)?;
    for identity in subset.keys() {
        if !full.contains_key(identity) {
            return Err("reviewed evidence record is not present in source dataset".into());
        }
    }

    if let Some(final_review) = final_review {
        for (identity, label) in records(final_review)? {
            if full.get(&identity) != Some(&label) {
                return Err("adjudicated label differs from source dataset".into());
            }
        }
    }

    let source_questions = question_counts(&source);
    let reviewed_questions = question_counts(first);
    let mut completely_reviewed = 0_usize;
    let mut partially_reviewed = 0_usize;
    let mut unreviewed = 0_usize;
    for (question, total) in &source_questions {
        match reviewed_questions.get(question).copied().unwrap_or(0) {
            0 => unreviewed += 1,
            count if count == *total => completely_reviewed += 1,
            count if count < *total => partially_reviewed += 1,
            _ => return Err("reviewed question exceeds source evidence count".into()),
        }
    }
    let source_repositories: BTreeSet<_> = source
        .iter()
        .map(|row| row.repository.trim().to_lowercase())
        .collect();
    let reviewed_repositories: BTreeSet<_> = first
        .iter()
        .map(|row| row.repository.trim().to_lowercase())
        .collect();

    let labeled = finish_annotations(source).map_err(io::Error::other)?;
    let fingerprint = training_data_fingerprint(&labeled).map_err(io::Error::other)?;
    Ok(serde_json::json!({
        "schema": "repodna-double-review-coverage/1",
        "sourceDatasetSha256": fingerprint,
        "sourceAnnotations": full.len(),
        "doubleReviewedAnnotations": subset.len(),
        "notDoubleReviewedAnnotations": full.len() - subset.len(),
        "annotationCoverageRate": subset.len() as f64 / full.len() as f64,
        "sourceQuestions": source_questions.len(),
        "doubleReviewedQuestions": reviewed_questions.len(),
        "completelyDoubleReviewedQuestions": completely_reviewed,
        "partiallyDoubleReviewedQuestions": partially_reviewed,
        "notDoubleReviewedQuestions": unreviewed,
        "sourceRepositories": source_repositories.len(),
        "doubleReviewedRepositories": reviewed_repositories.len(),
        "adjudicatedLabelsMatchSourceVerified": final_review.is_some(),
        "note": "Structural membership only: coverage does not establish reviewer independence, correctness or representative sampling. Matching finalized labels are checked only when ADJUDICATED.json is supplied. The dataset fingerprint can link repeated uses of private data."
    }))
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let usage = "usage: audit-repodna-reviewers REVIEW_A.json REVIEW_B.json [ADJUDICATED.json] [--tasks TASKS.json] [--source FINAL_DATASET.json]";
    if args.len() < 2 {
        return Err(usage.into());
    }
    let (mut final_path, mut tasks_path, mut source_path) = (None, None, None);
    let mut index = 2;
    while index < args.len() {
        match args[index].to_str() {
            Some("--tasks") | Some("--source") => {
                let flag = args[index].to_str().ok_or(usage)?;
                let value = args.get(index + 1).ok_or(usage)?;
                if value.to_str().is_some_and(|text| text.starts_with("--")) {
                    return Err(usage.into());
                }
                let destination = if flag == "--tasks" {
                    &mut tasks_path
                } else {
                    &mut source_path
                };
                if destination.replace(Path::new(value)).is_some() {
                    return Err("duplicate reviewer audit option".into());
                }
                index += 2;
            }
            Some(flag) if flag.starts_with("--") => return Err(usage.into()),
            _ if final_path.is_none() && tasks_path.is_none() && source_path.is_none() => {
                final_path = Some(Path::new(&args[index]));
                index += 1;
            }
            _ => return Err(usage.into()),
        }
    }

    let first = load(Path::new(&args[0]))?;
    let second = load(Path::new(&args[1]))?;
    let mut report = agreement(&first, &second)?;
    let finalized = if let Some(path) = final_path {
        // Load and validate the entire finalized subset before producing any
        // output. Missing judgments and changed evidence fail closed.
        let labels = load(path)?;
        report["adjudication"] = adjudication(&first, &second, &labels)?;
        Some(labels)
    } else {
        None
    };
    if let Some(path) = tasks_path {
        report["taskAgreement"] = task_agreement(&first, &second, path)?;
    }
    if let Some(path) = source_path {
        report["sourceCoverage"] = source_coverage(&first, finalized.as_deref(), path)?;
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
