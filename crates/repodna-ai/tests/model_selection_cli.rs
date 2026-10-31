//! End-to-end three-way native model selection tests.
//! Synthetic examples only verify the pipeline and leakage guards.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use repodna_ai::ranking::RankerModel;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/native-ai")
        .join(name)
}

fn select(training: &Path, validation: &Path, test: &Path, checkpoint: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_select-repodna-ranker"))
        .arg(training)
        .arg(validation)
        .arg(test)
        .arg(checkpoint)
        .output()
        .expect("launch offline model selector")
}

#[test]
fn chooses_from_validation_and_saves_a_valid_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("selected.json");
    let result = select(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
        &checkpoint,
    );
    assert!(
        result.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["objective"], "validationNdcgAt5");
    assert_eq!(report["candidateEpochs"].as_array().unwrap().len(), 5);
    assert_eq!(report["trainingExamples"], 6);
    assert_eq!(report["trainingRepositories"], 2);
    assert_eq!(report["testRetrieval"]["totalQueries"], 3);
    assert_eq!(report["testRetrieval"]["evaluatedQueries"], 2);
    assert_eq!(report["testRetrieval"]["unanswerableQueries"], 1);
    assert_eq!(report["testPairOrdering"]["totalPairs"], 2);
    assert_eq!(report["testPairOrdering"]["evaluatedQueries"], 2);
    assert!(report["validationPairOrdering"]["agreement"].is_number());
    assert!(report["testRetrievalDelta"]["ndcgAtK"].is_number());
    let selected_epochs = report["selectedEpochs"].as_u64().unwrap();
    assert!([10, 30, 60, 100, 150].contains(&selected_epochs));

    let checkpoint_data = fs::read(&checkpoint).unwrap();
    let model: RankerModel = serde_json::from_slice(&checkpoint_data).unwrap();
    model.validate().unwrap();
    assert!(model.training_dataset_sha256.is_some());
    assert_eq!(model.training_examples, 6);
    let output_text = String::from_utf8(result.stdout).unwrap();
    for private in ["synthetic-delta", "src/main.rs", "\"relevant\":"] {
        assert!(!output_text.contains(private));
    }

    // Never silently overwrite a previously exported model.
    let second = select(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
        &checkpoint,
    );
    assert!(!second.status.success());
    assert!(second.stdout.is_empty());
    assert_eq!(checkpoint_data, fs::read(&checkpoint).unwrap());
}

#[test]
fn test_labels_do_not_determine_selected_training_epochs() {
    let dir = tempfile::tempdir().unwrap();
    let first_path = dir.path().join("first.json");
    let second_path = dir.path().join("second.json");
    let original = select(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
        &first_path,
    );
    assert!(original.status.success());

    let mut samples: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("test.demo.json")).unwrap()).unwrap();
    // Alter the previously unseen test labels, preserving an answerable group.
    samples[0]["relevant"] = true.into();
    samples[1]["relevant"] = false.into();
    let altered = dir.path().join("altered-test.json");
    fs::write(&altered, serde_json::to_vec(&samples).unwrap()).unwrap();
    let changed = select(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &altered,
        &second_path,
    );
    assert!(
        changed.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&changed.stderr)
    );
    let before: serde_json::Value = serde_json::from_slice(&original.stdout).unwrap();
    let after: serde_json::Value = serde_json::from_slice(&changed.stdout).unwrap();
    assert_eq!(before["selectedEpochs"], after["selectedEpochs"]);
    assert_eq!(before["candidateEpochs"], after["candidateEpochs"]);
    assert_eq!(
        fs::read(first_path).unwrap(),
        fs::read(second_path).unwrap()
    );
}

#[test]
fn rejects_every_split_overlap_including_canonical_repository_aliases() {
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        ("train.demo.json", "train.demo.json", "test.demo.json"),
        ("train.demo.json", "holdout.demo.json", "train.demo.json"),
        ("train.demo.json", "holdout.demo.json", "holdout.demo.json"),
    ];
    for (index, (training, validation, test)) in cases.into_iter().enumerate() {
        let checkpoint = dir.path().join(format!("no-model-{index}.json"));
        let result = select(
            &fixture(training),
            &fixture(validation),
            &fixture(test),
            &checkpoint,
        );
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(!checkpoint.exists());
        assert!(String::from_utf8_lossy(&result.stderr).contains("repository leakage"));
    }

    let mut validation: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("holdout.demo.json")).unwrap()).unwrap();
    validation[0]["repository"] = " SYNTHETIC-ALPHA ".into();
    let alias = dir.path().join("aliased.json");
    fs::write(&alias, serde_json::to_vec(&validation).unwrap()).unwrap();
    let checkpoint = dir.path().join("alias-checkpoint.json");
    let result = select(
        &fixture("train.demo.json"),
        &alias,
        &fixture("test.demo.json"),
        &checkpoint,
    );
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!checkpoint.exists());
}

#[test]
fn refuses_unanswerable_validation_and_oversized_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let mut samples: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("holdout.demo.json")).unwrap()).unwrap();
    for sample in &mut samples {
        sample["relevant"] = false.into();
    }
    let validation = dir.path().join("unanswerable.json");
    fs::write(&validation, serde_json::to_vec(&samples).unwrap()).unwrap();
    let checkpoint = dir.path().join("must-not-exist.json");
    let result = select(
        &fixture("train.demo.json"),
        &validation,
        &fixture("test.demo.json"),
        &checkpoint,
    );
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!checkpoint.exists());
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("validation has no answerable labeled questions")
    );

    let large = dir.path().join("huge.json");
    fs::write(&large, vec![b' '; 8 * 1024 * 1024 + 1]).unwrap();
    let result = select(
        &large,
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
        &checkpoint,
    );
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!checkpoint.exists());
    assert!(String::from_utf8_lossy(&result.stderr).contains("8 MiB"));
}
