//! CLI tests for explicit, manually curated fork-family separation.
//! Synthetic manifests exercise split integrity, not actual code provenance.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/native-ai")
        .join(name)
}

fn audit(manifest: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_audit-repodna-labels"));
    command
        .arg(fixture("train.demo.json"))
        .arg(fixture("holdout.demo.json"))
        .arg(fixture("test.demo.json"));
    if let Some(path) = manifest {
        command.arg(path);
    }
    command.output().expect("execute dataset audit")
}

fn compare(manifest: Option<&Path>, output: &Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_compare-repodna-rankers"));
    command
        .arg(fixture("train.demo.json"))
        .arg(fixture("holdout.demo.json"))
        .arg(fixture("test.demo.json"))
        .arg(output);
    if let Some(path) = manifest {
        command.arg(path);
    }
    command.output().expect("execute objective comparison")
}

fn write_manifest(dir: &Path, entries: &[serde_json::Value]) -> PathBuf {
    let path = dir.join("families.json");
    fs::write(
        &path,
        serde_json::to_vec(entries).expect("valid test fixture and output"),
    )
    .expect("valid test fixture and output");
    path
}

fn manifest() -> Vec<serde_json::Value> {
    serde_json::from_slice(
        &fs::read(fixture("families.demo.json")).expect("valid test fixture and output"),
    )
    .expect("valid test fixture and output")
}

#[test]
fn valid_family_manifest_is_checked_before_training_and_audit() {
    let dir = tempfile::tempdir().expect("valid test fixture and output");
    let family_path = fixture("families.demo.json");
    let audited = audit(Some(&family_path));
    assert!(
        audited.status.success(),
        "{}",
        String::from_utf8_lossy(&audited.stderr)
    );
    let summary: serde_json::Value =
        serde_json::from_slice(&audited.stdout).expect("valid test fixture and output");
    assert_eq!(summary["structuralChecksPassed"], true);
    assert_eq!(summary["familyManifestChecked"], true);
    assert_eq!(summary["familySplit"]["manifestRepositories"], 5);
    assert_eq!(summary["familySplit"]["trainingFamilies"], 2);
    assert_eq!(summary["familySplit"]["validationFamilies"], 1);
    assert_eq!(summary["familySplit"]["finalTestFamilies"], 2);

    let saved = dir.path().join("native-model.json");
    let trained = compare(Some(&family_path), &saved);
    assert!(
        trained.status.success(),
        "{}",
        String::from_utf8_lossy(&trained.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&trained.stdout).expect("valid test fixture and output");
    assert_eq!(report["familyManifestChecked"], true);
    assert_eq!(report["familySplit"], summary["familySplit"]);
    assert!(saved.exists());

    let clean = audit(None);
    assert!(clean.status.success());
    let clean_report: serde_json::Value =
        serde_json::from_slice(&clean.stdout).expect("valid test fixture and output");
    assert_eq!(clean_report["familyManifestChecked"], false);
    assert!(clean_report["familySplit"].is_null());

    let text = String::from_utf8(audited.stdout).expect("valid test fixture and output");
    assert!(!text.contains("synthetic-alpha"));
    assert!(!text.contains("synthetic-family-1"));
}

#[test]
fn different_repository_ids_in_one_family_are_rejected_in_both_clis() {
    let dir = tempfile::tempdir().expect("valid test fixture and output");
    let mut entries = manifest();
    entries[2]["family"] = " SYNTHETIC-FAMILY-1 ".into();
    let path = write_manifest(dir.path(), &entries);

    let audited = audit(Some(&path));
    assert!(!audited.status.success());
    assert!(audited.stdout.is_empty());
    assert!(String::from_utf8_lossy(&audited.stderr).contains("family leakage"));

    let saved = dir.path().join("must-not-create.json");
    let compared = compare(Some(&path), &saved);
    assert!(!compared.status.success());
    assert!(compared.stdout.is_empty());
    assert!(String::from_utf8_lossy(&compared.stderr).contains("family leakage"));
    assert!(!saved.exists());

    // Error messages should not reveal exact family/repository identifiers.
    assert!(!String::from_utf8_lossy(&compared.stderr).contains("synthetic-alpha"));
}

#[test]
fn missing_or_aliased_repository_entries_fail_closed() {
    let dir = tempfile::tempdir().expect("valid test fixture and output");
    let mut entries = manifest();
    entries.pop();
    let path = write_manifest(dir.path(), &entries);
    let result = audit(Some(&path));
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("missing"));

    let mut entries = manifest();
    entries[1]["repository"] = " SYNTHETIC-ALPHA ".into();
    let path = write_manifest(dir.path(), &entries);
    let result = audit(Some(&path));
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("duplicate"));

    let mut entries = manifest();
    entries.push(serde_json::json!({
        "repository":"unused-repository",
        "family":"unused-family",
        "revision":"f".repeat(40),
        "license":"NOASSERTION"
    }));
    let path = write_manifest(dir.path(), &entries);
    let result = audit(Some(&path));
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("absent"));
}

#[test]
fn malformed_metadata_and_excessive_size_are_refused() {
    let dir = tempfile::tempdir().expect("valid test fixture and output");
    let mut entries = manifest();
    entries[0]["revision"] = "main".into();
    let path = write_manifest(dir.path(), &entries);
    let invalid = audit(Some(&path));
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());

    let mut entries = manifest();
    entries[0]["license"] = " ".into();
    let path = write_manifest(dir.path(), &entries);
    assert!(!audit(Some(&path)).status.success());

    let too_large = dir.path().join("oversized.json");
    fs::write(&too_large, vec![b' '; 2 * 1024 * 1024 + 1]).expect("valid test fixture and output");
    let result = audit(Some(&too_large));
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("2 MiB"));

    let malformed = dir.path().join("wrong.json");
    fs::write(&malformed, b"{}").expect("valid test fixture and output");
    assert!(!audit(Some(&malformed)).status.success());
}
