//! Verify the identity of offline AI benchmark artifacts without network use.
//!
//! Usage: verify-repodna-benchmark REPORT.json MODEL.json TRAIN.json
//!        VALIDATION.json TEST.json [FAMILIES.json]
//!
//! This verifies file contents and report metadata, NOT whether the reported
//! metrics are scientifically valid, the original training was honest, or the
//! curator correctly identified clone/fork families.

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

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (
        Some(report_path),
        Some(model_path),
        Some(train_path),
        Some(valid_path),
        Some(test_path),
        family_path,
        None,
    ) = (
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
            "usage: verify-repodna-benchmark REPORT.json MODEL.json TRAIN.json VALIDATION.json TEST.json [FAMILIES.json]"
                .into(),
        );
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

    // Report only summary fingerprints and aggregate facts.
    let result = serde_json::json!({
        "verified": true,
        "schema": "repodna-benchmark-provenance/1",
        "checkpointSha256": model_sha256,
        "trainingDatasetSha256": training_sha256,
        "validationDatasetSha256": validation_sha256,
        "finalTestDatasetSha256": test_sha256,
        "familyManifestSha256": manifest_hash,
        "familyManifestChecked": manifest_hash.is_some(),
        "note": "Verified typed input and model identities, split boundaries, and selection metadata consistency. This does not authenticate model authorship, verify family curator claims, reproduce training weights, recompute every metric, or prove benchmark accuracy."
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
