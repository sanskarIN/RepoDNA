//! Exercise offline evaluation of saved ranker checkpoints at the CLI boundary.
//! Synthetic examples verify plumbing, not model quality on real repositories.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/native-ai")
        .join(name)
}

fn train(destination: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_train-repodna-ranker"))
        .arg(fixture("train.demo.json"))
        .arg(fixture("holdout.demo.json"))
        .arg(destination)
        .output()
        .expect("launch offline trainer")
}

fn evaluate(model: &Path, training: &Path, test: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_evaluate-repodna-ranker"))
        .arg(model)
        .arg(training)
        .arg(test)
        .output()
        .expect("launch frozen checkpoint evaluator")
}

#[test]
fn frozen_evaluation_matches_training_time_metrics_without_changing_weights() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("ranker.json");
    let trained = train(&checkpoint);
    assert!(
        trained.status.success(),
        "trainer stderr: {}",
        String::from_utf8_lossy(&trained.stderr)
    );
    let before = fs::read(&checkpoint).unwrap();

    let evaluated = evaluate(
        &checkpoint,
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
    );
    assert!(
        evaluated.status.success(),
        "evaluator stderr: {}",
        String::from_utf8_lossy(&evaluated.stderr)
    );
    let training_report: serde_json::Value = serde_json::from_slice(&trained.stdout).unwrap();
    let evaluation_report: serde_json::Value = serde_json::from_slice(&evaluated.stdout).unwrap();
    for key in [
        "holdout",
        "holdoutAccuracy",
        "relevanceScoreDiagnostics",
        "retrieval",
        "originalOrderBaseline",
        "retrievalDelta",
        "answerability",
        "answerabilitySweep",
        "answerableDetectionRate",
        "unanswerableAbstentionRate",
    ] {
        assert_eq!(training_report[key], evaluation_report[key], "mismatch: {key}");
    }
    assert_eq!(evaluation_report["trainingRepositories"], 2);
    assert_eq!(evaluation_report["trainingExamples"], 6);
    assert_eq!(before, fs::read(&checkpoint).unwrap());

    // No source paths, labels, or repository identifiers are included in the
    // aggregate report.
    let output_text = String::from_utf8(evaluated.stdout).unwrap();
    assert!(!output_text.contains("synthetic-alpha"));
    assert!(!output_text.contains("src/main.rs"));
    assert!(!output_text.contains("\"relevant\":"));
}

#[test]
fn frozen_evaluation_rejects_leakage_including_repository_aliases() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("ranker.json");
    assert!(train(&checkpoint).status.success());
    let holdout = dir.path().join("leaked.json");
    let mut records: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture("holdout.demo.json")).unwrap()).unwrap();
    records[0]["repository"] = " SYNTHETIC-ALPHA ".into();
    fs::write(&holdout, serde_json::to_vec(&records).unwrap()).unwrap();

    let output = evaluate(&checkpoint, &fixture("train.demo.json"), &holdout);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("repository leakage"));
}

#[test]
fn frozen_evaluation_rejects_corrupt_or_unrelated_checkpoints() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("ranker.json");
    assert!(train(&checkpoint).status.success());

    let mut model: serde_json::Value =
        serde_json::from_slice(&fs::read(&checkpoint).unwrap()).unwrap();
    model["weights"] = serde_json::json!([0.0]);
    let invalid = dir.path().join("invalid.json");
    fs::write(&invalid, serde_json::to_vec(&model).unwrap()).unwrap();
    let output = evaluate(
        &invalid,
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("number of ranker weights"));

    let mut model: serde_json::Value = serde_json::from_slice(&fs::read(&checkpoint).unwrap()).unwrap();
    model["training_examples"] = 7.into();
    let mismatched = dir.path().join("mismatched.json");
    fs::write(&mismatched, serde_json::to_vec(&model).unwrap()).unwrap();
    let output = evaluate(
        &mismatched,
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("training counts"));
}

#[test]
fn frozen_evaluation_rejects_repeated_holdout_labels_and_oversized_model() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("ranker.json");
    assert!(train(&checkpoint).status.success());

    let mut records: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("holdout.demo.json")).unwrap()).unwrap();
    records.push(records[0].clone());
    let duplicates = dir.path().join("duplicate.json");
    fs::write(&duplicates, serde_json::to_vec(&records).unwrap()).unwrap();

    let output = evaluate(&checkpoint, &fixture("train.demo.json"), &duplicates);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("duplicate evidence ID"));

    let oversized = dir.path().join("oversized.json");
    fs::write(&oversized, vec![b' '; 64 * 1024 + 1]).unwrap();
    let output = evaluate(
        &oversized,
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("65536-byte limit"));
}
