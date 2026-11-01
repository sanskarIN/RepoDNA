//! Offline adjudication audit. Fixtures test mechanics, not human review.
//! No private source metadata or reviewer identities may be printed.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture() -> Vec<serde_json::Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/native-ai/train.demo.json");
    serde_json::from_slice(&fs::read(path).expect("read fixture")).expect("parse fixture")
}

fn write(path: &Path, labels: &[serde_json::Value]) {
    fs::write(path, serde_json::to_vec(labels).expect("serialize")).expect("write");
}

fn audit(a: &Path, b: &Path, final_path: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_audit-repodna-reviewers"))
        .arg(a)
        .arg(b)
        .arg(final_path)
        .output()
        .expect("run adjudication check")
}

#[test]
fn validates_two_independently_resolved_disputes_in_any_record_order() {
    let dir = tempfile::tempdir().expect("temp");
    let a_path = dir.path().join("review-a.json");
    let b_path = dir.path().join("review-b.json");
    let final_path = dir.path().join("final.json");
    let a = fixture();
    let mut b = a.clone();
    b[0]["relevant"] = false.into();
    b[1]["relevant"] = true.into();
    let mut final_labels = a.clone();
    final_labels[0]["relevant"] = false.into();
    b.reverse();
    final_labels.reverse();
    write(&a_path, &a);
    write(&b_path, &b);
    write(&final_path, &final_labels);

    let run = audit(&a_path, &b_path, &final_path);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&run.stdout).expect("report");
    assert_eq!(report["pairedAnnotations"], 6);
    assert_eq!(report["agreements"], 4);
    assert_eq!(report["disagreements"], 2);
    assert_eq!(
        report["adjudication"]["schema"],
        "repodna-reviewer-adjudication/1"
    );
    assert_eq!(report["adjudication"]["structuralChecksPassed"], true);
    assert_eq!(report["adjudication"]["agreedLabelsPreserved"], 4);
    assert_eq!(report["adjudication"]["disagreementsAdjudicated"], 2);
    assert_eq!(report["adjudication"]["adjudicatedInFavorOfReviewerA"], 1);
    assert_eq!(report["adjudication"]["adjudicatedInFavorOfReviewerB"], 1);
    assert_eq!(report["adjudication"]["finalPositiveLabels"], 2);
    assert_eq!(report["adjudication"]["finalNegativeLabels"], 4);

    let output = String::from_utf8(run.stdout).expect("UTF-8");
    for private in [
        "synthetic-alpha",
        "Which dependencies",
        "Cargo.lock",
        "Recorded contributor activity",
        "review-a.json",
        "review-b.json",
    ] {
        assert!(!output.contains(private), "private data in stdout");
    }
}

#[test]
fn rejects_silent_override_of_two_agreeing_reviews() {
    let dir = tempfile::tempdir().expect("temp");
    let a_path = dir.path().join("a.json");
    let b_path = dir.path().join("b.json");
    let final_path = dir.path().join("final.json");
    let a = fixture();
    let mut b = a.clone();
    b[0]["relevant"] = false.into();
    let mut final_labels = a.clone();
    final_labels[2]["relevant"] = false.into();
    write(&a_path, &a);
    write(&b_path, &b);
    write(&final_path, &final_labels);
    let run = audit(&a_path, &b_path, &final_path);
    assert!(!run.status.success());
    assert!(run.stdout.is_empty());
    assert!(String::from_utf8_lossy(&run.stderr).contains("both reviewers agreed"));
}

#[test]
fn rejects_unreviewed_missing_and_changed_final_records_without_stdout() {
    let dir = tempfile::tempdir().expect("temp");
    let a_path = dir.path().join("a.json");
    let b_path = dir.path().join("b.json");
    let final_path = dir.path().join("final.json");
    let a = fixture();
    let b = a.clone();
    write(&a_path, &a);
    write(&b_path, &b);

    let mut incomplete = a.clone();
    incomplete[0]["relevant"] = serde_json::Value::Null;
    write(&final_path, &incomplete);
    let run = audit(&a_path, &b_path, &final_path);
    assert!(!run.status.success());
    assert!(run.stdout.is_empty());
    assert!(String::from_utf8_lossy(&run.stderr).contains("no relevance label"));

    let mut missing = a.clone();
    missing.pop();
    write(&final_path, &missing);
    let run = audit(&a_path, &b_path, &final_path);
    assert!(!run.status.success());
    assert!(run.stdout.is_empty());
    assert!(String::from_utf8_lossy(&run.stderr).contains("different evidence records"));

    let mut changed = a.clone();
    changed[0]["evidence"]["reference"] = "tampered".into();
    write(&final_path, &changed);
    let run = audit(&a_path, &b_path, &final_path);
    assert!(!run.status.success());
    assert!(run.stdout.is_empty());
    assert!(String::from_utf8_lossy(&run.stderr).contains("different evidence records"));
}

#[test]
fn rejects_duplicate_final_ids_without_printing_evidence() {
    let dir = tempfile::tempdir().expect("temp");
    let a_path = dir.path().join("a.json");
    let b_path = dir.path().join("b.json");
    let final_path = dir.path().join("final.json");
    let a = fixture();
    write(&a_path, &a);
    write(&b_path, &a);
    let mut duplicated = a.clone();
    duplicated.push(a[0].clone());
    write(&final_path, &duplicated);
    let run = audit(&a_path, &b_path, &final_path);
    assert!(!run.status.success());
    assert!(run.stdout.is_empty());
    assert!(String::from_utf8_lossy(&run.stderr).contains("duplicate evidence ID"));
}

#[test]
fn supports_fully_agreed_final_reviews_without_requiring_disputes() {
    let dir = tempfile::tempdir().expect("temp");
    let a_path = dir.path().join("a.json");
    let b_path = dir.path().join("b.json");
    let final_path = dir.path().join("final.json");
    let a = fixture();
    write(&a_path, &a);
    write(&b_path, &a);
    write(&final_path, &a);
    let run = audit(&a_path, &b_path, &final_path);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let report: serde_json::Value = serde_json::from_slice(&run.stdout).expect("report");
    assert_eq!(report["adjudication"]["agreedLabelsPreserved"], 6);
    assert_eq!(report["adjudication"]["disagreementsAdjudicated"], 0);
    assert_eq!(report["adjudication"]["finalPositiveLabels"], 3);
}
