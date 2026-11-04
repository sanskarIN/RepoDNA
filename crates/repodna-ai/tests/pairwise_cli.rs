//! Verify the pairwise ranker CLI and checkpoint reproducibility offline.
//! Synthetic fixtures verify plumbing, not real-world model effectiveness.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use repodna_ai::ranking::{PAIRWISE_MODEL_FORMAT, RankerModel};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/native-ai")
        .join(name)
}

fn train(training: &Path, holdout: &Path, destination: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_train-repodna-pairwise"))
        .arg(training)
        .arg(holdout)
        .arg(destination)
        .output()
        .expect("launch pairwise ranker trainer")
}

#[test]
fn exports_valid_pairwise_checkpoint_and_offline_comparison() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("pairwise.json");
    let trained = train(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &checkpoint,
    );
    assert!(
        trained.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&trained.stderr)
    );

    let saved = fs::read(&checkpoint).unwrap();
    let model: RankerModel = serde_json::from_slice(&saved).unwrap();
    model.validate().unwrap();
    assert_eq!(model.format, PAIRWISE_MODEL_FORMAT);
    assert_eq!(model.training_examples, 6);
    assert_eq!(model.training_repositories, 2);
    assert!(model.training_dataset_sha256.is_some());

    let report: serde_json::Value = serde_json::from_slice(&trained.stdout).unwrap();
    assert_eq!(report["modelFormat"], PAIRWISE_MODEL_FORMAT);
    assert_eq!(report["epochs"], 60);
    assert_eq!(report["retrieval"]["evaluatedQueries"], 2);
    assert!(report["retrievalDelta"]["ndcgAtK"].is_number());
    assert_eq!(report["pointwiseRetrieval"]["evaluatedQueries"], 2);
    assert!(report["pairwiseVsPointwiseDelta"]["ndcgAtK"].is_number());
    assert!(!String::from_utf8_lossy(&trained.stdout).contains("synthetic-alpha"));

    let duplicate = train(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &checkpoint,
    );
    assert!(!duplicate.status.success());
    assert!(duplicate.stdout.is_empty());
    assert_eq!(saved, fs::read(&checkpoint).unwrap());

    let audited = Command::new(env!("CARGO_BIN_EXE_reproduce-repodna-ranker"))
        .arg(&checkpoint)
        .arg(fixture("train.demo.json"))
        .arg("60")
        .output()
        .expect("reproduce pairwise weights");
    assert!(
        audited.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&audited.stderr)
    );
    let audit: serde_json::Value = serde_json::from_slice(&audited.stdout).unwrap();
    assert_eq!(audit["reproduced"], true);
}

#[test]
fn refuses_repository_overlap_and_unpaired_training() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("must-not-exist.json");
    let overlap = train(
        &fixture("train.demo.json"),
        &fixture("train.demo.json"),
        &checkpoint,
    );
    assert!(!overlap.status.success());
    assert!(overlap.stdout.is_empty());
    assert!(String::from_utf8_lossy(&overlap.stderr).contains("repository leakage"));

    let mut labels: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("train.demo.json")).unwrap()).unwrap();
    for sample in &mut labels {
        sample["relevant"] = true.into();
    }
    let unpaired = dir.path().join("unpaired.json");
    fs::write(&unpaired, serde_json::to_vec(&labels).unwrap()).unwrap();
    let failed = train(&unpaired, &fixture("holdout.demo.json"), &checkpoint);
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("same question"));
    assert!(!checkpoint.exists());
}
