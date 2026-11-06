//! Integration tests for offline, fully paired annotation agreement.
//! Synthetic labels verify statistics and privacy, not reviewer expertise.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/native-ai/train.demo.json")
}

fn labels() -> Vec<serde_json::Value> {
    serde_json::from_slice(&fs::read(fixture()).expect("read synthetic labels"))
        .expect("parse synthetic labels")
}

fn review(path: &Path, labels: &[serde_json::Value]) {
    fs::write(path, serde_json::to_vec(labels).expect("JSON")).expect("write labels");
}

fn audit(a: &Path, b: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_audit-repodna-reviewers"))
        .arg(a)
        .arg(b)
        .output()
        .expect("run reviewer agreement audit")
}

#[test]
fn rejects_nonregular_reviewer_inputs_without_partial_output() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let output = audit(dir.path(), &fixture());
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("regular file"));
}

#[test]
fn reports_perfect_order_independent_agreement_without_leaking_evidence() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let a_path = dir.path().join("a.json");
    let b_path = dir.path().join("b.json");
    let a = labels();
    let mut b = a.clone();
    b.reverse();
    review(&a_path, &a);
    review(&b_path, &b);
    let result = audit(&a_path, &b_path);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&result.stdout).expect("aggregate output");
    assert_eq!(report["schema"], "repodna-reviewer-agreement/1");
    assert_eq!(report["pairedAnnotations"], 6);
    assert_eq!(report["agreements"], 6);
    assert_eq!(report["disagreements"], 0);
    assert_eq!(report["bothRelevant"], 3);
    assert_eq!(report["bothIrrelevant"], 3);
    assert_eq!(report["observedAgreement"], 1.0);
    assert_eq!(report["cohensKappa"], 1.0);
    let published = String::from_utf8(result.stdout).expect("UTF-8 JSON");
    for private in [
        "synthetic-alpha",
        "Which dependencies are locked?",
        "Cargo.lock",
        "Recorded contributor activity",
    ] {
        assert!(!published.contains(private), "source data leaked");
    }
}

#[test]
fn reports_disagreements_and_expected_chance_correction() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let a_path = dir.path().join("a.json");
    let b_path = dir.path().join("b.json");
    let a = labels();
    let mut b = a.clone();
    b[0]["relevant"] = false.into();
    review(&a_path, &a);
    review(&b_path, &b);
    let result = audit(&a_path, &b_path);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&result.stdout).expect("aggregate output");
    assert_eq!(report["agreements"], 5);
    assert_eq!(report["disagreements"], 1);
    assert_eq!(report["onlyReviewerARelevant"], 1);
    assert_eq!(report["onlyReviewerBRelevant"], 0);
    assert!((report["observedAgreement"].as_f64().unwrap() - 5.0 / 6.0).abs() < 1e-12);
    assert!((report["expectedChanceAgreement"].as_f64().unwrap() - 0.5).abs() < 1e-12);
    assert!((report["cohensKappa"].as_f64().unwrap() - 2.0 / 3.0).abs() < 1e-12);
}

#[test]
fn reports_undefined_kappa_for_unanimous_single_class() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let a_path = dir.path().join("a.json");
    let b_path = dir.path().join("b.json");
    let mut a = labels();
    for item in &mut a {
        item["relevant"] = true.into();
    }
    review(&a_path, &a);
    review(&b_path, &a);
    let result = audit(&a_path, &b_path);
    assert!(result.status.success());
    let report: serde_json::Value =
        serde_json::from_slice(&result.stdout).expect("aggregate output");
    assert_eq!(report["observedAgreement"], 1.0);
    assert_eq!(report["expectedChanceAgreement"], 1.0);
    assert!(report["cohensKappa"].is_null());
}

#[test]
fn rejects_changed_evidence_and_missing_labels_without_partial_stdout() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let a_path = dir.path().join("a.json");
    let b_path = dir.path().join("b.json");
    let a = labels();
    let mut b = a.clone();
    b[0]["evidence"]["detail"] = "modified annotation".into();
    review(&a_path, &a);
    review(&b_path, &b);
    let mismatch = audit(&a_path, &b_path);
    assert!(!mismatch.status.success());
    assert!(mismatch.stdout.is_empty());
    assert!(String::from_utf8_lossy(&mismatch.stderr).contains("different evidence records"));

    b = a.clone();
    b[0]["relevant"] = serde_json::Value::Null;
    review(&b_path, &b);
    let unreviewed = audit(&a_path, &b_path);
    assert!(!unreviewed.status.success());
    assert!(unreviewed.stdout.is_empty());
    assert!(String::from_utf8_lossy(&unreviewed.stderr).contains("no relevance label"));

    b = a.clone();
    b[0]["evidence"]["kind"] = "excerpt".into();
    review(&b_path, &b);
    let excerpt = audit(&a_path, &b_path);
    assert!(!excerpt.status.success());
    assert!(excerpt.stdout.is_empty());
}

#[test]
fn rejects_incomplete_and_duplicate_review_subsets() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let a_path = dir.path().join("a.json");
    let b_path = dir.path().join("b.json");
    let a = labels();
    let mut b = a.clone();
    b.pop();
    review(&a_path, &a);
    review(&b_path, &b);
    let incomplete = audit(&a_path, &b_path);
    assert!(!incomplete.status.success());
    assert!(incomplete.stdout.is_empty());

    b = a.clone();
    b.push(a[0].clone());
    review(&b_path, &b);
    let duplicate = audit(&a_path, &b_path);
    assert!(!duplicate.status.success());
    assert!(duplicate.stdout.is_empty());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("duplicate evidence ID"));
}
