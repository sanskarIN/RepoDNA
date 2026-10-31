//! Verify that saved benchmark reports are bound to unchanged local inputs.
//! Synthetic fixtures validate reproducibility mechanics, not model quality.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/native-ai")
        .join(name)
}

fn prepare(dir: &Path, manifest: Option<&Path>) -> (PathBuf, PathBuf) {
    let model = dir.join("selected.json");
    let report = dir.join("benchmark-report.json");
    let mut command = Command::new(env!("CARGO_BIN_EXE_compare-repodna-rankers"));
    command
        .arg(fixture("train.demo.json"))
        .arg(fixture("holdout.demo.json"))
        .arg(fixture("test.demo.json"))
        .arg(&model);
    if let Some(path) = manifest {
        command.arg(path);
    }
    let output = command.output().expect("execute native model selection");
    assert!(
        output.status.success(),
        "comparison failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(&report, &output.stdout).expect("write benchmark report");
    (report, model)
}

fn verify(report: &Path, model: &Path, validation: &Path, manifest: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_verify-repodna-benchmark"));
    command
        .arg(report)
        .arg(model)
        .arg(fixture("train.demo.json"))
        .arg(validation)
        .arg(fixture("test.demo.json"));
    if let Some(path) = manifest {
        command.arg(path);
    }
    command
        .output()
        .expect("execute benchmark identity verification")
}

fn verify_replay(
    report: &Path,
    model: &Path,
    validation: &Path,
    manifest: Option<&Path>,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_verify-repodna-benchmark"));
    command
        .arg(report)
        .arg(model)
        .arg(fixture("train.demo.json"))
        .arg(validation)
        .arg(fixture("test.demo.json"));
    if let Some(path) = manifest {
        command.arg(path);
    }
    command
        .arg("--replay")
        .output()
        .expect("execute full benchmark replay")
}

fn records(name: &str) -> Vec<serde_json::Value> {
    serde_json::from_slice(&fs::read(fixture(name)).expect("read fixture")).expect("parse fixture")
}

#[test]
fn verifies_saved_model_and_all_label_splits_without_manifest() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let (report, model) = prepare(dir.path(), None);
    let verified = verify(&report, &model, &fixture("holdout.demo.json"), None);
    assert!(
        verified.status.success(),
        "{}",
        String::from_utf8_lossy(&verified.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&verified.stdout).expect("valid verifier output");
    assert_eq!(result["verified"], true);
    assert_eq!(result["schema"], "repodna-benchmark-provenance/1");
    assert_eq!(result["familyManifestChecked"], false);
    assert!(result["familyManifestSha256"].is_null());
    assert_eq!(
        result["checkpointSha256"]
            .as_str()
            .expect("checkpoint digest")
            .len(),
        64
    );

    let output = String::from_utf8(verified.stdout).expect("UTF-8 report");
    assert!(!output.contains("synthetic-alpha"));
    assert!(!output.contains("Cargo.lock"));
}

#[test]
fn verifies_reordered_family_records_but_rejects_changed_provenance() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let family = fixture("families.demo.json");
    let (report, model) = prepare(dir.path(), Some(&family));
    let first = verify(
        &report,
        &model,
        &fixture("holdout.demo.json"),
        Some(&family),
    );
    assert!(first.status.success());

    let mut entries = records("families.demo.json");
    entries.reverse();
    entries[0]["family"] = entries[0]["family"]
        .as_str()
        .expect("family identifier")
        .to_uppercase()
        .into();
    let permuted = dir.path().join("reordered-families.json");
    fs::write(
        &permuted,
        serde_json::to_vec(&entries).expect("serializable"),
    )
    .expect("write manifest");
    let reordered = verify(
        &report,
        &model,
        &fixture("holdout.demo.json"),
        Some(&permuted),
    );
    assert!(
        reordered.status.success(),
        "{}",
        String::from_utf8_lossy(&reordered.stderr)
    );

    entries[0]["revision"] = "b".repeat(40).into();
    let changed = dir.path().join("modified-families.json");
    fs::write(
        &changed,
        serde_json::to_vec(&entries).expect("serializable"),
    )
    .expect("write changed manifest");
    let rejected = verify(
        &report,
        &model,
        &fixture("holdout.demo.json"),
        Some(&changed),
    );
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("familyManifestSha256"));

    // A report bound to family provenance must never verify without it.
    let absent = verify(&report, &model, &fixture("holdout.demo.json"), None);
    assert!(!absent.status.success());
    assert!(absent.stdout.is_empty());
}

#[test]
fn detects_tampered_validation_labels_and_checkpoint_weights() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let (report, model) = prepare(dir.path(), None);
    let mut validation = records("holdout.demo.json");
    let original = validation[0]["relevant"].as_bool().expect("label bool");
    validation[0]["relevant"] = (!original).into();
    let altered = dir.path().join("altered-validation.json");
    fs::write(
        &altered,
        serde_json::to_vec(&validation).expect("serializable"),
    )
    .expect("write altered labels");
    let invalid = verify(&report, &model, &altered, None);
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("validationDatasetSha256"));

    let mut checkpoint: serde_json::Value =
        serde_json::from_slice(&fs::read(&model).expect("read model")).expect("parse model");
    let old = checkpoint["weights"][0].as_f64().expect("finite weight");
    checkpoint["weights"][0] = (old + 0.01).into();
    let tampered = dir.path().join("modified-model.json");
    fs::write(
        &tampered,
        serde_json::to_vec(&checkpoint).expect("serializable"),
    )
    .expect("write changed model");
    let invalid = verify(&report, &tampered, &fixture("holdout.demo.json"), None);
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("checkpointSha256"));
}

#[test]
fn rejects_modified_report_objective_and_missing_schema() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let (report, model) = prepare(dir.path(), None);
    let mut payload: serde_json::Value =
        serde_json::from_slice(&fs::read(&report).expect("read report")).expect("parse report");
    payload["selectedObjective"] = "invalid".into();
    let changed = dir.path().join("changed-report.json");
    fs::write(&changed, serde_json::to_vec(&payload).expect("serialize")).expect("write report");
    let invalid = verify(&changed, &model, &fixture("holdout.demo.json"), None);
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());

    payload["selectedObjective"] = "pointwise".into();
    payload["provenanceSchema"] = "unknown/0".into();
    fs::write(&changed, serde_json::to_vec(&payload).expect("serialize")).expect("write report");
    let invalid = verify(&changed, &model, &fixture("holdout.demo.json"), None);
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
}

#[test]
fn full_replay_verifies_all_ten_candidates_and_final_test_without_manifest() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let (report, model) = prepare(dir.path(), None);
    let replayed = verify_replay(&report, &model, &fixture("holdout.demo.json"), None);
    assert!(
        replayed.status.success(),
        "{}",
        String::from_utf8_lossy(&replayed.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_slice(&replayed.stdout).expect("valid replay JSON");
    assert_eq!(value["replayVerified"], true);
    assert_eq!(value["verified"], true);
    assert!(!String::from_utf8_lossy(&replayed.stdout).contains("synthetic-alpha"));
}

#[test]
fn full_replay_verifies_family_gated_training() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let family = fixture("families.demo.json");
    let (report, model) = prepare(dir.path(), Some(&family));
    let replayed = verify_replay(
        &report,
        &model,
        &fixture("holdout.demo.json"),
        Some(&family),
    );
    assert!(
        replayed.status.success(),
        "{}",
        String::from_utf8_lossy(&replayed.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_slice(&replayed.stdout).expect("valid replay JSON");
    assert_eq!(value["replayVerified"], true);
    assert_eq!(value["familyManifestChecked"], true);
}

#[test]
fn full_replay_rejects_tampered_validation_pair_agreement_while_identity_passes() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let (report, model) = prepare(dir.path(), None);
    let mut payload: serde_json::Value =
        serde_json::from_slice(&fs::read(&report).expect("read report")).expect("parse report");
    let original = payload["candidates"][0]["validationPairAgreement"]
        .as_f64()
        .expect("pair agreement");
    payload["candidates"][0]["validationPairAgreement"] =
        (if original > 0.5 { 0.25 } else { 0.75 }).into();
    let edited = dir.path().join("tampered-candidates.json");
    fs::write(&edited, serde_json::to_vec(&payload).expect("JSON report"))
        .expect("write tampered report");
    let identity_only = verify(&edited, &model, &fixture("holdout.demo.json"), None);
    assert!(identity_only.status.success());
    let replay = verify_replay(&edited, &model, &fixture("holdout.demo.json"), None);
    assert!(!replay.status.success());
    assert!(replay.stdout.is_empty());
    assert!(String::from_utf8_lossy(&replay.stderr).contains("candidates"));
}

#[test]
fn full_replay_rejects_modified_final_test_metrics() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let (report, model) = prepare(dir.path(), None);
    let mut payload: serde_json::Value =
        serde_json::from_slice(&fs::read(&report).expect("read report")).expect("parse report");
    let original = payload["finalTestRetrieval"]["ndcgAtK"]
        .as_f64()
        .expect("final NDCG");
    payload["finalTestRetrieval"]["ndcgAtK"] = (if original > 0.5 { 0.25 } else { 0.75 }).into();
    let edited = dir.path().join("tampered-final-test.json");
    fs::write(&edited, serde_json::to_vec(&payload).expect("JSON report"))
        .expect("write tampered report");
    assert!(
        verify(&edited, &model, &fixture("holdout.demo.json"), None)
            .status
            .success()
    );
    let replay = verify_replay(&edited, &model, &fixture("holdout.demo.json"), None);
    assert!(!replay.status.success());
    assert!(replay.stdout.is_empty());
    assert!(String::from_utf8_lossy(&replay.stderr).contains("finalTestRetrieval"));
}

#[test]
fn replay_mode_rejects_misordered_candidates_and_argument_misuse() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let (report, model) = prepare(dir.path(), None);
    let mut payload: serde_json::Value =
        serde_json::from_slice(&fs::read(&report).expect("read report")).expect("parse report");
    payload["candidates"]
        .as_array_mut()
        .expect("candidate array")
        .swap(1, 2);
    let edited = dir.path().join("reordered-candidates.json");
    fs::write(&edited, serde_json::to_vec(&payload).expect("JSON report"))
        .expect("write tampered report");
    let replay = verify_replay(&edited, &model, &fixture("holdout.demo.json"), None);
    assert!(!replay.status.success());
    assert!(replay.stdout.is_empty());

    let misplaced = Command::new(env!("CARGO_BIN_EXE_verify-repodna-benchmark"))
        .arg(&report)
        .arg(&model)
        .arg(fixture("train.demo.json"))
        .arg(fixture("holdout.demo.json"))
        .arg(fixture("test.demo.json"))
        .arg("--replay")
        .arg("--replay")
        .output()
        .expect("launch malformed replay");
    assert!(!misplaced.status.success());
    assert!(misplaced.stdout.is_empty());
}
