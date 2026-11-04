//! Offline benchmark CLI smoke tests. Numbers vary by host and are not quality claims.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use repodna_ai::ranking::{FEATURE_COUNT, MODEL_FORMAT, RankerModel};

fn checkpoint(path: &Path) {
    let model = RankerModel {
        format: MODEL_FORMAT.into(),
        weights: vec![0.0; FEATURE_COUNT],
        training_examples: 2,
        training_repositories: 1,
        training_dataset_sha256: None,
    };
    let serialized = serde_json::to_vec(&model).expect("serialize checkpoint");
    fs::write(path, serialized).expect("write checkpoint");
}

fn benchmark(path: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_benchmark-repodna-ranker"))
        .arg(path)
        .args(extra)
        .output()
        .expect("launch offline benchmark")
}

#[test]
fn reports_bounded_private_local_latency_diagnostics() {
    let dir = tempfile::tempdir().expect("create temp directory");
    let path = dir.path().join("ranker.json");
    checkpoint(&path);
    let result = benchmark(&path, &["8", "5"]);
    assert!(
        result.status.success(),
        "benchmark stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&result.stdout).expect("parse benchmark report");
    assert_eq!(report["schema"], "repodna-ranker-performance/1");
    assert_eq!(report["modelFormat"], MODEL_FORMAT);
    assert_eq!(report["workload"], "synthetic-evidence");
    assert_eq!(report["candidates"], 8);
    assert_eq!(report["iterations"], 5);
    let median = report["medianLatencyMicros"]
        .as_f64()
        .expect("median latency");
    let p95 = report["p95LatencyMicros"].as_f64().expect("p95 latency");
    assert!(median.is_finite() && median >= 0.0);
    assert!(p95.is_finite() && p95 >= median);
    let mean = report["meanLatencyMicros"].as_f64().expect("mean latency");
    assert!(mean.is_finite());
    let printed = String::from_utf8(result.stdout).expect("UTF-8 report");
    assert!(!printed.contains(path.to_str().expect("UTF-8 path")));
    assert!(!printed.contains("trainingDatasetSha256"));
    assert!(!printed.contains("Cargo.lock"));
}

#[test]
fn invalid_counts_and_checkpoints_fail_without_json() {
    let dir = tempfile::tempdir().expect("create temp directory");
    let path = dir.path().join("ranker.json");
    checkpoint(&path);
    for args in [
        vec!["0"],
        vec!["1025"],
        vec!["x"],
        vec!["8", "0"],
        vec!["8", "1001"],
        vec!["8", "-1"],
        vec!["8", "5", "unexpected"],
    ] {
        let result = benchmark(&path, &args);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
    }
    let too_large = dir.path().join("oversize.json");
    fs::write(&too_large, vec![b' '; 65_537]).expect("write oversized checkpoint");
    let result = benchmark(&too_large, &[]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());

    let bad = dir.path().join("invalid.json");
    let saved = fs::read(&path).expect("read checkpoint");
    let mut invalid: serde_json::Value = serde_json::from_slice(&saved).expect("parse checkpoint");
    invalid["weights"][0] = 100.0.into();
    let serialized = serde_json::to_vec(&invalid).expect("serialize invalid checkpoint");
    fs::write(&bad, serialized).expect("write invalid checkpoint");
    let result = benchmark(&bad, &[]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
}

#[cfg(unix)]
#[test]
fn rejects_special_checkpoint_files_without_json() {
    let result = benchmark(Path::new("/dev/null"), &[]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("regular file"));
}
