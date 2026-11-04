//! Bounded offline microbenchmark for RepoDNA's native evidence ranker.
//!
//! Usage: `benchmark-repodna-ranker MODEL.json [CANDIDATES] [ITERATIONS]`
//! All candidate evidence is synthetic. Neither labels nor source files are read.

use std::env;
use std::fs;
use std::hint::black_box;
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Instant;

use repodna_ai::ranking::RankerModel;
use repodna_ai::{EvidenceItem, EvidenceKind};

const MAX_MODEL_BYTES: u64 = 64 * 1024;
const DEFAULT_CANDIDATES: usize = 12;
const DEFAULT_ITERATIONS: usize = 100;
const MAX_CANDIDATES: usize = 1_024;
const MAX_ITERATIONS: usize = 1_000;
const QUERY: &str = "Which modules have dependency and build evidence?";

fn positive_count(
    argument: Option<&std::ffi::OsString>,
    default: usize,
    max: usize,
    name: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    let Some(raw) = argument else {
        return Ok(default);
    };
    let value = raw
        .to_str()
        .ok_or_else(|| format!("{name} must be an ASCII integer"))?;
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{name} must be an ASCII integer"))?;
    if !(1..=max).contains(&parsed) {
        return Err(format!("{name} must be between 1 and {max}").into());
    }
    Ok(parsed)
}

fn synthetic_evidence(count: usize) -> Vec<EvidenceItem> {
    (0..count)
        .map(|index| EvidenceItem {
            id: format!("E{}", index + 1),
            kind: match index % 4 {
                0 => EvidenceKind::Package,
                1 => EvidenceKind::Module,
                2 => EvidenceKind::Finding,
                _ => EvidenceKind::Command,
            },
            reference: format!("src/module_{}/manifest", index % 37),
            detail: format!(
                "Recorded dependency and build evidence for synthetic module {}",
                index % 19
            ),
        })
        .collect()
}

/// Mean, conventional median (midpoint for an even sample count), and
/// nearest-rank p95. Samples are nonnegative elapsed times in microseconds.
fn summarize_latency_micros(durations: &mut [f64]) -> (f64, f64, f64) {
    assert!(
        !durations.is_empty(),
        "at least one timing sample is required"
    );
    durations.sort_by(f64::total_cmp);
    let n = durations.len();
    let mean = durations.iter().sum::<f64>() / n as f64;
    let median = if n.is_multiple_of(2) {
        (durations[n / 2 - 1] + durations[n / 2]) / 2.0
    } else {
        durations[n / 2]
    };
    let p95 = durations[(n * 95).div_ceil(100) - 1];
    (mean, median, p95)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    if !(1..=3).contains(&args.len()) {
        return Err("usage: benchmark-repodna-ranker MODEL.json [CANDIDATES] [ITERATIONS]".into());
    }
    let candidates = positive_count(
        args.get(1),
        DEFAULT_CANDIDATES,
        MAX_CANDIDATES,
        "candidates",
    )?;
    let iterations = positive_count(
        args.get(2),
        DEFAULT_ITERATIONS,
        MAX_ITERATIONS,
        "iterations",
    )?;
    let path = Path::new(&args[0]);
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err("checkpoint must be a regular file".into());
    }
    if metadata.len() > MAX_MODEL_BYTES {
        return Err("checkpoint exceeds 65536-byte limit".into());
    }
    // A metadata check alone cannot bound files that grow between stat and
    // read. Never allocate more than the size cap plus one byte.
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_MODEL_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MODEL_BYTES {
        return Err("checkpoint grew beyond 65536-byte limit".into());
    }
    let model: RankerModel = serde_json::from_slice(&bytes)?;
    model.validate().map_err(io::Error::other)?;

    let evidence = synthetic_evidence(candidates);
    let original_order = model.rank_indices(QUERY, &evidence);
    // Warm the same workload before collecting samples, but do not count it.
    for _ in 0..10 {
        black_box(model.rank_indices(black_box(QUERY), black_box(&evidence)));
    }
    let mut durations = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let started = Instant::now();
        let ranked = black_box(model.rank_indices(black_box(QUERY), black_box(&evidence)));
        let elapsed = started.elapsed();
        if ranked != original_order {
            return Err("nondeterministic rank ordering detected".into());
        }
        durations.push(elapsed.as_secs_f64() * 1_000_000.0);
    }
    let (mean, median, p95) = summarize_latency_micros(&mut durations);
    let results = serde_json::json!({
        "schema": "repodna-ranker-performance/1",
        "modelFormat": model.format,
        "workload": "synthetic-evidence",
        "candidates": candidates,
        "iterations": iterations,
        "meanLatencyMicros": mean,
        "medianLatencyMicros": median,
        "p95LatencyMicros": p95,
        "note": "Machine-dependent local microbenchmark. Not a model-quality, accuracy or production-latency claim."
    });
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &results)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(io::stderr().lock(), "benchmark failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::summarize_latency_micros;

    #[test]
    fn conventional_median_averages_middle_pair() {
        let mut samples = [9.0, 1.0, 3.0, 5.0];
        let (mean, median, p95) = summarize_latency_micros(&mut samples);
        assert_eq!(mean, 4.5);
        assert_eq!(median, 4.0);
        assert_eq!(p95, 9.0);
        assert_eq!(samples, [1.0, 3.0, 5.0, 9.0]);
    }

    #[test]
    fn odd_and_single_sample_percentiles_are_well_defined() {
        let mut odd = [5.0, 1.0, 3.0];
        assert_eq!(summarize_latency_micros(&mut odd), (3.0, 3.0, 5.0));
        let mut single = [2.5];
        assert_eq!(summarize_latency_micros(&mut single), (2.5, 2.5, 2.5));
    }

    #[test]
    fn nearest_rank_p95_uses_the_ceiling_rank() {
        let mut samples = (1..=100).rev().map(f64::from).collect::<Vec<_>>();
        let (mean, median, p95) = summarize_latency_micros(&mut samples);
        assert_eq!(mean, 50.5);
        assert_eq!(median, 50.5);
        assert_eq!(p95, 95.0);
    }
}
