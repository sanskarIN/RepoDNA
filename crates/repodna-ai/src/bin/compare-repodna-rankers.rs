//! Offline selection between two native evidence ranking objectives.
//!
//! Usage: `compare-repodna-rankers TRAIN.json VALIDATION.json TEST.json OUTPUT.json`
//! Select objective and epochs ONLY on repository-disjoint validation NDCG@5.
//! Evaluate the saved winner on a separate final test once, after selection.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use repodna_ai::ranking::{
    LabeledEvidence, MAX_EXAMPLES, RankerModel, evaluate_original_order,
    validate_repository_split,
};

const MAX_DATASET_BYTES: u64 = 8 * 1024 * 1024;
const EPOCHS: [u32; 5] = [10, 30, 60, 100, 150];
const K: usize = 5;

fn load(path: &Path) -> Result<Vec<LabeledEvidence>, Box<dyn std::error::Error>> {
    if fs::metadata(path)?.len() > MAX_DATASET_BYTES {
        return Err("dataset exceeds 8 MiB".into());
    }
    let contents = fs::read(path)?;
    if contents.len() as u64 > MAX_DATASET_BYTES {
        return Err("dataset grew beyond 8 MiB".into());
    }
    let labels: Vec<LabeledEvidence> = serde_json::from_slice(&contents)?;
    if labels.len() > MAX_EXAMPLES {
        return Err("dataset exceeds maximum example count".into());
    }
    Ok(labels)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(train_path), Some(valid_path), Some(test_path), Some(destination), None) = (
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
    ) else {
        return Err(
            "usage: compare-repodna-rankers TRAIN.json VALIDATION.json TEST.json OUTPUT.json"
                .into(),
        );
    };
    let training = load(Path::new(&train_path))?;
    let validation = load(Path::new(&valid_path))?;
    let test = load(Path::new(&test_path))?;

    // Check ALL splits, including canonical repository aliases, before model
    // fitting. The test examples are not used to compare candidate models.
    validate_repository_split(&training, &validation).map_err(io::Error::other)?;
    validate_repository_split(&training, &test).map_err(io::Error::other)?;
    validate_repository_split(&validation, &test).map_err(io::Error::other)?;

    // Prefer the simpler pointwise objective and fewer epochs on exact ties.
    // No tuning is performed against the test set, including threshold tuning.
    let mut winner: Option<(&str, u32, f64, RankerModel)> = None;
    let mut candidates = Vec::new();
    for objective in ["pointwise", "pairwise"] {
        for epochs in EPOCHS {
            let model = match objective {
                "pointwise" => RankerModel::train(&training, epochs),
                "pairwise" => RankerModel::train_pairwise(&training, epochs),
                _ => unreachable!("static objective list"),
            }
            .map_err(io::Error::other)?;
            let score = model.evaluate_retrieval(&validation, K).map_err(io::Error::other)?;
            if score.evaluated_queries == 0 {
                return Err("validation has no answerable questions".into());
            }
            candidates.push(serde_json::json!({
                "objective": objective,
                "epochs": epochs,
                "validationNdcgAt5": score.ndcg_at_k,
                "evaluatedQueries": score.evaluated_queries,
            }));
            if winner
                .as_ref()
                .is_none_or(|(_, _, best, _)| score.ndcg_at_k > *best)
            {
                winner = Some((objective, epochs, score.ndcg_at_k, model));
            }
        }
    }
    let (objective, epochs, _, model) = winner.ok_or("no viable candidate")?;

    // Final test metrics are calculated strictly AFTER choosing objective and
    // epochs. Never use these values to select the next model revision.
    let test_retrieval = model.evaluate_retrieval(&test, K).map_err(io::Error::other)?;
    if test_retrieval.evaluated_queries == 0 {
        return Err("final test has no answerable questions".into());
    }
    let baseline = evaluate_original_order(&test, K).map_err(io::Error::other)?;
    let delta = test_retrieval.delta_from(baseline).map_err(io::Error::other)?;

    // Export only after validation and final evaluation have succeeded.
    // create_new refuses an accidental overwrite of a prior experiment.
    let bytes = serde_json::to_vec_pretty(&model)?;
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(Path::new(&destination))?;
    out.write_all(&bytes)?;
    out.write_all(b"\n")?;
    out.flush()?;

    let report = serde_json::json!({
        "selectionMetric": "validationNdcgAt5",
        "selectedObjective": objective,
        "selectedEpochs": epochs,
        "checkpointFormat": model.format,
        "trainingExamples": model.training_examples,
        "trainingRepositories": model.training_repositories,
        "trainingDatasetSha256": model.training_dataset_sha256,
        "candidates": candidates,
        "finalTestRetrieval": test_retrieval,
        "finalTestOriginalOrderBaseline": baseline,
        "finalTestRetrievalDelta": delta,
        "note": "Native evidence relevance ranking only. Evaluation metrics require independently reviewed repository-disjoint labels. Final test is not used for objective, epoch, or threshold selection. Synthetic demo data cannot establish generalization. Pairwise scores are not calibrated probabilities."
    });
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &report)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(
            io::stderr().lock(),
            "ranker objective selection failed: {error}"
        );
        std::process::exit(1);
    }
}
