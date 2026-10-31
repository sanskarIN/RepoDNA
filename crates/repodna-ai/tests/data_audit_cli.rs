//! Verify privacy-preserving dataset quality checks at the CLI boundary.
//! Synthetic fixtures only exercise the audit, never prove label quality.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/native-ai")
        .join(name)
}

fn audit(train: &Path, validation: &Path, test: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_audit-repodna-labels"))
        .arg(train)
        .arg(validation)
        .arg(test)
        .output()
        .expect("run native label audit")
}

#[test]
fn audits_independent_datasets_with_aggregates_only() {
    let output = audit(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["structuralChecksPassed"], true);
    assert_eq!(report["training"]["examples"], 6);
    assert_eq!(report["training"]["repositories"], 2);
    assert_eq!(report["training"]["positiveLabels"], 3);
    assert_eq!(report["training"]["negativeLabels"], 3);
    assert_eq!(report["training"]["questionsWithBothLabels"], 3);
    assert_eq!(report["validation"]["repositories"], 1);
    assert_eq!(report["finalTest"]["repositories"], 2);
    assert_eq!(report["finalTest"]["questionsWithoutRelevantEvidence"], 1);
    assert_eq!(
        report["matchingEvidenceSignaturesAcrossSplits"]["trainingValidation"],
        0
    );
    assert_eq!(report["training"]["labelFingerprintSha256"].as_str().unwrap().len(), 64);
    assert!(report["warnings"].as_array().unwrap().len() >= 1);
    let text = String::from_utf8(output.stdout).unwrap();
    for secret in [
        "synthetic-alpha",
        "synthetic-delta",
        "Which dependencies are locked?",
        "Cargo.lock",
        "Recorded contributor activity",
    ] {
        assert!(!text.contains(secret), "raw label leaked into audit output");
    }
}

#[test]
fn detects_exact_question_evidence_reuse_with_different_repository_and_id() {
    let dir = tempfile::tempdir().unwrap();
    let mut test: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("test.demo.json")).unwrap()).unwrap();
    let train: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("train.demo.json")).unwrap()).unwrap();
    // The repository identifier stays distinct, and evidence IDs need not
    // match. Content matching must still flag the reused fact.
    test[0]["query"] = train[0]["query"].clone();
    test[0]["evidence"]["kind"] = train[0]["evidence"]["kind"].clone();
    test[0]["evidence"]["reference"] = train[0]["evidence"]["reference"].clone();
    test[0]["evidence"]["detail"] = train[0]["evidence"]["detail"].clone();
    test[0]["evidence"]["id"] = "new-evidence-id".into();
    let suspect = dir.path().join("suspect-test.json");
    fs::write(&suspect, serde_json::to_vec(&test).unwrap()).unwrap();

    let result = audit(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &suspect,
    );
    assert!(
        result.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        report["matchingEvidenceSignaturesAcrossSplits"]["trainingFinalTest"],
        1
    );
    assert!(report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("across different splits")));
}

#[test]
fn rejects_leakage_and_invalid_training_classes() {
    let dir = tempfile::tempdir().unwrap();
    let alias = dir.path().join("alias.json");
    let mut test: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("test.demo.json")).unwrap()).unwrap();
    test[0]["repository"] = " SYNTHETIC-ALPHA ".into();
    fs::write(&alias, serde_json::to_vec(&test).unwrap()).unwrap();
    let leak = audit(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &alias,
    );
    assert!(!leak.status.success());
    assert!(leak.stdout.is_empty());
    assert!(String::from_utf8_lossy(&leak.stderr).contains("repository leakage"));

    let train_path = dir.path().join("no-pairs.json");
    let mut training: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("train.demo.json")).unwrap()).unwrap();
    for item in &mut training {
        item["relevant"] = false.into();
    }
    fs::write(&train_path, serde_json::to_vec(&training).unwrap()).unwrap();
    let invalid = audit(
        &train_path,
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
    );
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("both labels"));
}

#[test]
fn rejects_oversized_files_and_repeated_evidence_ids() {
    let dir = tempfile::tempdir().unwrap();
    let large = dir.path().join("oversized.json");
    fs::write(&large, vec![b' '; 8 * 1024 * 1024 + 1]).unwrap();
    let out = audit(
        &large,
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
    );
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("8 MiB"));

    let repeated = dir.path().join("duplicate.json");
    let mut items: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("train.demo.json")).unwrap()).unwrap();
    items.push(items[0].clone());
    fs::write(&repeated, serde_json::to_vec(&items).unwrap()).unwrap();
    let out = audit(
        &repeated,
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
    );
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("duplicate evidence ID"));
}
