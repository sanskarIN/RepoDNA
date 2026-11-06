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
fn rejects_nonregular_frozen_model_inputs_without_partial_report() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let output = evaluate(
        dir.path(),
        &fixture("train.demo.json"),
        &fixture("holdout.demo.json"),
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("regular file"));
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
        assert_eq!(
            training_report[key], evaluation_report[key],
            "mismatch: {key}"
        );
    }
    // The synthetic holdout has one repository, so query-weighted and
    // repository-balanced metrics coincide. No repository names are exposed.
    assert_eq!(
        evaluation_report["repositoryBalancedRetrieval"]["repositories"],
        1
    );
    assert_eq!(
        evaluation_report["repositoryBalancedRetrieval"]["evaluatedRepositories"],
        1
    );
    assert_eq!(
        evaluation_report["repositoryBalancedRetrieval"]["ndcgAtK"],
        evaluation_report["retrieval"]["ndcgAtK"]
    );
    assert_eq!(
        evaluation_report["repositoryBalancedRetrieval"]["originalOrderNdcgAtK"],
        evaluation_report["originalOrderBaseline"]["ndcgAtK"]
    );
    assert_eq!(evaluation_report["trainingRepositories"], 2);
    assert_eq!(evaluation_report["trainingExamples"], 6);
    assert_eq!(evaluation_report["trainingFingerprintVerified"], true);
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

    let mut model: serde_json::Value =
        serde_json::from_slice(&fs::read(&checkpoint).unwrap()).unwrap();
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

#[test]
fn frozen_evaluation_rejects_modified_training_labels_with_unchanged_counts() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("ranker.json");
    assert!(train(&checkpoint).status.success());
    let mut records: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("train.demo.json")).unwrap()).unwrap();
    records[0]["relevant"] = false.into();
    let changed = dir.path().join("changed-labels.json");
    fs::write(&changed, serde_json::to_vec(&records).unwrap()).unwrap();

    let output = evaluate(&checkpoint, &changed, &fixture("holdout.demo.json"));
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("fingerprint mismatch"));
}

#[test]
fn frozen_evaluation_accepts_reordered_training_labels_and_marks_legacy_unverified() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("ranker.json");
    assert!(train(&checkpoint).status.success());
    let mut records: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("train.demo.json")).unwrap()).unwrap();
    records.reverse();
    let reversed = dir.path().join("reversed.json");
    fs::write(&reversed, serde_json::to_vec_pretty(&records).unwrap()).unwrap();

    let checked = evaluate(&checkpoint, &reversed, &fixture("holdout.demo.json"));
    assert!(checked.status.success());
    let report: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(report["trainingFingerprintVerified"], true);

    let mut old: serde_json::Value =
        serde_json::from_slice(&fs::read(&checkpoint).unwrap()).unwrap();
    old.as_object_mut()
        .unwrap()
        .remove("training_dataset_sha256");
    let legacy = dir.path().join("legacy.json");
    fs::write(&legacy, serde_json::to_vec(&old).unwrap()).unwrap();
    let legacy_output = evaluate(&legacy, &reversed, &fixture("holdout.demo.json"));
    assert!(legacy_output.status.success());
    let legacy_report: serde_json::Value = serde_json::from_slice(&legacy_output.stdout).unwrap();
    assert_eq!(legacy_report["trainingFingerprintVerified"], false);
}

#[test]
fn frozen_pairwise_evaluation_reports_ranking_without_false_probability_metrics() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("pairwise.json");
    let trained = Command::new(env!("CARGO_BIN_EXE_train-repodna-pairwise"))
        .arg(fixture("train.demo.json"))
        .arg(fixture("holdout.demo.json"))
        .arg(&checkpoint)
        .output()
        .expect("launch pairwise trainer");
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
    let report: serde_json::Value = serde_json::from_slice(&evaluated.stdout).unwrap();
    assert_eq!(report["modelFormat"], "repodna-evidence-pairwise/1");
    assert_eq!(report["scoreSemantics"], "relative-ranking-only");
    assert_eq!(report["trainingFingerprintVerified"], true);
    assert_eq!(report["retrieval"]["evaluatedQueries"], 2);
    assert_eq!(report["pairOrdering"]["totalPairs"], 2);
    assert_eq!(report["repositoryBalancedRetrieval"]["repositories"], 1);
    assert_eq!(
        report["repositoryBalancedRetrieval"]["ndcgAtK"],
        report["retrieval"]["ndcgAtK"]
    );
    assert!(report["retrievalDelta"]["ndcgAtK"].is_number());
    // It is mathematically incorrect to treat a pairwise score as an
    // absolute relevance probability or claim-confidence threshold.
    for absent in [
        "holdout",
        "holdoutAccuracy",
        "relevanceScoreDiagnostics",
        "answerability",
        "answerabilitySweep",
        "answerableDetectionRate",
        "unanswerableAbstentionRate",
    ] {
        assert!(report.get(absent).is_none(), "unexpected field: {absent}");
    }
    assert_eq!(before, fs::read(&checkpoint).unwrap());
    let output = String::from_utf8(evaluated.stdout).unwrap();
    assert!(!output.contains("synthetic-alpha"));
    assert!(!output.contains("\"relevant\":"));
}

#[test]
fn frozen_pairwise_evaluation_checks_original_training_fingerprint() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("pairwise.json");
    let trained = Command::new(env!("CARGO_BIN_EXE_train-repodna-pairwise"))
        .arg(fixture("train.demo.json"))
        .arg(fixture("holdout.demo.json"))
        .arg(&checkpoint)
        .output()
        .unwrap();
    assert!(trained.status.success());
    let mut records: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(fixture("train.demo.json")).unwrap()).unwrap();
    records[0]["relevant"] = false.into();
    let modified = dir.path().join("different-training.json");
    fs::write(&modified, serde_json::to_vec(&records).unwrap()).unwrap();
    let result = evaluate(&checkpoint, &modified, &fixture("holdout.demo.json"));
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("fingerprint mismatch"));
}

fn task_manifest() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "repository": "synthetic-gamma",
            "query": "What build commands exist?",
            "task": "build_commands"
        }),
        serde_json::json!({
            "repository": "synthetic-gamma",
            "query": "What dependencies are installed?",
            "task": "dependencies"
        }),
        serde_json::json!({
            "repository": "synthetic-delta",
            "query": "Which build command is available?",
            "task": "build_commands"
        }),
        serde_json::json!({
            "repository": "synthetic-delta",
            "query": "Which dependency is locked?",
            "task": "dependencies"
        }),
        serde_json::json!({
            "repository": "synthetic-epsilon",
            "query": "Where is the deployment secret?",
            "task": "security"
        }),
    ]
}

fn task_evaluate(model: &Path, training: &Path, test: &Path, manifest: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_evaluate-repodna-ranker"))
        .arg(model)
        .arg(training)
        .arg(test)
        .arg(manifest)
        .output()
        .expect("run frozen task evaluation")
}

#[test]
fn reports_task_stratified_frozen_metrics_for_pointwise_and_pairwise_checkpoints() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("tasks.json");
    let mut tasks = task_manifest();
    tasks.retain(|entry| entry["repository"] != "synthetic-gamma");
    tasks.reverse();
    tasks[0]["repository"] = " SYNTHETIC-EPSILON ".into();
    fs::write(&manifest, serde_json::to_vec(&tasks).unwrap()).unwrap();

    for objective in ["train-repodna-ranker", "train-repodna-pairwise"] {
        let checkpoint = dir.path().join(format!("{objective}.json"));
        let trained = Command::new(match objective {
            "train-repodna-ranker" => env!("CARGO_BIN_EXE_train-repodna-ranker"),
            _ => env!("CARGO_BIN_EXE_train-repodna-pairwise"),
        })
        .arg(fixture("train.demo.json"))
        .arg(fixture("holdout.demo.json"))
        .arg(&checkpoint)
        .output()
        .unwrap();
        assert!(
            trained.status.success(),
            "trainer failed: {}",
            String::from_utf8_lossy(&trained.stderr)
        );
        let before = fs::read(&checkpoint).unwrap();
        let evaluated = task_evaluate(
            &checkpoint,
            &fixture("train.demo.json"),
            &fixture("test.demo.json"),
            &manifest,
        );
        assert!(
            evaluated.status.success(),
            "evaluator failed: {}",
            String::from_utf8_lossy(&evaluated.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&evaluated.stdout).unwrap();
        let stratified = &result["taskStratifiedRetrieval"];
        assert_eq!(stratified["schema"], "repodna-frozen-task-retrieval/1");
        assert_eq!(stratified["manifestQuestions"], 3);
        assert_eq!(stratified["tasks"]["build_commands"]["questions"], 1);
        assert_eq!(
            stratified["tasks"]["build_commands"]["answerableQuestions"],
            1
        );
        assert!(stratified["tasks"]["build_commands"]["retrieval"]["ndcgAtK"].is_number());
        assert!(stratified["tasks"]["dependencies"]["retrievalDelta"]["ndcgAtK"].is_number());
        assert_eq!(stratified["tasks"]["security"]["questions"], 1);
        assert_eq!(stratified["tasks"]["security"]["unanswerableQuestions"], 1);
        assert!(stratified["tasks"]["security"]["retrieval"].is_null());
        assert!(stratified["tasks"]["architecture"]["retrieval"].is_null());
        assert_eq!(before, fs::read(&checkpoint).unwrap());

        let stdout = String::from_utf8(evaluated.stdout).unwrap();
        for secret in [
            "synthetic-delta",
            "synthetic-epsilon",
            "Where is the deployment secret?",
            "Cargo.lock",
        ] {
            assert!(!stdout.contains(secret), "private input leaked");
        }
        if objective == "train-repodna-pairwise" {
            assert_eq!(result["scoreSemantics"], "relative-ranking-only");
            assert!(result.get("answerability").is_none());
        }
    }
}

#[test]
fn frozen_task_metrics_reject_incomplete_conflicting_and_unknown_manifests() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("model.json");
    assert!(train(&checkpoint).status.success());
    let path = dir.path().join("tasks.json");
    let test = fixture("test.demo.json");
    let training = fixture("train.demo.json");

    let mut cases = Vec::new();
    let mut missing = task_manifest();
    missing.retain(|entry| entry["repository"] != "synthetic-gamma");
    missing.pop();
    cases.push(missing);

    let mut duplicate = task_manifest();
    duplicate.retain(|entry| entry["repository"] != "synthetic-gamma");
    duplicate.push(serde_json::json!({
        "repository": " SYNTHETIC-DELTA ",
        "query": "Which dependency is locked?",
        "task": "other"
    }));
    cases.push(duplicate);

    let mut unknown = task_manifest();
    unknown.retain(|entry| entry["repository"] != "synthetic-gamma");
    unknown[0]["task"] = "guess_from_title".into();
    cases.push(unknown);

    let mut extra = task_manifest();
    extra.push(serde_json::json!({
        "repository": "orphan",
        "query": "Unknown?",
        "task": "other"
    }));
    cases.push(extra);

    for (index, case) in cases.iter().enumerate() {
        fs::write(&path, serde_json::to_vec(case).unwrap()).unwrap();
        let result = task_evaluate(&checkpoint, &training, &test, &path);
        assert!(!result.status.success(), "invalid case {index} accepted");
        assert!(result.stdout.is_empty(), "partial JSON for case {index}");
    }
}
