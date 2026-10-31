//! Train the optional pairwise native evidence ranker entirely offline.
//!
//! Usage: `train-repodna-pairwise TRAIN.json HOLDOUT.json OUTPUT.json`
//! Comparisons are formed only within the same repository and question.
//! Output scores are relative rankings, not calibrated truth probabilities.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use repodna_ai::ranking::{
    LabeledEvidence, MAX_EXAMPLES, RankerModel, evaluate_original_order, validate_repository_split,
};

const MAX_DATASET_BYTES: u64 = 8 * 1024 * 1024;
const DEFAULT_EPOCHS: u32 = 60;

fn load(path: &Path) -> Result<Vec<LabeledEvidence>, Box<dyn std::error::Error>> {
    if fs::metadata(path)?.len() > MAX_DATASET_BYTES {
        return Err("dataset exceeds the 8 MiB size limit".into());
    }
    let bytes = fs::read(path)?;
    if bytes.len() as u64 > MAX_DATASET_BYTES {
        return Err("dataset grew beyond the 8 MiB size limit".into());
    }
    let samples: Vec<LabeledEvidence> = serde_json::from_slice(&bytes)?;
    if samples.len() > MAX_EXAMPLES {
        return Err("too many labeled examples".into());
    }
    Ok(samples)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(training), Some(holdout), Some(destination), None) =
        (args.next(), args.next(), args.next(), args.next())
    else {
        return Err("usage: train-repodna-pairwise TRAIN.json HOLDOUT.json OUTPUT.json".into());
    };

    let training = load(Path::new(&training))?;
    let holdout = load(Path::new(&holdout))?;
    validate_repository_split(&training, &holdout).map_err(io::Error::other)?;
    let model = RankerModel::train_pairwise(&training, DEFAULT_EPOCHS)
        .map_err(io::Error::other)?;
    let retrieval = model
        .evaluate_retrieval(&holdout, 5)
        .map_err(io::Error::other)?;
    if retrieval.evaluated_queries == 0 {
        return Err("holdout has no answerable labeled questions".into());
    }
    let baseline = evaluate_original_order(&holdout, 5).map_err(io::Error::other)?;
    let delta = retrieval.delta_from(baseline).map_err(io::Error::other)?;
    // Use identical labels and epoch count for an honest objective comparison.
    // Neither model selection nor hyperparameters may use the final test set.
    let pointwise = RankerModel::train(&training, DEFAULT_EPOCHS)
        .map_err(io::Error::other)?;
    let pointwise_retrieval = pointwise
        .evaluate_retrieval(&holdout, 5)
        .map_err(io::Error::other)?;
    let pairwise_vs_pointwise = retrieval
        .delta_from(pointwise_retrieval)
        .map_err(io::Error::other)?;

    let payload = serde_json::to_vec_pretty(&model)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(Path::new(&destination))?;
    output.write_all(&payload)?;
    output.write_all(b"\n")?;
    output.flush()?;

    let report = serde_json::json!({
        "modelFormat": model.format,
        "trainingExamples": model.training_examples,
        "trainingRepositories": model.training_repositories,
        "trainingDatasetSha256": model.training_dataset_sha256,
        "epochs": DEFAULT_EPOCHS,
        "retrieval": retrieval,
        "originalOrderBaseline": baseline,
        "retrievalDelta": delta,
        "pointwiseRetrieval": pointwise_retrieval,
        "pairwiseVsPointwiseDelta": pairwise_vs_pointwise,
        "note": "Experimental pairwise evidence ranking. Same-epoch pointwise comparison uses the same training and holdout labels. Scores are not calibrated probabilities, factual accuracy, or language generation. Synthetic examples are plumbing tests only. Use separate validation repositories to choose models and reserve an untouched final test set."
    });
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &report)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(io::stderr().lock(), "pairwise training failed: {error}");
        std::process::exit(1);
    }
}
