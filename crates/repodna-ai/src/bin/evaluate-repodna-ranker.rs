//! Evaluate a frozen native ranker checkpoint without fitting new weights.
//!
//! Usage: evaluate-repodna-ranker MODEL.json TRAIN.json TEST.json [TEST_TASKS.json]
//! The original training labels allow repository-disjointness checks and,
//! for modern checkpoints, content fingerprint verification. Hashes do not
//! authenticate the model or prove that weights were honestly trained.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::{self, Write};
use std::path::Path;

use serde::Deserialize;

use repodna_ai::ranking::{
    LabeledEvidence, MAX_EXAMPLES, MODEL_FORMAT, RankerModel, evaluate_original_order,
    evaluate_repository_balanced, evaluate_repository_stability, training_data_fingerprint,
    validate_repository_split,
};

#[path = "support/bounded_input.rs"]
mod bounded_input;

const MAX_DATASET_BYTES: u64 = 8 * 1024 * 1024;
const MAX_CHECKPOINT_BYTES: u64 = 64 * 1024;
const MAX_TASK_MANIFEST_BYTES: u64 = 2 * 1024 * 1024;

fn bounded_read(path: &Path, limit: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    bounded_input::read_bounded_file(
        path,
        limit,
        &format!("input exceeds the {limit}-byte limit"),
    )
}

fn load_samples(path: &Path) -> Result<Vec<LabeledEvidence>, Box<dyn std::error::Error>> {
    let input = bounded_read(path, MAX_DATASET_BYTES)?;
    let samples: Vec<LabeledEvidence> = serde_json::from_slice(&input)?;
    if samples.len() > MAX_EXAMPLES {
        return Err("too many labeled examples".into());
    }
    Ok(samples)
}

// Per-question task labels are supplied and checked, never inferred from
// question wording. This manifest covers only the frozen final-test dataset.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskAssignment {
    repository: String,
    query: String,
    task: String,
}

const TASK_KINDS: [&str; 8] = [
    "architecture",
    "dependencies",
    "build_commands",
    "security",
    "onboarding",
    "history",
    "hotspots",
    "other",
];

fn task_key(repository: &str, query: &str) -> (String, String) {
    (repository.trim().to_lowercase(), query.to_owned())
}

/// Structural task coverage and frozen metrics only: never selects a model
/// or reports individual repository IDs, question strings or evidence.
fn evaluate_tasks(
    model: &RankerModel,
    test: &[LabeledEvidence],
    path: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let entries: Vec<TaskAssignment> =
        serde_json::from_slice(&bounded_read(path, MAX_TASK_MANIFEST_BYTES)?)?;
    if entries.is_empty() || entries.len() > MAX_EXAMPLES {
        return Err("final-test task manifest must contain 1..=100000 entries".into());
    }

    let mut assignments = BTreeMap::new();
    for entry in entries {
        if entry.repository.trim().is_empty()
            || entry.repository.len() > 200
            || entry.query.trim().is_empty()
            || entry.query.len() > 4_000
        {
            return Err("final-test task manifest contains missing or oversized fields".into());
        }
        if !TASK_KINDS.contains(&entry.task.as_str()) {
            return Err("final-test task manifest contains an unknown task category".into());
        }
        if assignments
            .insert(task_key(&entry.repository, &entry.query), entry.task)
            .is_some()
        {
            return Err("final-test task manifest contains duplicate normalized questions".into());
        }
    }

    let expected: BTreeSet<_> = test
        .iter()
        .map(|sample| task_key(&sample.repository, &sample.query))
        .collect();
    let supplied: BTreeSet<_> = assignments.keys().cloned().collect();
    if expected != supplied {
        return Err("final-test task manifest has missing or unused questions".into());
    }

    let mut grouped: BTreeMap<&str, Vec<LabeledEvidence>> = BTreeMap::new();
    for sample in test {
        let key = task_key(&sample.repository, &sample.query);
        let kind = assignments
            .get(&key)
            .ok_or("final-test task manifest is missing a question")?;
        grouped.entry(kind).or_default().push(sample.clone());
    }

    let mut by_task = BTreeMap::new();
    for kind in TASK_KINDS {
        let Some(samples) = grouped.get(kind) else {
            by_task.insert(
                kind,
                serde_json::json!({
                    "questions": 0,
                    "answerableQuestions": 0,
                    "unanswerableQuestions": 0,
                    "retrieval": null,
                    "originalOrderBaseline": null,
                    "retrievalDelta": null,
                    "repositoryBalancedRetrieval": null
                }),
            );
            continue;
        };
        let retrieval = model
            .evaluate_retrieval(samples, 5)
            .map_err(io::Error::other)?;
        let queries = retrieval.total_queries;
        let answerable = retrieval.evaluated_queries;
        let unanswerable = retrieval.unanswerable_queries;
        if answerable == 0 {
            by_task.insert(
                kind,
                serde_json::json!({
                    "questions": queries,
                    "answerableQuestions": answerable,
                    "unanswerableQuestions": unanswerable,
                    "retrieval": null,
                    "originalOrderBaseline": null,
                    "retrievalDelta": null,
                    "repositoryBalancedRetrieval": null
                }),
            );
            continue;
        }
        let baseline = evaluate_original_order(samples, 5).map_err(io::Error::other)?;
        let delta = retrieval.delta_from(baseline).map_err(io::Error::other)?;
        let repository_balanced =
            evaluate_repository_balanced(model, samples, 5).map_err(io::Error::other)?;
        by_task.insert(
            kind,
            serde_json::json!({
                "questions": queries,
                "answerableQuestions": answerable,
                "unanswerableQuestions": unanswerable,
                "retrieval": retrieval,
                "originalOrderBaseline": baseline,
                "retrievalDelta": delta,
                "repositoryBalancedRetrieval": repository_balanced
            }),
        );
    }
    Ok(serde_json::json!({
        "schema": "repodna-frozen-task-retrieval/1",
        "manifestQuestions": assignments.len(),
        "tasks": by_task,
        "note": "Categories were supplied by reviewers and checked against this frozen final-test set. Null metrics have no answerable questions; they are not zero model performance. This is a held-out diagnostic, never a model-selection criterion, and measures relevance labels rather than factual truth."
    }))
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(checkpoint), Some(training), Some(test), task_manifest, None) = (
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
    ) else {
        return Err(
            "usage: evaluate-repodna-ranker MODEL.json TRAIN.json TEST.json [TEST_TASKS.json]"
                .into(),
        );
    };

    let checkpoint = bounded_read(Path::new(&checkpoint), MAX_CHECKPOINT_BYTES)?;
    let model: RankerModel = serde_json::from_slice(&checkpoint)?;
    model.validate().map_err(io::Error::other)?;

    let training = load_samples(Path::new(&training))?;
    let test = load_samples(Path::new(&test))?;
    validate_repository_split(&training, &test).map_err(io::Error::other)?;

    // Compare basic provenance, then verify the full typed label contents
    // for checkpoints that recorded a SHA-256 fingerprint.
    let training_repositories = training
        .iter()
        .map(|sample| sample.repository.trim().to_lowercase())
        .collect::<HashSet<_>>()
        .len();
    if model.training_examples != training.len()
        || model.training_repositories != training_repositories
    {
        return Err("checkpoint training counts do not match the supplied training data".into());
    }
    if !training.iter().any(|sample| sample.relevant)
        || !training.iter().any(|sample| !sample.relevant)
    {
        return Err("training labels must include positive and negative examples".into());
    }
    let fingerprint_verified = if let Some(expected) = &model.training_dataset_sha256 {
        let actual = training_data_fingerprint(&training).map_err(io::Error::other)?;
        if &actual != expected {
            return Err("checkpoint training dataset fingerprint mismatch".into());
        }
        true
    } else {
        false // Legacy checkpoints predate training dataset fingerprints.
    };

    // Both pointwise and pairwise checkpoints can be judged by evidence
    // ordering. Only the pointwise model exposes classification, calibration
    // and absolute answerability-threshold diagnostics. Pairwise sigmoid
    // scores are relative-order transforms, NOT calibrated probabilities.
    let retrieval = model
        .evaluate_retrieval(&test, 5)
        .map_err(io::Error::other)?;
    let baseline = evaluate_original_order(&test, 5).map_err(io::Error::other)?;
    let delta = retrieval.delta_from(baseline).map_err(io::Error::other)?;
    let pair_ordering = model
        .evaluate_pair_ordering(&test)
        .map_err(io::Error::other)?;
    let repository_balanced =
        evaluate_repository_balanced(&model, &test, 5).map_err(io::Error::other)?;
    let repository_stability =
        evaluate_repository_stability(&model, &test, 5).map_err(io::Error::other)?;

    // Emit only aggregate metrics. No labels, source paths or evidence text.
    let mut report = serde_json::json!({
        "modelFormat": model.format,
        "trainingExamples": model.training_examples,
        "trainingRepositories": model.training_repositories,
        "trainingFingerprintVerified": fingerprint_verified,
        "retrieval": retrieval,
        "originalOrderBaseline": baseline,
        "retrievalDelta": delta,
        "pairOrdering": pair_ordering,
        "repositoryBalancedRetrieval": repository_balanced,
        "repositoryStability": repository_stability,
    });

    if let Some(manifest_path) = task_manifest {
        report["taskStratifiedRetrieval"] =
            evaluate_tasks(&model, &test, Path::new(&manifest_path))?;
    }

    if model.format == MODEL_FORMAT {
        let classification = model.evaluate(&test);
        let score_diagnostics = model
            .evaluate_relevance_scores(&test)
            .map_err(io::Error::other)?;
        let answerability_sweep = model
            .evaluate_answerability_sweep(&test, &[0.25, 0.5, 0.75])
            .map_err(io::Error::other)?;
        let answerability = answerability_sweep[1];
        report["scoreSemantics"] = serde_json::json!("pointwise-relevance");
        report["holdout"] = serde_json::json!(classification);
        report["holdoutAccuracy"] = serde_json::json!(classification.accuracy());
        report["relevanceScoreDiagnostics"] = serde_json::json!(score_diagnostics);
        report["answerability"] = serde_json::json!(answerability);
        report["answerabilitySweep"] = serde_json::json!(answerability_sweep);
        report["answerableDetectionRate"] =
            serde_json::json!(answerability.answerable_detection_rate());
        report["unanswerableAbstentionRate"] =
            serde_json::json!(answerability.unanswerable_abstention_rate());
        report["note"] = serde_json::json!(
            "Checks repository splits and training counts; modern checkpoints also verify a SHA-256 hash of typed training labels. Fingerprints do not authenticate model weights or prove honest training. Legacy checkpoints lack the hash. Relevance labels do not establish factual correctness."
        );
    } else {
        report["scoreSemantics"] = serde_json::json!("relative-ranking-only");
        report["note"] = serde_json::json!(
            "Pairwise scores are relative ranking signals, not calibrated relevance probabilities. Classification, score calibration and answerability-threshold metrics are intentionally omitted. Training fingerprint checks do not authenticate weights or prove honest training. Evidence relevance is not factual correctness."
        );
    }

    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &report)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(io::stderr().lock(), "evaluation failed: {error}");
        std::process::exit(1);
    }
}
