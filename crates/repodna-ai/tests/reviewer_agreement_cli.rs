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

fn task_manifest() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "repository": " SYNTHETIC-ALPHA ",
            "query": "Which dependencies are locked?",
            "task": "dependencies"
        }),
        serde_json::json!({
            "repository": "synthetic-alpha",
            "query": "How is the project built?",
            "task": "build_commands"
        }),
        serde_json::json!({
            "repository": "synthetic-beta",
            "query": "Where are security findings?",
            "task": "security"
        }),
    ]
}

fn audit_tasks(a: &Path, b: &Path, tasks: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_audit-repodna-reviewers"))
        .arg(a)
        .arg(b)
        .arg("--tasks")
        .arg(tasks)
        .output()
        .expect("run task-stratified reviewer agreement")
}

fn audit_source(a: &Path, b: &Path, source: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_audit-repodna-reviewers"))
        .arg(a)
        .arg(b)
        .arg("--source")
        .arg(source)
        .output()
        .expect("run reviewer source coverage audit")
}

#[test]
fn source_coverage_counts_complete_partial_and_unreviewed_questions() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let first_path = dir.path().join("first.json");
    let second_path = dir.path().join("second.json");
    let source_path = dir.path().join("source.json");
    let original = labels();
    let first = original[..3].to_vec();
    let mut second = first.clone();
    second[0]["relevant"] = false.into();
    second.reverse();
    review(&first_path, &first);
    review(&second_path, &second);
    review(&source_path, &original);

    let output = audit_source(&first_path, &second_path, &source_path);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("coverage report");
    assert_eq!(report["disagreements"], 1);
    let coverage = &report["sourceCoverage"];
    assert_eq!(coverage["schema"], "repodna-double-review-coverage/1");
    assert_eq!(coverage["sourceAnnotations"], 6);
    assert_eq!(coverage["doubleReviewedAnnotations"], 3);
    assert_eq!(coverage["notDoubleReviewedAnnotations"], 3);
    assert_eq!(coverage["annotationCoverageRate"], 0.5);
    assert_eq!(coverage["sourceQuestions"], 3);
    assert_eq!(coverage["doubleReviewedQuestions"], 2);
    assert_eq!(coverage["completelyDoubleReviewedQuestions"], 1);
    assert_eq!(coverage["partiallyDoubleReviewedQuestions"], 1);
    assert_eq!(coverage["notDoubleReviewedQuestions"], 1);
    assert_eq!(coverage["sourceRepositories"], 2);
    assert_eq!(coverage["doubleReviewedRepositories"], 1);
    assert_eq!(coverage["adjudicatedLabelsMatchSourceVerified"], false);
    assert_eq!(coverage["sourceDatasetSha256"].as_str().unwrap().len(), 64);

    let published = String::from_utf8(output.stdout).expect("UTF-8");
    for private in [
        "synthetic-alpha",
        "synthetic-beta",
        "Which dependencies are locked?",
        "Cargo.lock",
        "Where are security findings?",
    ] {
        assert!(!published.contains(private), "private input leaked");
    }
}

#[test]
fn rejects_source_subset_mismatch_duplicates_and_nonregular_files() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let first_path = dir.path().join("first.json");
    let second_path = dir.path().join("second.json");
    let source_path = dir.path().join("source.json");
    let original = labels();
    let first = original[..3].to_vec();
    review(&first_path, &first);
    review(&second_path, &first);

    let mut changed_source = original.clone();
    changed_source[0]["evidence"]["detail"] = "changed evidence metadata".into();
    review(&source_path, &changed_source);
    let mismatch = audit_source(&first_path, &second_path, &source_path);
    assert!(!mismatch.status.success());
    assert!(mismatch.stdout.is_empty());
    assert!(String::from_utf8_lossy(&mismatch.stderr).contains("not present"));

    let mut duplicate = original.clone();
    duplicate.push(original[0].clone());
    review(&source_path, &duplicate);
    let rejected = audit_source(&first_path, &second_path, &source_path);
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("duplicate evidence ID"));

    let nonregular = audit_source(&first_path, &second_path, dir.path());
    assert!(!nonregular.status.success());
    assert!(nonregular.stdout.is_empty());
    assert!(String::from_utf8_lossy(&nonregular.stderr).contains("regular file"));
}

#[test]
fn finalized_review_must_match_source_labels_and_work_with_task_audit() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let first_path = dir.path().join("first.json");
    let second_path = dir.path().join("second.json");
    let finalized_path = dir.path().join("adjudicated.json");
    let source_path = dir.path().join("source.json");
    let tasks_path = dir.path().join("tasks.json");

    let original = labels();
    let first = original[..3].to_vec();
    let mut second = first.clone();
    second[0]["relevant"] = false.into();
    review(&first_path, &first);
    review(&second_path, &second);
    review(&finalized_path, &first);
    review(&source_path, &original);
    review(&tasks_path, &task_manifest()[..2]);

    let run = || {
        Command::new(env!("CARGO_BIN_EXE_audit-repodna-reviewers"))
            .arg(&first_path)
            .arg(&second_path)
            .arg(&finalized_path)
            .arg("--source")
            .arg(&source_path)
            .arg("--tasks")
            .arg(&tasks_path)
            .output()
            .expect("run all reviewer validation modes")
    };
    let success = run();
    assert!(
        success.status.success(),
        "{}",
        String::from_utf8_lossy(&success.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&success.stdout).expect("valid report");
    assert_eq!(value["adjudication"]["disagreementsAdjudicated"], 1);
    assert_eq!(
        value["sourceCoverage"]["adjudicatedLabelsMatchSourceVerified"],
        true
    );
    assert_eq!(value["taskAgreement"]["manifestQuestions"], 2);

    let mut changed = original.clone();
    changed[0]["relevant"] = false.into();
    review(&source_path, &changed);
    let mismatch = run();
    assert!(!mismatch.status.success());
    assert!(mismatch.stdout.is_empty());
    assert!(String::from_utf8_lossy(&mismatch.stderr).contains("adjudicated label differs"));

    // Without a final adjudication, source coverage verifies membership only.
    let structural = audit_source(&first_path, &second_path, &source_path);
    assert!(structural.status.success());
    let value: serde_json::Value =
        serde_json::from_slice(&structural.stdout).expect("structural report");
    assert_eq!(
        value["sourceCoverage"]["adjudicatedLabelsMatchSourceVerified"],
        false
    );
}

#[test]
fn rejects_invalid_source_argument_combinations_without_partial_output() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let first = dir.path().join("first.json");
    let second = dir.path().join("second.json");
    let source = dir.path().join("source.json");
    let original = labels();
    review(&first, &original);
    review(&second, &original);
    review(&source, &original);
    for arguments in [
        vec!["--source", "--source"],
        vec!["--source", "source.json", "--source", "source.json"],
        vec!["--unknown"],
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_audit-repodna-reviewers"));
        command.arg(&first).arg(&second);
        for argument in arguments {
            command.arg(argument);
        }
        let output = command.output().expect("launch invalid invocation");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn reports_curated_task_agreement_without_disclosing_individual_reviews() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let first = dir.path().join("first.json");
    let second = dir.path().join("second.json");
    let tasks = dir.path().join("tasks.json");
    let a = labels();
    let mut b = a.clone();
    b[0]["relevant"] = false.into();
    b.reverse();
    review(&first, &a);
    review(&second, &b);
    review(&tasks, &task_manifest());

    let output = audit_tasks(&first, &second, &tasks);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("task agreement JSON");
    assert_eq!(report["pairedAnnotations"], 6);
    assert_eq!(report["disagreements"], 1);
    assert_eq!(
        report["taskAgreement"]["schema"],
        "repodna-reviewer-task-agreement/1"
    );
    assert_eq!(report["taskAgreement"]["manifestQuestions"], 3);
    assert_eq!(
        report["taskAgreement"]["summaries"]["dependencies"]["pairedAnnotations"],
        2
    );
    assert_eq!(
        report["taskAgreement"]["summaries"]["dependencies"]["disagreements"],
        1
    );
    assert_eq!(
        report["taskAgreement"]["summaries"]["build_commands"]["disagreements"],
        0
    );
    assert_eq!(
        report["taskAgreement"]["summaries"]["security"]["agreements"],
        2
    );
    let public = String::from_utf8(output.stdout).expect("UTF-8");
    for private in [
        "synthetic-alpha",
        "synthetic-beta",
        "Which dependencies",
        "Cargo.lock",
        "Where are security findings?",
    ] {
        assert!(!public.contains(private), "raw review metadata was emitted");
    }
}

#[test]
fn task_manifest_fails_closed_on_missing_duplicate_and_unknown_categories() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let first = dir.path().join("first.json");
    let second = dir.path().join("second.json");
    let tasks = dir.path().join("tasks.json");
    let a = labels();
    review(&first, &a);
    review(&second, &a);

    let mut entries = task_manifest();
    entries.pop();
    review(&tasks, &entries);
    let missing = audit_tasks(&first, &second, &tasks);
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("missing"));

    let mut entries = task_manifest();
    entries.push(entries[0].clone());
    review(&tasks, &entries);
    let duplicate = audit_tasks(&first, &second, &tasks);
    assert!(!duplicate.status.success());
    assert!(duplicate.stdout.is_empty());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("duplicate"));

    let mut entries = task_manifest();
    entries[0]["task"] = "uncertain".into();
    review(&tasks, &entries);
    let unknown = audit_tasks(&first, &second, &tasks);
    assert!(!unknown.status.success());
    assert!(unknown.stdout.is_empty());

    let nonregular = audit_tasks(&first, &second, dir.path());
    assert!(!nonregular.status.success());
    assert!(nonregular.stdout.is_empty());
    assert!(String::from_utf8_lossy(&nonregular.stderr).contains("regular file"));
}

#[test]
fn supports_simultaneous_adjudication_and_task_aggregation() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let first = dir.path().join("first.json");
    let second = dir.path().join("second.json");
    let finalized = dir.path().join("final.json");
    let tasks = dir.path().join("tasks.json");
    let a = labels();
    review(&first, &a);
    review(&second, &a);
    review(&finalized, &a);
    review(&tasks, &task_manifest());
    let output = Command::new(env!("CARGO_BIN_EXE_audit-repodna-reviewers"))
        .arg(&first)
        .arg(&second)
        .arg(&finalized)
        .arg("--tasks")
        .arg(&tasks)
        .output()
        .expect("run adjudication and task audit");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(report["adjudication"]["structuralChecksPassed"], true);
    assert_eq!(report["taskAgreement"]["manifestQuestions"], 3);
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
