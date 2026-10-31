//! End-to-end checks for offline ranker training reproducibility.
//! Synthetic fixtures are pipeline tests, not empirical model quality evidence.

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

fn reproduce(checkpoint: &Path, training: &Path, epochs: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_reproduce-repodna-ranker"));
    command.arg(checkpoint).arg(training);
    if let Some(epochs) = epochs {
        command.arg(epochs);
    }
    command.output().expect("launch offline reproducibility audit")
}

#[test]
fn saved_weights_reproduce_even_after_input_records_are_reordered() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("model.json");
    assert!(train(&checkpoint).status.success());
    let before = fs::read(&checkpoint).unwrap();
    let training = fixture("train.demo.json");

    let audit = reproduce(&checkpoint, &training, None);
    assert!(
        audit.status.success(),
        "audit stderr: {}",
        String::from_utf8_lossy(&audit.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&audit.stdout).unwrap();
    assert_eq!(report["reproduced"], true);
    assert_eq!(report["epochs"], 60);
    assert_eq!(report["trainingExamples"], 6);
    assert_eq!(report["trainingRepositories"], 2);
    assert_eq!(report["maxAbsoluteWeightDelta"], 0.0);

    let mut records: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(&training).unwrap()).unwrap();
    records.reverse();
    let reversed = dir.path().join("reordered.json");
    fs::write(&reversed, serde_json::to_vec(&records).unwrap()).unwrap();
    assert!(reproduce(&checkpoint, &reversed, Some("60")).status.success());
    assert_eq!(before, fs::read(&checkpoint).unwrap());

    // The report never exposes examples or repository identifiers.
    let output_text = String::from_utf8(audit.stdout).unwrap();
    assert!(!output_text.contains("synthetic-alpha"));
    assert!(!output_text.contains("src/main.rs"));
    assert!(!output_text.contains("\"relevant\":"));
}

#[test]
fn altered_weights_or_labels_fail_even_when_training_counts_match() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("model.json");
    assert!(train(&checkpoint).status.success());

    let mut saved: serde_json::Value =
        serde_json::from_slice(&fs::read(&checkpoint).unwrap()).unwrap();
    let original = saved["weights"][0].as_f64().unwrap();
    saved["weights"][0] = (original + 0.25).into();
    let altered = dir.path().join("altered-checkpoint.json");
    fs::write(&altered, serde_json::to_vec(&saved).unwrap()).unwrap();
    let weight_result = reproduce(&altered, &fixture("train.demo.json"), None);
    assert!(!weight_result.status.success());
    assert!(weight_result.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&weight_result.stderr)
            .contains("weights differ from deterministic retraining")
    );

    let mut records: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("train.demo.json")).unwrap()).unwrap();
    let previous = records[0]["relevant"].as_bool().unwrap();
    records[0]["relevant"] = (!previous).into();
    let altered_labels = dir.path().join("altered-labels.json");
    fs::write(&altered_labels, serde_json::to_vec(&records).unwrap()).unwrap();
    let label_result = reproduce(&checkpoint, &altered_labels, None);
    assert!(!label_result.status.success());
    assert!(label_result.stdout.is_empty());
}

#[test]
fn rejects_wrong_epoch_setting_and_bad_inputs_without_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("model.json");
    assert!(train(&checkpoint).status.success());

    for epochs in ["0", "201", "abc"] {
        let result = reproduce(&checkpoint, &fixture("train.demo.json"), Some(epochs));
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
    let wrong_epochs = reproduce(&checkpoint, &fixture("train.demo.json"), Some("1"));
    assert!(!wrong_epochs.status.success());
    assert!(
        String::from_utf8_lossy(&wrong_epochs.stderr)
            .contains("weights differ from deterministic retraining")
    );

    let too_large = dir.path().join("oversized.json");
    fs::write(&too_large, vec![b' '; 64 * 1024 + 1]).unwrap();
    let result = reproduce(&too_large, &fixture("train.demo.json"), None);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("65536-byte limit"));
}
