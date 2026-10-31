//! Reproduce native ranker weights from a labeled training set, entirely offline.
//!
//! Usage: reproduce-repodna-ranker MODEL.json TRAIN.json [EPOCHS]
//! The default 60 epochs matches train-repodna-ranker's current settings.
//! Matching weights are a reproducibility check, NOT proof of dataset identity:
//! different label files may yield identical feature vectors and weights.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use repodna_ai::ranking::{LabeledEvidence, MAX_EXAMPLES, RankerModel};

const MAX_DATASET_BYTES: u64 = 8 * 1024 * 1024;
const MAX_CHECKPOINT_BYTES: u64 = 64 * 1024;
const DEFAULT_EPOCHS: u32 = 60;
// Permit small floating-point differences across platforms, but not substantive
// checkpoint drift. This is NOT a cryptographic model or dataset attestation.
const RELATIVE_WEIGHT_TOLERANCE: f64 = 1e-12;

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

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(checkpoint), Some(training), maybe_epochs, None) =
        (args.next(), args.next(), args.next(), args.next())
    else {
        return Err(
            "usage: reproduce-repodna-ranker MODEL.json TRAIN.json [EPOCHS]".into(),
        );
    };
    let epochs = match maybe_epochs {
        Some(value) => value
            .to_str()
            .ok_or("epochs must be an integer between 1 and 200")?
            .parse::<u32>()?,
        None => DEFAULT_EPOCHS,
    };
    if !(1..=200).contains(&epochs) {
        return Err("epochs must be between 1 and 200".into());
    }

    let model_bytes = bounded_read(Path::new(&checkpoint), MAX_CHECKPOINT_BYTES)?;
    let model: RankerModel = serde_json::from_slice(&model_bytes)?;
    model.validate().map_err(io::Error::other)?;

    let training_bytes = bounded_read(Path::new(&training), MAX_DATASET_BYTES)?;
    let training: Vec<LabeledEvidence> = serde_json::from_slice(&training_bytes)?;
    if training.len() > MAX_EXAMPLES {
        return Err("too many labeled examples".into());
    }

    // Match the same deterministic learning algorithm used by the trainer.
    // It validates repository IDs, evidence IDs, source excerpt exclusion,
    // label balance and input bounds before fitting weights.
    let reproduced = RankerModel::train(&training, epochs).map_err(io::Error::other)?;
    if model.format != reproduced.format
        || model.training_examples != reproduced.training_examples
        || model.training_repositories != reproduced.training_repositories
    {
        return Err("checkpoint metadata differs from deterministic retraining".into());
    }
    let max_absolute_weight_delta = model
        .weights
        .iter()
        .zip(&reproduced.weights)
        .map(|(saved, expected)| (saved - expected).abs())
        .fold(0.0_f64, f64::max);
    let matches = model.weights.iter().zip(&reproduced.weights).all(
        |(saved, expected)| {
            (saved - expected).abs()
                <= RELATIVE_WEIGHT_TOLERANCE * expected.abs().max(1.0)
        },
    );
    if !matches {
        return Err("checkpoint weights differ from deterministic retraining".into());
    }

    // Never print labels, questions, evidence or source paths.
    let report = serde_json::json!({
        "modelFormat": model.format,
        "reproduced": true,
        "epochs": epochs,
        "trainingExamples": model.training_examples,
        "trainingRepositories": model.training_repositories,
        "maxAbsoluteWeightDelta": max_absolute_weight_delta,
        "relativeWeightTolerance": RELATIVE_WEIGHT_TOLERANCE,
        "note": "Weights match deterministic retraining within tolerance. This does not prove dataset identity, licensing, label correctness or model accuracy."
    });
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &report)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(io::stderr().lock(), "reproducibility audit failed: {error}");
        std::process::exit(1);
    }
}
