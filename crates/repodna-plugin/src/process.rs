//! Running an analyzer process under constraints: no shell, a minimal environment, the
//! plugin directory as working directory, a time limit, output limits, and cancellation.
//!
//! These limits reduce what a plugin receives and how long it can run. They are not an
//! operating-system sandbox: a plugin runs with the permissions of the user who runs
//! RepoDNA, so only enable plugins you trust.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use repodna_core::CancellationToken;

/// Environment variables passed through to plugins; everything else is removed, so API
/// keys and tokens in the environment never reach a plugin.
const PASSED_VARIABLES: &[&str] = &[
    "PATH",
    "PATHEXT",
    "HOME",
    "USERPROFILE",
    "SYSTEMROOT",
    "WINDIR",
    "TEMP",
    "TMP",
    "TMPDIR",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
];

/// Largest error output kept, in bytes.
const MAX_STDERR: u64 = 64 * 1024;

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The process exited successfully.
    Success(String),
    /// The process could not start or exited unsuccessfully.
    Failed(String),
    /// The process exceeded its time limit and was stopped.
    TimedOut,
    /// The analysis was cancelled and the process was stopped.
    Cancelled,
}

/// Limits for one run.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Time limit.
    pub timeout: Duration,
    /// Largest accepted standard output, in bytes.
    pub max_output_bytes: u64,
}

fn read_limited(
    mut source: impl Read + Send + 'static,
    limit: u64,
) -> thread::JoinHandle<(Vec<u8>, bool)> {
    thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = (&mut source).take(limit).read_to_end(&mut buffer);
        let mut rest = [0u8; 1];
        let exceeded = matches!(source.read(&mut rest), Ok(n) if n > 0);
        // Keep draining so the process never blocks on a full pipe.
        let _ = std::io::copy(&mut source, &mut std::io::sink());
        (buffer, exceeded)
    })
}

/// Runs `command` in `directory`, writing `input` to its standard input.
pub fn run(
    command: &[String],
    directory: &Path,
    input: &[u8],
    limits: Limits,
    cancel: &CancellationToken,
) -> Outcome {
    let Some((program, args)) = command.split_first() else {
        return Outcome::Failed("no command".to_owned());
    };
    let program_path = if program.contains(['/', '\\']) {
        directory.join(program)
    } else {
        Path::new(program).to_path_buf()
    };
    let mut process = Command::new(&program_path);
    process
        .args(args)
        .current_dir(directory)
        .env_clear()
        .env(
            "REPODNA_PLUGIN_API",
            crate::manifest::PLUGIN_API.to_string(),
        )
        .env("REPODNA_VERSION", env!("CARGO_PKG_VERSION"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for name in PASSED_VARIABLES {
        if let Some(value) = std::env::var_os(name) {
            process.env(name, value);
        }
    }
    let mut child = match process.spawn() {
        Ok(child) => child,
        Err(error) => return Outcome::Failed(format!("could not start `{program}`: {error}")),
    };
    let mut stdin = child.stdin.take();
    let input = input.to_vec();
    let writer = thread::spawn(move || {
        if let Some(stdin) = stdin.as_mut() {
            let _ = stdin.write_all(&input);
        }
    });
    let stdout = child
        .stdout
        .take()
        .map(|out| read_limited(out, limits.max_output_bytes));
    let stderr = child.stderr.take().map(|err| read_limited(err, MAX_STDERR));

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => return Outcome::Failed(error.to_string()),
        }
        let cancelled = cancel.is_cancelled();
        if cancelled || started.elapsed() >= limits.timeout {
            let _ = child.kill();
            let _ = child.wait();
            return if cancelled {
                Outcome::Cancelled
            } else {
                Outcome::TimedOut
            };
        }
        thread::sleep(Duration::from_millis(20));
    };
    let _ = writer.join();
    let (output, exceeded) = stdout
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();
    let (errors, _) = stderr
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();
    let error_text = crate::protocol::clean(&String::from_utf8_lossy(&errors), 500);
    if !status.success() {
        return Outcome::Failed(if error_text.is_empty() {
            format!("exited with {status}")
        } else {
            format!("exited with {status}: {error_text}")
        });
    }
    if exceeded {
        return Outcome::Failed(format!(
            "wrote more than {} bytes to standard output",
            limits.max_output_bytes
        ));
    }
    Outcome::Success(String::from_utf8_lossy(&output).into_owned())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn limits() -> Limits {
        Limits {
            timeout: Duration::from_secs(10),
            max_output_bytes: 1024,
        }
    }

    fn sh(script: &str) -> Vec<String> {
        vec!["sh".into(), "-c".into(), script.into()]
    }

    #[test]
    fn runs_with_a_minimal_environment_in_the_plugin_directory() {
        let dir = tempfile::tempdir().unwrap();
        // Cargo sets CARGO_MANIFEST_DIR for test processes; it must not reach the plugin.
        assert!(std::env::var_os("CARGO_MANIFEST_DIR").is_some());
        let outcome = run(
            &sh("cat; pwd; echo \"api=$REPODNA_PLUGIN_API manifest=${CARGO_MANIFEST_DIR:-unset}\""),
            dir.path(),
            b"hello\n",
            limits(),
            &CancellationToken::new(),
        );
        let Outcome::Success(output) = outcome else {
            panic!("{outcome:?}");
        };
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "hello");
        assert_eq!(
            Path::new(lines[1]).canonicalize().unwrap(),
            dir.path().canonicalize().unwrap()
        );
        assert_eq!(lines[2], "api=1 manifest=unset");
    }

    #[test]
    fn enforces_limits() {
        let dir = tempfile::tempdir().unwrap();
        let token = CancellationToken::new();
        let big = run(
            &sh("head -c 5000 /dev/zero"),
            dir.path(),
            b"",
            limits(),
            &token,
        );
        assert!(matches!(big, Outcome::Failed(ref m) if m.contains("more than 1024 bytes")));
        let failed = run(
            &sh("echo oops >&2; exit 4"),
            dir.path(),
            b"",
            limits(),
            &token,
        );
        assert!(matches!(failed, Outcome::Failed(ref m) if m.contains("oops")));
        let slow = run(
            &sh("sleep 5"),
            dir.path(),
            b"",
            Limits {
                timeout: Duration::from_millis(200),
                max_output_bytes: 1024,
            },
            &token,
        );
        assert_eq!(slow, Outcome::TimedOut);
        let cancelled_token = CancellationToken::new();
        cancelled_token.cancel();
        assert_eq!(
            run(&sh("sleep 5"), dir.path(), b"", limits(), &cancelled_token),
            Outcome::Cancelled
        );
        let missing = run(
            &["./missing.sh".to_owned()],
            dir.path(),
            b"",
            limits(),
            &token,
        );
        assert!(matches!(missing, Outcome::Failed(ref m) if m.contains("could not start")));
    }
}
