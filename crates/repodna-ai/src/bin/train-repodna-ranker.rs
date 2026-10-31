//! Offline training and repository-disjoint evaluation for RepoDNA's ranker.
//!
//! Usage: train-repodna-ranker TRAIN.json HOLDOUT.json OUTPUT.json
//! Inputs are arrays of LabeledEvidence objects. No network or model download.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use repodna_ai::ranking::{
    LabeledEvidence, MAX_EXAMPLES, RankerModel, evaluate_original_order, validate_repository_split,
};

const MAX_INPUT_BYTES: u64 = 8 * 1024 * 1024;

/// Read bounded JSON without making a network request.
fn load(path: &Path) -> Result<Vec<LabeledEvidence>, Box<dyn std::error::Error>> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_INPUT_BYTES {
        return Err("dataset exceeds the 8 MiB size limit".into());
    }
    let data = fs::read(path)?;
    if data.len() as u64 > MAX_INPUT_BYTES {
        return Err("dataset grew beyond the 8 MiB size limit".into());
    }
    let examples: Vec<LabeledEvidence> = serde_json::from_slice(&data)?;
    if examples.len() > MAX_EXAMPLES {
        return Err("too many labeled examples".into());
    }
    Ok(examples)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(training), Some(holdout), Some(destination), None) =
        (args.next(), args.next(), args.next(), args.next())
    else {
        return Err("usage: train-repodna-ranker TRAIN.json HOLDOUT.json OUTPUT.json".into());
    };
    let training = load(Path::new(&training))?;
    let holdout = load(Path::new(&holdout))?;
    validate_repository_split(&training, &holdout).map_err(io::Error::other)?;
    let model = RankerModel::train(&training, 60).map_err(io::Error::other)?;
    let evaluation = model.evaluate(&holdout);
    let score_diagnostics = model
        .evaluate_relevance_scores(&holdout)
        .map_err(io::Error::other)?;
    let retrieval = model
        .evaluate_retrieval(&holdout, 5)
        .map_err(io::Error::other)?;
    let baseline = evaluate_original_order(&holdout, 5).map_err(io::Error::other)?;
    let delta = retrieval.delta_from(baseline).map_err(io::Error::other)?;
    let answerability_sweep = model
        .evaluate_answerability_sweep(&holdout, &[0.25, 0.5, 0.75])
        .map_err(io::Error::other)?;
    let answerability = answerability_sweep[1];
    let payload = serde_json::to_vec_pretty(&model)?;
    // Never silently destroy an existing or unrelated checkpoint.
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(Path::new(&destination))?;
    output.write_all(&payload)?;
    output.write_all(b"\n")?;
    output.flush()?;
    let results = serde_json::json!({
        "modelFormat": model.format,
        "trainingExamples": model.training_examples,
        "trainingRepositories": model.training_repositories,
        "trainingDatasetSha256": model.training_dataset_sha256,
        "holdout": evaluation,
        "holdoutAccuracy": evaluation.accuracy(),
        "relevanceScoreDiagnostics": score_diagnostics,
        "retrieval": retrieval,
        "originalOrderBaseline": baseline,
        "retrievalDelta": delta,
        "answerability": answerability,
        "answerabilitySweep": answerability_sweep,
        "answerableDetectionRate": answerability.answerable_detection_rate(),
        "unanswerableAbstentionRate": answerability.unanswerable_abstention_rate(),
        "note": "Synthetic fixtures demonstrate the pipeline, not real-world quality; retrieval scores only evaluate labels, not factual correctness"
    });
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &results)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(io::stderr().lock(), "training failed: {error}");
        std::process::exit(1);
    }
}
