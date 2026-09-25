//! Development tasks for the RepoDNA repository.
//!
//! ```text
//! cargo xtask fixtures [--out DIR] [--large-files N] [--large-commits N]
//! cargo xtask bench [--repodna PATH] [--fixtures DIR] [--runs N]
//! ```
//!
//! `fixtures` writes every fixture repository (see `repodna_testkit::fixtures`) to
//! `fixtures/generated/`. `bench` times `repodna analyze` on those fixtures and on this
//! repository and prints a Markdown table for the benchmark notes.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

use repodna_testkit::fixtures::{FIXTURES, FixtureOptions, build};

/// Written into the fixtures directory so that it is only ever replaced when RepoDNA made it.
const MARKER: &str = ".repodna-fixtures";

type Result<T> = std::result::Result<T, String>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("fixtures") => fixtures(&args[1..]),
        Some("bench") => bench(&args[1..]),
        _ => {
            let _ = writeln!(
                io::stderr(),
                "usage:\n  cargo xtask fixtures [--out DIR] [--large-files N] [--large-commits N]\n  cargo xtask bench [--repodna PATH] [--fixtures DIR] [--runs N]"
            );
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            let _ = writeln!(io::stderr(), "error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn say(text: &str) {
    let _ = writeln!(io::stdout(), "{text}");
}

/// The value after `--name`, if given.
fn option<'a>(args: &'a [String], name: &str) -> Result<Option<&'a str>> {
    match args.iter().position(|arg| arg == name) {
        None => Ok(None),
        Some(index) => args
            .get(index + 1)
            .map(|value| Some(value.as_str()))
            .ok_or_else(|| format!("{name} needs a value")),
    }
}

fn number(args: &[String], name: &str, default: usize) -> Result<usize> {
    match option(args, name)? {
        None => Ok(default),
        Some(value) => value
            .parse()
            .map_err(|_| format!("{name} must be a whole number, not {value}")),
    }
}

fn fixtures(args: &[String]) -> Result<()> {
    let out =
        option(args, "--out")?.map_or_else(|| root().join("fixtures/generated"), PathBuf::from);
    let defaults = FixtureOptions::default();
    let options = FixtureOptions {
        large_files: number(args, "--large-files", defaults.large_files)?,
        large_commits: number(args, "--large-commits", defaults.large_commits)?,
    };
    if out.exists() {
        let empty = out
            .read_dir()
            .map_err(|e| format!("{}: {e}", out.display()))?
            .next()
            .is_none();
        if !empty && !out.join(MARKER).is_file() {
            return Err(format!(
                "{} exists and was not created by cargo xtask fixtures; choose another --out",
                out.display()
            ));
        }
        std::fs::remove_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    for fixture in FIXTURES {
        let started = Instant::now();
        build(fixture.name, &out.join(fixture.name), &options)
            .map_err(|e| format!("fixture {}: {e}", fixture.name))?;
        say(&format!(
            "{:<14} {:>6} ms  {}",
            fixture.name,
            started.elapsed().as_millis(),
            fixture.description
        ));
    }
    std::fs::write(
        out.join(MARKER),
        "Created by `cargo xtask fixtures`. Safe to delete; run the command again to recreate.\n",
    )
    .map_err(|e| format!("{}: {e}", out.display()))?;
    say(&format!(
        "\nWrote {} fixtures to {}",
        FIXTURES.len(),
        out.display()
    ));
    Ok(())
}

/// Facts about one analysis, read from its JSON artifact.
struct Measured {
    files: u64,
    code_lines: u64,
    commits: u64,
}

fn analyze(
    repodna: &Path,
    home: &Path,
    target: &Path,
    profile: &str,
) -> Result<(Duration, Measured)> {
    let started = Instant::now();
    let output = Command::new(repodna)
        .args(["analyze"])
        .arg(target)
        .args([
            "--profile",
            profile,
            "--format",
            "json",
            "--no-store",
            "--no-cache",
        ])
        .env("REPODNA_HOME", home)
        .env("NO_COLOR", "1")
        .output()
        .map_err(|e| format!("{}: {e}", repodna.display()))?;
    let elapsed = started.elapsed();
    if !output.status.success() {
        return Err(format!(
            "repodna analyze {} failed: {}",
            target.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let dna: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("the artifact for {} is not JSON: {e}", target.display()))?;
    let number = |pointer: &str| dna.pointer(pointer).and_then(serde_json::Value::as_u64);
    Ok((
        elapsed,
        Measured {
            files: number("/structure/totalFiles").unwrap_or(0),
            code_lines: number("/structure/codeLines").unwrap_or(0),
            commits: number("/git/commitCount").unwrap_or(0),
        },
    ))
}

fn bench(args: &[String]) -> Result<()> {
    let root = root();
    let repodna = option(args, "--repodna")?.map_or_else(
        || {
            root.join("target/release")
                .join(format!("repodna{}", std::env::consts::EXE_SUFFIX))
        },
        PathBuf::from,
    );
    if !repodna.is_file() {
        return Err(format!(
            "{} does not exist; build it with `cargo build --release -p repodna-cli` or pass --repodna",
            repodna.display()
        ));
    }
    let fixtures =
        option(args, "--fixtures")?.map_or_else(|| root.join("fixtures/generated"), PathBuf::from);
    if !fixtures.join(MARKER).is_file() {
        return Err(format!(
            "no fixtures in {}; run `cargo xtask fixtures` first",
            fixtures.display()
        ));
    }
    let runs = number(args, "--runs", 3)?.max(1);
    let home = std::env::temp_dir().join(format!("repodna-bench-{}", std::process::id()));
    std::fs::create_dir_all(&home).map_err(|e| format!("{}: {e}", home.display()))?;

    let mut targets: Vec<(String, PathBuf)> = FIXTURES
        .iter()
        .map(|f| (format!("fixture `{}`", f.name), fixtures.join(f.name)))
        .collect();
    targets.push(("this repository".to_owned(), root.clone()));

    let version = Command::new(&repodna)
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default();
    say(&format!(
        "{version} on {} {}, {} logical CPUs, median of {runs} runs, no cache, not stored.\n",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::thread::available_parallelism().map_or(1, usize::from)
    ));
    say("| Repository | Profile | Files | Code lines | Commits | Median time |");
    say("|---|---|---:|---:|---:|---:|");
    for (label, path) in &targets {
        let profiles: &[&str] = if label == "this repository" || label.contains("large") {
            &["quick", "standard", "deep"]
        } else {
            &["standard"]
        };
        for profile in profiles {
            let mut times = Vec::with_capacity(runs);
            let mut measured = None;
            for _ in 0..runs {
                let (elapsed, facts) = analyze(&repodna, &home, path, profile)?;
                times.push(elapsed);
                measured = Some(facts);
            }
            times.sort_unstable();
            let median = times[times.len() / 2];
            let facts = measured.unwrap_or(Measured {
                files: 0,
                code_lines: 0,
                commits: 0,
            });
            say(&format!(
                "| {label} | {profile} | {} | {} | {} | {:.2} s |",
                facts.files,
                facts.code_lines,
                facts.commits,
                median.as_secs_f64()
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&home);
    Ok(())
}
