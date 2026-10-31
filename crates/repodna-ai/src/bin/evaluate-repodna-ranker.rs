//! Evaluate a frozen native ranker checkpoint without fitting new weights.
//!
//! Usage: evaluate-repodna-ranker MODEL.json TRAIN.json TEST.json
//! The original training labels allow repository-disjointness checks and,
//! for modern checkpoints, content fingerprint verification. Hashes do not
//! authenticate the model or prove that weights were honestly trained.

use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use repodna_ai::ranking::{
    LabeledEvidence, MAX_EXAMPLES, RankerModel, evaluate_original_order, training_data_fingerprint,
    validate_repository_split,
};

const MAX_DATASET_BYTES: u64 = 8 * 1024 * 1024;
const MAX_CHECKPOINT_BYTES: u64 = 64 * 1024;

fn bounded_read(path: &Path, limit: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if fs::metadata(path)?.len() > limit {
        return Err(format!("input exceeds the {limit}-byte limit").into());
    }
    let contents = fs::read(path)?;
    if contents.len() as u64 > limit {
        return Err(format!("input grew beyond the {limit}-byte limit").into());
    }
    Ok(contents)
}

fn load_samples(path: &Path) -> Result<Vec<LabeledEvidence>, Box<dyn std::error::Error>> {
    let input = bounded_read(path, MAX_DATASET_BYTES)?;
    let samples: Vec<LabeledEvidence> = serde_json::from_slice(&input)?;
    if samples.len() > MAX_EXAMPLES {
        return Err("too many labeled examples".into());
    }
    Ok(samples)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(checkpoint), Some(training), Some(test), None) =
        (args.next(), args.next(), args.next(), args.next())
    else {
        return Err("usage: evaluate-repodna-ranker MODEL.json TRAIN.json TEST.json".into());
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

    let classification = model.evaluate(&test);
    let score_diagnostics = model
        .evaluate_relevance_scores(&test)
        .map_err(io::Error::other)?;
    let retrieval = model
        .evaluate_retrieval(&test, 5)
        .map_err(io::Error::other)?;
    let baseline = evaluate_original_order(&test, 5).map_err(io::Error::other)?;
    let delta = retrieval.delta_from(baseline).map_err(io::Error::other)?;
    let answerability_sweep = model
        .evaluate_answerability_sweep(&test, &[0.25, 0.5, 0.75])
        .map_err(io::Error::other)?;
    let answerability = answerability_sweep[1];

    // Intentionally emit aggregate metrics only. Relevance labels and source
    // paths stay local and are not copied into the evaluation report.
    let report = serde_json::json!({
        "modelFormat": model.format,
        "trainingExamples": model.training_examples,
        "trainingRepositories": model.training_repositories,
        "trainingFingerprintVerified": fingerprint_verified,
        "holdout": classification,
        "holdoutAccuracy": classification.accuracy(),
        "relevanceScoreDiagnostics": score_diagnostics,
        "retrieval": retrieval,
        "originalOrderBaseline": baseline,
        "retrievalDelta": delta,
        "answerability": answerability,
        "answerabilitySweep": answerability_sweep,
        "answerableDetectionRate": answerability.answerable_detection_rate(),
        "unanswerableAbstentionRate": answerability.unanswerable_abstention_rate(),
        "note": "Checks repository splits and training counts; modern checkpoints also verify a SHA-256 hash of typed training labels. Fingerprints do not authenticate model weights or prove honest training. Legacy checkpoints lack the hash. Relevance labels do not establish factual correctness."
    });
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
