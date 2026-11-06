//! Synthetic datasets verify the offline curated task coverage audit only.
//! Task categories in real benchmarks must be assigned by human reviewers.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/native-ai")
        .join(name)
}

fn tasks() -> Vec<serde_json::Value> {
    [
        ("synthetic-alpha", "Which dependencies are locked?", "dependencies"),
        ("synthetic-alpha", "How is the project built?", "build_commands"),
        ("synthetic-beta", "Where are security findings?", "security"),
        ("synthetic-gamma", "What build commands exist?", "build_commands"),
        ("synthetic-gamma", "What dependencies are installed?", "dependencies"),
        ("synthetic-delta", "Which build command is available?", "build_commands"),
        ("synthetic-delta", "Which dependency is locked?", "dependencies"),
        ("synthetic-epsilon", "Where is the deployment secret?", "security"),
    ]
    .into_iter()
    .map(|(repository, query, task)| {
        serde_json::json!({ "repository": repository, "query": query, "task": task })
    })
    .collect()
}

fn save(path: &Path, data: &[serde_json::Value]) {
    fs::write(path, serde_json::to_vec(data).expect("json")).expect("write manifest");
}

fn run(train: &Path, valid: &Path, test: &Path, manifest: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_audit-repodna-tasks"))
        .arg(train)
        .arg(valid)
        .arg(test)
        .arg(manifest)
        .output()
        .expect("launch task auditor")
}

#[test]
fn rejects_nonregular_manifest_without_partial_aggregate_report() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let result = run(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
        dir.path(),
    );
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("regular file"));
}

#[test]
fn reports_curated_task_coverage_without_leaking_repository_or_questions() {
    let dir = tempfile::tempdir().expect("temp directory");
    let path = dir.path().join("tasks.json");
    let mut entries = tasks();
    entries.reverse();
    entries[0]["repository"] = " SYNTHETIC-EPSILON ".into();
    save(&path, &entries);
    let result = run(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
        &path,
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).expect("report");
    assert_eq!(report["schema"], "repodna-task-coverage/1");
    assert_eq!(report["taskManifestChecked"], true);
    assert_eq!(report["manifestQuestions"], 8);
    assert_eq!(report["training"]["dependencies"]["questions"], 1);
    assert_eq!(report["training"]["build_commands"]["positiveLabels"], 1);
    assert_eq!(
        report["validation"]["build_commands"]["answerableQuestions"],
        1
    );
    assert_eq!(report["finalTest"]["security"]["unanswerableQuestions"], 1);
    assert_eq!(report["finalTest"]["security"]["negativeLabels"], 2);
    assert_eq!(report["training"]["architecture"]["questions"], 0);
    assert!(!report["warnings"].as_array().unwrap().is_empty());
    let stdout = String::from_utf8(result.stdout).expect("utf8");
    for private in [
        "synthetic-alpha",
        "SYNTHETIC-EPSILON",
        "Which dependencies",
        "Cargo.lock",
        "Where is the deployment secret?",
    ] {
        assert!(
            !stdout.contains(private),
            "private input leaked into aggregate report"
        );
    }
}

#[test]
fn rejects_missing_extra_and_normalized_duplicate_manifest_entries() {
    let dir = tempfile::tempdir().expect("temp directory");
    let path = dir.path().join("tasks.json");
    let mut entries = tasks();
    entries.pop();
    save(&path, &entries);
    let missing = run(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
        &path,
    );
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("missing or unused"));

    let mut entries = tasks();
    entries.push(serde_json::json!({
        "repository": "orphan",
        "query": "Unseen question?",
        "task": "other"
    }));
    save(&path, &entries);
    let orphan = run(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
        &path,
    );
    assert!(!orphan.status.success());
    assert!(orphan.stdout.is_empty());

    let mut entries = tasks();
    entries.push(serde_json::json!({
        "repository": " SYNTHETIC-ALPHA ",
        "query": "Which dependencies are locked?",
        "task": "other"
    }));
    save(&path, &entries);
    let duplicate = run(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
        &path,
    );
    assert!(!duplicate.status.success());
    assert!(duplicate.stdout.is_empty());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("repeats"));
}

#[test]
fn rejects_unknown_task_categories_and_mutated_questions() {
    let dir = tempfile::tempdir().expect("temp directory");
    let path = dir.path().join("tasks.json");
    let mut entries = tasks();
    entries[0]["task"] = "guess_from_text".into();
    save(&path, &entries);
    let unknown = run(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
        &path,
    );
    assert!(!unknown.status.success());
    assert!(unknown.stdout.is_empty());

    let mut entries = tasks();
    entries[0]["query"] = "Changed without reviewer approval".into();
    save(&path, &entries);
    let changed = run(
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
        &path,
    );
    assert!(!changed.status.success());
    assert!(changed.stdout.is_empty());
}

#[test]
fn rejects_repository_leakage_and_duplicate_evidence() {
    let dir = tempfile::tempdir().expect("temp directory");
    let path = dir.path().join("tasks.json");
    save(&path, &tasks());
    let overlap = run(
        &fixture("train.demo.json"),
        &fixture("train.demo.json"),
        &fixture("test.demo.json"),
        &path,
    );
    assert!(!overlap.status.success());
    assert!(overlap.stdout.is_empty());

    let mut labels: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("train.demo.json")).unwrap()).unwrap();
    labels.push(labels[0].clone());
    let duplicate_path = dir.path().join("duplicate.json");
    save(&duplicate_path, &labels);
    let duplicate = run(
        &duplicate_path,
        &fixture("holdout.demo.json"),
        &fixture("test.demo.json"),
        &path,
    );
    assert!(!duplicate.status.success());
    assert!(duplicate.stdout.is_empty());
}
