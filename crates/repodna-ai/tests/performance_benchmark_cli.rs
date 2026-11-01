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
    fs::write(path, serde_json::to_vec(&model).unwrap()).unwrap();
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
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ranker.json");
    checkpoint(&path);
    let result = benchmark(&path, &["8", "5"]);
    assert!(
        result.status.success(),
        "benchmark stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["schema"], "repodna-ranker-performance/1");
    assert_eq!(report["modelFormat"], MODEL_FORMAT);
    assert_eq!(report["workload"], "synthetic-evidence");
    assert_eq!(report["candidates"], 8);
    assert_eq!(report["iterations"], 5);
    let median = report["medianLatencyMicros"].as_f64().unwrap();
    let p95 = report["p95LatencyMicros"].as_f64().unwrap();
    assert!(median.is_finite() && median >= 0.0);
    assert!(p95.is_finite() && p95 >= median);
    assert!(report["meanLatencyMicros"].as_f64().unwrap().is_finite());
    let printed = String::from_utf8(result.stdout).unwrap();
    assert!(!printed.contains(path.to_str().unwrap()));
    assert!(!printed.contains("trainingDatasetSha256"));
    assert!(!printed.contains("Cargo.lock"));
}

#[test]
fn invalid_counts_and_checkpoints_fail_without_json() {
    let dir = tempfile::tempdir().unwrap();
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
    fs::write(&too_large, vec![b' '; 65_537]).unwrap();
    let result = benchmark(&too_large, &[]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());

    let bad = dir.path().join("invalid.json");
    let mut invalid: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    invalid["weights"][0] = 100.0.into();
    fs::write(&bad, serde_json::to_vec(&invalid).unwrap()).unwrap();
    let result = benchmark(&bad, &[]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
}
