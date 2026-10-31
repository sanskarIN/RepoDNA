//! Verify the identity of offline AI benchmark artifacts without network use.
//!
//! Usage: verify-repodna-benchmark REPORT.json MODEL.json TRAIN.json
//!        VALIDATION.json TEST.json [FAMILIES.json]
//!
//! By default, checks artifact fingerprints and report metadata only.
//! Optional --replay retrains all ten candidates, independently recomputes
//! validation and final-test metrics, and checks the saved model weights.
//! Neither mode verifies reviewer quality, authorship or real generalization.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use repodna_ai::ranking::{
    LabeledEvidence, MAX_EXAMPLES, MODEL_FORMAT, PAIRWISE_MODEL_FORMAT, RankerModel,
    RepositoryProvenance, checkpoint_fingerprint, family_manifest_fingerprint,
    training_data_fingerprint, validate_repository_families, validate_repository_split,
};

const MAX_DATASET_BYTES: u64 = 8 * 1024 * 1024;
const MAX_REPORT_BYTES: u64 = 512 * 1024;
const MAX_CHECKPOINT_BYTES: u64 = 64 * 1024;
const MAX_MANIFEST_BYTES: u64 = 2 * 1024 * 1024;

fn load_bytes(path: &Path, limit: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if fs::metadata(path)?.len() > limit {
        return Err(format!("input exceeds {limit}-byte bound").into());
    }
    let bytes = fs::read(path)?;
    if bytes.len() as u64 > limit {
        return Err(format!("input grew beyond {limit}-byte bound").into());
    }
    Ok(bytes)
}

fn load_labels(path: &Path) -> Result<Vec<LabeledEvidence>, Box<dyn std::error::Error>> {
    let labels: Vec<LabeledEvidence> =
        serde_json::from_slice(&load_bytes(path, MAX_DATASET_BYTES)?)?;
    if labels.len() > MAX_EXAMPLES {
        return Err("too many labeled examples".into());
    }
    Ok(labels)
}

fn require_string(
    report: &serde_json::Value,
    key: &str,
    expected: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if report[key].as_str() != Some(expected) {
        return Err(format!("{key} fingerprint or metadata mismatch").into());
    }
    Ok(())
}


/// Compare a replayed metric object with the saved report. Integer counts and
/// object structures must match exactly; floating-point metrics tolerate
/// tiny platform-dependent rounding but never arbitrary score changes.
fn matching_metric(
    actual: &serde_json::Value,
    expected: &serde_json::Value,
    field: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match (actual, expected) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            if a.len() != b.len() || !a.keys().all(|key| b.contains_key(key)) {
                return Err(format!("{field}: replayed metric fields differ").into());
            }
            for (key, value) in b {
                matching_metric(&a[key], value, field)?;
            }
        }
        (serde_json::Value::Array(a), serde_json::Value::Array(b)) => {
            if a.len() != b.len() {
                return Err(format!("{field}: replayed metric array length differs").into());
            }
            for (saved, calculated) in a.iter().zip(b) {
                matching_metric(saved, calculated, field)?;
            }
        }
        (serde_json::Value::Number(a), serde_json::Value::Number(b)) => {
            if a.is_i64() && b.is_i64() {
                if a != b {
                    return Err(format!("{field}: replayed integer count differs").into());
                }
            } else {
                let reported = a.as_f64().ok_or("unreadable reported metric")?;
                let replayed = b.as_f64().ok_or("unreadable recomputed metric")?;
                if !reported.is_finite()
                    || !replayed.is_finite()
                    || (reported - replayed).abs() > 1e-10 * replayed.abs().max(1.0)
                {
                    return Err(format!("{field}: replayed metric value differs").into());
                }
            }
        }
        _ if actual == expected => {}
        _ => return Err(format!("{field}: replayed metric metadata differs").into()),
    }
    Ok(())
}

/// Rerun the frozen training and evaluation recipe. Selection uses validation
/// results only; final-test labels are read strictly after the winner is
/// chosen, and never influence objective or epoch decisions.
fn replay_metrics(
    report: &serde_json::Value,
    saved: &RankerModel,
    training: &[LabeledEvidence],
    validation: &[LabeledEvidence],
    test: &[LabeledEvidence],
) -> Result<(), Box<dyn std::error::Error>> {
    const EPOCHS: [u32; 5] = [10, 30, 60, 100, 150];
    let candidates = report["candidates"]
        .as_array()
        .ok_or("report is missing replayable candidates")?;
    if candidates.len() != 2 * EPOCHS.len() {
        return Err("replay requires ten ordered candidates".into());
    }
    let mut best: Option<(&str, u32, f64, RankerModel)> = None;
    let mut at = 0;
    for objective in ["pointwise", "pairwise"] {
        for epochs in EPOCHS {
            let model = match objective {
                "pointwise" => RankerModel::train(training, epochs),
                "pairwise" => RankerModel::train_pairwise(training, epochs),
                _ => unreachable!("static objective list"),
            }
            .map_err(io::Error::other)?;
            let retrieval = model.evaluate_retrieval(validation, 5).map_err(io::Error::other)?;
            let ordering = model
                .evaluate_pair_ordering(validation)
                .map_err(io::Error::other)?;
            if retrieval.evaluated_queries == 0 {
                return Err("replay validation contains no answerable questions".into());
            }
            let expected = serde_json::json!({
                "objective": objective,
                "epochs": epochs,
                "validationNdcgAt5": retrieval.ndcg_at_k,
                "evaluatedQueries": retrieval.evaluated_queries,
                "validationPairAgreement": ordering.agreement,
                "validationComparablePairs": ordering.total_pairs,
                "validationComparableQueries": ordering.evaluated_queries,
            });
            matching_metric(&candidates[at], &expected, "candidates")?;
            at += 1;
            if best
                .as_ref()
                .is_none_or(|(_, _, value, _)| retrieval.ndcg_at_k > *value)
            {
                best = Some((objective, epochs, retrieval.ndcg_at_k, model));
            }
        }
    }
    let (objective, epochs, _, selected) = best.ok_or("no replay candidate selected")?;
    if report["selectedObjective"] != objective || report["selectedEpochs"] != epochs {
        return Err("replayed selection differs from reported winner".into());
    }
    if saved.format != selected.format
        || saved.training_examples != selected.training_examples
        || saved.training_repositories != selected.training_repositories
        || saved.training_dataset_sha256 != selected.training_dataset_sha256
        || saved.weights.len() != selected.weights.len()
        || saved
            .weights
            .iter()
            .zip(&selected.weights)
            .any(|(a, b)| (a - b).abs() > 1e-10 * b.abs().max(1.0))
    {
        return Err("replayed model weights or training metadata differ".into());
    }
    let final_retrieval = selected.evaluate_retrieval(test, 5).map_err(io::Error::other)?;
    if final_retrieval.evaluated_queries == 0 {
        return Err("replay final test contains no answerable questions".into());
    }
    let original = repodna_ai::ranking::evaluate_original_order(test, 5)
        .map_err(io::Error::other)?;
    let delta = final_retrieval.delta_from(original).map_err(io::Error::other)?;
    let ordering = selected
        .evaluate_pair_ordering(test)
        .map_err(io::Error::other)?;
    for (name, expected) in [
        ("finalTestRetrieval", serde_json::to_value(final_retrieval)?),
        ("finalTestOriginalOrderBaseline", serde_json::to_value(original)?),
        ("finalTestRetrievalDelta", serde_json::to_value(delta)?),
        ("finalTestPairOrdering", serde_json::to_value(ordering)?),
    ] {
        matching_metric(&report[name], &expected, name)?;
    }
    Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (
        Some(report_path),
        Some(model_path),
        Some(train_path),
        Some(valid_path),
        Some(test_path),
        optional_arg,
        optional_flag,
        None,
    ) = (
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
    )
    else {
        return Err(
            "usage: verify-repodna-benchmark REPORT.json MODEL.json TRAIN.json VALIDATION.json TEST.json [FAMILIES.json] [--replay]"
                .into(),
        );
    };
    let (family_path, replay) = match (optional_arg, optional_flag) {
        (None, None) => (None, false),
        (Some(value), None) if value.to_str() == Some("--replay") => (None, true),
        (Some(value), None) => (Some(value), false),
        (Some(value), Some(flag))
            if value.to_str() != Some("--replay") && flag.to_str() == Some("--replay") =>
        {
            (Some(value), true)
        }
        _ => return Err("invalid optional family manifest or --replay argument".into()),
    };
    let report: serde_json::Value =
        serde_json::from_slice(&load_bytes(Path::new(&report_path), MAX_REPORT_BYTES)?)?;
    let model: RankerModel =
        serde_json::from_slice(&load_bytes(Path::new(&model_path), MAX_CHECKPOINT_BYTES)?)?;
    model.validate().map_err(io::Error::other)?;
    let training = load_labels(Path::new(&train_path))?;
    let validation = load_labels(Path::new(&valid_path))?;
    let test = load_labels(Path::new(&test_path))?;
    validate_repository_split(&training, &validation).map_err(io::Error::other)?;
    validate_repository_split(&training, &test).map_err(io::Error::other)?;
    validate_repository_split(&validation, &test).map_err(io::Error::other)?;

    let training_sha256 = training_data_fingerprint(&training).map_err(io::Error::other)?;
    let validation_sha256 = training_data_fingerprint(&validation).map_err(io::Error::other)?;
    let test_sha256 = training_data_fingerprint(&test).map_err(io::Error::other)?;
    let model_sha256 = checkpoint_fingerprint(&model).map_err(io::Error::other)?;

    require_string(
        &report,
        "provenanceSchema",
        "repodna-benchmark-provenance/1",
    )?;
    require_string(&report, "selectionMetric", "validationNdcgAt5")?;
    require_string(&report, "trainingDatasetSha256", &training_sha256)?;
    require_string(&report, "validationDatasetSha256", &validation_sha256)?;
    require_string(&report, "finalTestDatasetSha256", &test_sha256)?;
    require_string(&report, "checkpointSha256", &model_sha256)?;
    require_string(&report, "checkpointFormat", &model.format)?;
    if model.training_dataset_sha256.as_deref() != Some(training_sha256.as_str()) {
        return Err("saved checkpoint training label fingerprint mismatch".into());
    }
    if report["trainingExamples"].as_u64() != Some(model.training_examples as u64)
        || report["trainingRepositories"].as_u64() != Some(model.training_repositories as u64)
    {
        return Err("saved checkpoint training count mismatch".into());
    }
    let objective = match model.format.as_str() {
        MODEL_FORMAT => "pointwise",
        PAIRWISE_MODEL_FORMAT => "pairwise",
        _ => return Err("unsupported native model objective".into()),
    };
    require_string(&report, "selectedObjective", objective)?;
    let epochs = report["selectedEpochs"]
        .as_u64()
        .ok_or("report is missing selected epochs")?;
    if ![10, 30, 60, 100, 150].contains(&epochs) {
        return Err("report uses unsupported selection epoch count".into());
    }
    // Check reported decision metadata for self-consistency only. This does
    // not independently reproduce training, candidate metrics, or winner.
    let candidates = report["candidates"]
        .as_array()
        .ok_or("report is missing candidate list")?;
    if candidates.len() != 10 {
        return Err("benchmark report must contain exactly 10 candidates".into());
    }
    let mut best_score = f64::NEG_INFINITY;
    let mut winner = None;
    for item in candidates {
        let score = item["validationNdcgAt5"]
            .as_f64()
            .ok_or("invalid candidate validation score")?;
        let candidate_objective = item["objective"]
            .as_str()
            .ok_or("invalid candidate objective")?;
        let candidate_epochs = item["epochs"].as_u64().ok_or("invalid candidate epochs")?;
        if !(0.0..=1.0).contains(&score) {
            return Err("candidate validation score is out of range".into());
        }
        if score > best_score {
            best_score = score;
            winner = Some((candidate_objective, candidate_epochs));
        }
    }
    if winner != Some((objective, epochs)) {
        return Err(
            "selected objective and epochs differ from the reported validation winner".into(),
        );
    }

    let manifest_hash = if let Some(path) = family_path {
        let entries: Vec<RepositoryProvenance> =
            serde_json::from_slice(&load_bytes(Path::new(&path), MAX_MANIFEST_BYTES)?)?;
        let audit = validate_repository_families(&training, &validation, &test, &entries)
            .map_err(io::Error::other)?;
        if report["familyManifestChecked"] != true
            || report["familySplit"] != serde_json::to_value(audit)?
        {
            return Err("family split audit metadata differs from benchmark report".into());
        }
        let digest = family_manifest_fingerprint(&entries).map_err(io::Error::other)?;
        require_string(&report, "familyManifestSha256", &digest)?;
        Some(digest)
    } else {
        if report["familyManifestChecked"] != false
            || !report["familyManifestSha256"].is_null()
            || !report["familySplit"].is_null()
        {
            return Err("benchmark report requires its original family manifest".into());
        }
        None
    };

    if replay {
        replay_metrics(&report, &model, &training, &validation, &test)?;
    }

    // Report only summary fingerprints and aggregate facts.
    let result = serde_json::json!({
        "verified": true,
        "replayVerified": replay,
        "schema": "repodna-benchmark-provenance/1",
        "checkpointSha256": model_sha256,
        "trainingDatasetSha256": training_sha256,
        "validationDatasetSha256": validation_sha256,
        "finalTestDatasetSha256": test_sha256,
        "familyManifestSha256": manifest_hash,
        "familyManifestChecked": manifest_hash.is_some(),
        "note": if replay {
            "Offline replay recomputed all ten candidate validation metrics, selected checkpoint weights, and final-test metrics. This cannot authenticate model authorship, reviewer or clone-family claims, or scientific accuracy."
        } else {
            "Verified typed input and model identities, split boundaries, and selection metadata consistency. Use --replay to independently recompute native candidate training and metrics; file identity alone does not prove benchmark accuracy."
        },
    });
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &result)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(
            io::stderr().lock(),
            "benchmark provenance verification failed: {error}"
        );
        std::process::exit(1);
    }
}
