//! Offline, end-to-end regression tests for the native ranker CLI.
//!
//! Exercise the executable and its JSON/checkpoint boundary, not just library
//! helpers. Synthetic fixtures are smoke tests, never model-quality evidence.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use repodna_ai::ranking::RankerModel;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/native-ai")
        .join(name)
}

fn train(training: &Path, holdout: &Path, destination: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_train-repodna-ranker"))
        .arg(training)
        .arg(holdout)
        .arg(destination)
        .output()
        .expect("launch offline ranker trainer")
}

#[test]
fn trainer_emits_valid_checkpoint_and_complete_diagnostics() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("trained.json");
    let output = train(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &checkpoint,
    );
    assert!(
        output.status.success(),
        "trainer stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let model: RankerModel = serde_json::from_slice(&fs::read(&checkpoint).unwrap()).unwrap();
    model.validate().unwrap();
    assert_eq!(model.training_examples, 6);
    assert_eq!(model.training_repositories, 2);

    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["modelFormat"], model.format);
    assert_eq!(report["trainingExamples"], 6);
    assert_eq!(report["trainingRepositories"], 2);
    assert_eq!(report["holdout"]["samples"], 4);
    assert_eq!(report["retrieval"]["k"], 5);
    assert_eq!(report["originalOrderBaseline"]["k"], 5);

    let diagnostics = &report["relevanceScoreDiagnostics"];
    assert_eq!(diagnostics["samples"], 4);
    assert_eq!(
        diagnostics["bins"]
            .as_array()
            .unwrap()
            .iter()
            .map(|bin| bin["samples"].as_u64().unwrap())
            .sum::<u64>(),
        4
    );
    for metric in ["brierScore", "logLoss", "expectedCalibrationError"] {
        let value = diagnostics[metric]
            .as_f64()
            .expect("numeric score diagnostic");
        assert!(value.is_finite() && value >= 0.0);
    }

    let thresholds = report["answerabilitySweep"].as_array().unwrap();
    assert_eq!(thresholds.len(), 3);
    assert_eq!(thresholds[0]["threshold"], 0.25);
    assert_eq!(thresholds[1]["threshold"], 0.5);
    assert_eq!(thresholds[2]["threshold"], 0.75);
    assert_eq!(report["answerability"], thresholds[1]);

    // The CLI must export identical weights for identical offline input.
    let second = dir.path().join("second.json");
    let repeated = train(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &second,
    );
    assert!(repeated.status.success());
    assert_eq!(fs::read(checkpoint).unwrap(), fs::read(second).unwrap());
}

#[test]
fn trainer_never_overwrites_existing_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("existing.json");
    fs::write(&checkpoint, b"keep this file unchanged").unwrap();

    let output = train(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &checkpoint,
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read(&checkpoint).unwrap(), b"keep this file unchanged");
}

#[test]
fn trainer_rejects_repository_alias_leakage_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    let holdout = dir.path().join("leaked-holdout.json");
    let checkpoint = dir.path().join("should-not-exist.json");
    let mut samples: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture("holdout.demo.json")).unwrap()).unwrap();
    samples[0]["repository"] = " SYNTHETIC-ALPHA ".into();
    fs::write(&holdout, serde_json::to_vec(&samples).unwrap()).unwrap();

    let output = train(&fixture("train.demo.json"), &holdout, &checkpoint);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("repository leakage"));
    assert!(!checkpoint.exists());
}
