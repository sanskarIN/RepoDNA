//! Bounded offline microbenchmark for RepoDNA's native evidence ranker.
//!
//! Usage: benchmark-repodna-ranker MODEL.json [CANDIDATES] [ITERATIONS]
//! All candidate evidence is synthetic. Neither labels nor source files are read.

use std::env;
use std::fs;
use std::hint::black_box;
use std::io::{self, Write};
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

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    if !(1..=3).contains(&args.len()) {
        return Err(
            "usage: benchmark-repodna-ranker MODEL.json [CANDIDATES] [ITERATIONS]".into(),
        );
    }
    let candidates = positive_count(args.get(1), DEFAULT_CANDIDATES, MAX_CANDIDATES, "candidates")?;
    let iterations = positive_count(args.get(2), DEFAULT_ITERATIONS, MAX_ITERATIONS, "iterations")?;
    let path = Path::new(&args[0]);
    if fs::metadata(path)?.len() > MAX_MODEL_BYTES {
        return Err("checkpoint exceeds 65536-byte limit".into());
    }
    let bytes = fs::read(path)?;
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
    durations.sort_by(f64::total_cmp);
    let mean = durations.iter().sum::<f64>() / iterations as f64;
    let median = durations[(iterations - 1) / 2];
    let p95 = durations[(iterations * 95).div_ceil(100) - 1];
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
