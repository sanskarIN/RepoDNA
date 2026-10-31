//! Integration tests for offline objective and epoch selection.
//! Synthetic JSON fixtures check split handling and reproducibility, not accuracy.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use repodna_ai::ranking::{MODEL_FORMAT, PAIRWISE_MODEL_FORMAT, RankerModel};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/native-ai")
        .join(name)
}

fn compare(train: &Path, valid: &Path, test: &Path, out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_compare-repodna-rankers"))
        .arg(train)
        .arg(valid)
        .arg(test)
        .arg(out)
        .output()
        .expect("launch model comparison")
}

#[test]
fn selects_objective_from_validation_and_exports_reproducible_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("selected-ranker.json");
    let run = compare(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
        &checkpoint,
    );
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );

    let report: serde_json::Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(report["selectionMetric"], "validationNdcgAt5");
    assert_eq!(report["candidates"].as_array().unwrap().len(), 10);
    assert_eq!(report["trainingExamples"], 6);
    assert_eq!(report["trainingRepositories"], 2);
    assert_eq!(report["finalTestRetrieval"]["evaluatedQueries"], 2);
    assert!(report["finalTestRetrievalDelta"]["ndcgAtK"].is_number());

    let selected = report["selectedObjective"].as_str().unwrap();
    assert!(selected == "pointwise" || selected == "pairwise");
    let epochs = report["selectedEpochs"].as_u64().unwrap();
    assert!([10, 30, 60, 100, 150].contains(&epochs));

    let contents = fs::read(&checkpoint).unwrap();
    let model: RankerModel = serde_json::from_slice(&contents).unwrap();
    model.validate().unwrap();
    assert_eq!(
        model.format,
        if selected == "pointwise" {
            MODEL_FORMAT
        } else {
            PAIRWISE_MODEL_FORMAT
        }
    );
    assert_eq!(
        report["trainingDatasetSha256"],
        model.training_dataset_sha256.as_deref().unwrap()
    );

    let audit = Command::new(env!("CARGO_BIN_EXE_reproduce-repodna-ranker"))
        .arg(&checkpoint)
        .arg(fixture("train.demo.json"))
        .arg(epochs.to_string())
        .output()
        .expect("reproduce selected model");
    assert!(
        audit.status.success(),
        "{}",
        String::from_utf8_lossy(&audit.stderr)
    );
    let reproduced: serde_json::Value = serde_json::from_slice(&audit.stdout).unwrap();
    assert_eq!(reproduced["reproduced"], true);
    assert!(!String::from_utf8_lossy(&run.stdout).contains("synthetic-gamma"));

    // The CLI never overwrites an existing checkpoint.
    let duplicate = compare(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
        &checkpoint,
    );
    assert!(!duplicate.status.success());
    assert!(duplicate.stdout.is_empty());
    assert_eq!(contents, fs::read(&checkpoint).unwrap());
}

#[test]
fn final_test_label_changes_cannot_select_different_objective_or_epochs() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.json");
    let second = dir.path().join("second.json");
    let original = compare(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
        &first,
    );
    assert!(original.status.success());

    let mut test: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("test.demo.json")).unwrap()).unwrap();
    // Change held-out labels without removing every positive judgment.
    test[0]["relevant"] = true.into();
    test[1]["relevant"] = false.into();
    let altered = dir.path().join("other-test.json");
    fs::write(&altered, serde_json::to_vec(&test).unwrap()).unwrap();

    let changed = compare(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &altered,
        &second,
    );
    assert!(
        changed.status.success(),
        "{}",
        String::from_utf8_lossy(&changed.stderr)
    );

    let before: serde_json::Value = serde_json::from_slice(&original.stdout).unwrap();
    let after: serde_json::Value = serde_json::from_slice(&changed.stdout).unwrap();
    assert_eq!(before["selectedObjective"], after["selectedObjective"]);
    assert_eq!(before["selectedEpochs"], after["selectedEpochs"]);
    assert_eq!(before["candidates"], after["candidates"]);
    assert_eq!(fs::read(first).unwrap(), fs::read(second).unwrap());
}

#[test]
fn rejects_all_split_overlaps_and_canonical_aliases() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("not-created.json");
    for (train, valid, test) in [
        ("train.demo.json", "train.demo.json", "test.demo.json"),
        ("train.demo.json", "holdout.demo.json", "train.demo.json"),
        ("train.demo.json", "holdout.demo.json", "holdout.demo.json"),
    ] {
        let result = compare(&fixture(train), &fixture(valid), &fixture(test), &out);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(!out.exists());
    }

    let mut validation: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("holdout.demo.json")).unwrap()).unwrap();
    validation[0]["repository"] = " SYNTHETIC-ALPHA ".into();
    let alias = dir.path().join("aliased.json");
    fs::write(&alias, serde_json::to_vec(&validation).unwrap()).unwrap();
    let invalid = compare(
        &fixture("train.demo.json"),
        &alias,
        &fixture("test.demo.json"),
        &out,
    );
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("repository leakage"));
    assert!(!out.exists());
}

#[test]
fn refuses_unanswerable_validation_and_oversized_training_file() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("not-created.json");
    let mut validation: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("holdout.demo.json")).unwrap()).unwrap();
    for sample in &mut validation {
        sample["relevant"] = false.into();
    }
    let val_path = dir.path().join("unanswerable.json");
    fs::write(&val_path, serde_json::to_vec(&validation).unwrap()).unwrap();
    let result = compare(
        &fixture("train.demo.json"),
        &val_path,
        &fixture("test.demo.json"),
        &out,
    );
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("validation has no answerable questions")
    );

    let too_large = dir.path().join("too-large.json");
    fs::write(&too_large, vec![b' '; 8 * 1024 * 1024 + 1]).unwrap();
    let error = compare(
        &too_large,
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
        &out,
    );
    assert!(!error.status.success());
    assert!(error.stdout.is_empty());
    assert!(String::from_utf8_lossy(&error.stderr).contains("8 MiB"));
    assert!(!out.exists());
}
