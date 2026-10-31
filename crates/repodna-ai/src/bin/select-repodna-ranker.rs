//! Offline epoch selection using independent training, validation and test repositories.
//!
//! Usage: `select-repodna-ranker TRAIN.json VALIDATION.json TEST.json OUTPUT.json`
//! Candidate epochs are selected using validation NDCG@5 only. The test set is
//! evaluated exactly once after selection and is never used to choose settings.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use repodna_ai::ranking::{
    LabeledEvidence, MAX_EXAMPLES, RankerModel, evaluate_original_order,
    validate_repository_split,
};

const MAX_DATASET_BYTES: u64 = 8 * 1024 * 1024;
const CANDIDATE_EPOCHS: &[u32] = &[10, 30, 60, 100, 150];
const EVALUATION_K: usize = 5;

fn load(path: &Path) -> Result<Vec<LabeledEvidence>, Box<dyn std::error::Error>> {
    if fs::metadata(path)?.len() > MAX_DATASET_BYTES {
        return Err("dataset exceeds the 8 MiB size limit".into());
    }
    let contents = fs::read(path)?;
    if contents.len() as u64 > MAX_DATASET_BYTES {
        return Err("dataset grew beyond the 8 MiB size limit".into());
    }
    let samples: Vec<LabeledEvidence> = serde_json::from_slice(&contents)?;
    if samples.len() > MAX_EXAMPLES {
        return Err("too many labeled examples".into());
    }
    Ok(samples)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(training), Some(validation), Some(test), Some(destination), None) =
        (args.next(), args.next(), args.next(), args.next(), args.next())
    else {
        return Err(
            "usage: select-repodna-ranker TRAIN.json VALIDATION.json TEST.json OUTPUT.json"
                .into(),
        );
    };

    let training = load(Path::new(&training))?;
    let validation = load(Path::new(&validation))?;
    let test = load(Path::new(&test))?;
    // Canonical repository identifiers prevent alias leakage in EVERY pair.
    // The same validator also rejects repeated evidence IDs and excerpts.
    validate_repository_split(&training, &validation).map_err(io::Error::other)?;
    validate_repository_split(&training, &test).map_err(io::Error::other)?;
    validate_repository_split(&validation, &test).map_err(io::Error::other)?;

    // Train() validates class balance, full sample bounds and checkpoint schema.
    // Select settings using validation only; never access test metrics in this loop.
    let mut chosen: Option<(u32, RankerModel, f64)> = None;
    let mut candidates = Vec::new();
    for &epochs in CANDIDATE_EPOCHS {
        let model = RankerModel::train(&training, epochs).map_err(io::Error::other)?;
        let validation_metrics = model
            .evaluate_retrieval(&validation, EVALUATION_K)
            .map_err(io::Error::other)?;
        if validation_metrics.evaluated_queries == 0 {
            return Err("validation has no answerable labeled questions".into());
        }
        let ndcg = validation_metrics.ndcg_at_k;
        candidates.push(serde_json::json!({
            "epochs": epochs,
            "ndcgAt5": ndcg,
            "evaluatedQueries": validation_metrics.evaluated_queries,
        }));
        // Strict improvement; epochs are ascending so ties select fewer epochs.
        if chosen
            .as_ref()
            .is_none_or(|( _, _, previous)| ndcg > *previous)
        {
            chosen = Some((epochs, model, ndcg));
        }
    }

    let (selected_epochs, model, _) = chosen.ok_or("no training candidates")?;
    // Only now read test *labels* for scoring. The data was loaded and split-
    // validated above, but no test relevance metric influenced model selection.
    let test_retrieval = model
        .evaluate_retrieval(&test, EVALUATION_K)
        .map_err(io::Error::other)?;
    if test_retrieval.evaluated_queries == 0 {
        return Err("test has no answerable labeled questions".into());
    }
    let test_baseline = evaluate_original_order(&test, EVALUATION_K)
        .map_err(io::Error::other)?;
    let test_delta = test_retrieval
        .delta_from(test_baseline)
        .map_err(io::Error::other)?;
    let test_scores = model
        .evaluate_relevance_scores(&test)
        .map_err(io::Error::other)?;
    let validation_baseline = evaluate_original_order(&validation, EVALUATION_K)
        .map_err(io::Error::other)?;

    let payload = serde_json::to_vec_pretty(&model)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(Path::new(&destination))?;
    output.write_all(&payload)?;
    output.write_all(b"\n")?;
    output.flush()?;

    // Aggregate-only report. Do not output repository paths, questions or labels.
    let report = serde_json::json!({
        "modelFormat": model.format,
        "trainingExamples": model.training_examples,
        "trainingRepositories": model.training_repositories,
        "trainingDatasetSha256": model.training_dataset_sha256,
        "objective": "validationNdcgAt5",
        "candidateEpochs": candidates,
        "selectedEpochs": selected_epochs,
        "validationOriginalOrderBaseline": validation_baseline,
        "testRetrieval": test_retrieval,
        "testOriginalOrderBaseline": test_baseline,
        "testRetrievalDelta": test_delta,
        "testRelevanceScoreDiagnostics": test_scores,
        "note": "Synthetic fixtures are only pipeline tests. Epochs are selected from repository-disjoint validation labels; final test labels are evaluated after selection and must not be used to retune the model. Ranking relevance is not factual correctness."
    });
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &report)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(io::stderr().lock(), "ranker model selection failed: {error}");
        std::process::exit(1);
    }
}
