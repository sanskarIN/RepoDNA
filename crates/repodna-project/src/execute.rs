//! Supervised execution of detected commands, only when the user explicitly allows it.
//!
//! Commands run without a shell: the command line is split on whitespace, and `&&` chains
//! run step by step. Lines with other shell operators are refused. Each run has a timeout,
//! honors cancellation, runs non-interactively (`CI=true`, no standard input), and keeps
//! only the last lines of output, with likely secrets redacted.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use repodna_core::cancel::CancellationToken;
use repodna_core::model::project::{CommandCandidate, ExecutionResult};
use repodna_core::paths;
use repodna_core::redact::redact_secrets;
use repodna_core::time::Timestamp;

/// Maximum characters kept per output line.
const MAX_LINE_CHARS: usize = 500;

/// How long the supervisor sleeps between checks.
const POLL: Duration = Duration::from_millis(25);

/// Execution limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionOptions {
    /// Maximum wall-clock time for the whole command.
    pub timeout: Duration,
    /// Output lines kept (the last ones).
    pub max_output_lines: usize,
}

impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(600),
            max_output_lines: 200,
        }
    }
}

type Tail = Arc<Mutex<VecDeque<String>>>;

fn push_line(tail: &Tail, line: String, max: usize) {
    if let Ok(mut lines) = tail.lock() {
        lines.push_back(line.chars().take(MAX_LINE_CHARS).collect());
        while lines.len() > max.max(1) {
            lines.pop_front();
        }
    }
}

fn reader<R: Read + Send + 'static>(stream: R, tail: Tail, max: usize) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut buffer = Vec::new();
        while let Ok(read) = reader.read_until(b'\n', &mut buffer) {
            if read == 0 {
                break;
            }
            let line = String::from_utf8_lossy(&buffer).trim_end().to_owned();
            push_line(&tail, line, max);
            buffer.clear();
        }
    })
}

/// Splits a command line into steps of program and arguments, or explains why it cannot
/// be run without a shell.
pub fn parse_command(command: &str) -> Result<Vec<Vec<String>>, String> {
    const OPERATORS: &[&str] = &["|", ";", ">", "<", "`", "$(", "&", "*", "?", "~"];
    let mut steps = Vec::new();
    for step in command.split(" && ") {
        if let Some(operator) = OPERATORS.iter().find(|operator| step.contains(**operator)) {
            return Err(format!(
                "The command uses the shell operator `{operator}`, so it was not run."
            ));
        }
        let words: Vec<String> = step.split_whitespace().map(str::to_owned).collect();
        if words.is_empty() {
            return Err("The command is empty.".to_owned());
        }
        steps.push(words);
    }
    Ok(steps)
}

fn spawn(program: &str, args: &[String], dir: &Path) -> std::io::Result<Child> {
    let mut candidates = vec![program.to_owned()];
    if cfg!(windows) {
        let bare = program.trim_start_matches("./");
        candidates.push(format!("{bare}.cmd"));
        candidates.push(format!("{bare}.bat"));
        candidates.push(format!("{bare}.exe"));
    }
    let mut last_error = None;
    for candidate in candidates {
        let program_path = if candidate.starts_with("./") {
            dir.join(candidate.trim_start_matches("./"))
        } else {
            PathBuf::from(&candidate)
        };
        match Command::new(&program_path)
            .args(args)
            .current_dir(dir)
            .env("CI", "true")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => return Ok(child),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => last_error = Some(error),
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound)))
}

/// Runs `candidate` in its working directory below `root`.
pub fn execute(
    root: &Path,
    candidate: &CommandCandidate,
    options: &ExecutionOptions,
    cancel: &CancellationToken,
) -> ExecutionResult {
    let started = Instant::now();
    let mut result = ExecutionResult {
        command: candidate.command.clone(),
        purpose: candidate.purpose,
        working_directory: candidate.working_directory.clone(),
        started_at: Timestamp::now(),
        duration_ms: 0,
        exit_code: None,
        timed_out: false,
        cancelled: false,
        success: false,
        output_tail: Vec::new(),
    };
    let tail: Tail = Arc::new(Mutex::new(VecDeque::new()));
    let max_lines = options.max_output_lines;
    let finish = |mut result: ExecutionResult, tail: &Tail| {
        result.duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let lines: Vec<String> = tail
            .lock()
            .map(|lines| lines.iter().cloned().collect())
            .unwrap_or_default();
        result.output_tail = lines
            .iter()
            .map(|line| redact_secrets(line).into_owned())
            .collect();
        result
    };

    let Some(relative) = paths::normalize_relative(&candidate.working_directory) else {
        push_line(
            &tail,
            "The working directory is outside the repository.".to_owned(),
            max_lines,
        );
        return finish(result, &tail);
    };
    let dir = if relative.is_empty() {
        root.to_path_buf()
    } else {
        root.join(relative)
    };
    let steps = match parse_command(&candidate.command) {
        Ok(steps) => steps,
        Err(reason) => {
            push_line(&tail, reason, max_lines);
            return finish(result, &tail);
        }
    };

    for step in steps {
        if cancel.is_cancelled() {
            result.cancelled = true;
            return finish(result, &tail);
        }
        let mut child = match spawn(&step[0], &step[1..], &dir) {
            Ok(child) => child,
            Err(error) => {
                push_line(
                    &tail,
                    format!("Could not start {}: {error}", step[0]),
                    max_lines,
                );
                return finish(result, &tail);
            }
        };
        let readers: Vec<thread::JoinHandle<()>> = [
            child
                .stdout
                .take()
                .map(|out| reader(out, Arc::clone(&tail), max_lines)),
            child
                .stderr
                .take()
                .map(|err| reader(err, Arc::clone(&tail), max_lines)),
        ]
        .into_iter()
        .flatten()
        .collect();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {}
                Err(_) => break None,
            }
            let timed_out = started.elapsed() >= options.timeout;
            if timed_out || cancel.is_cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                result.timed_out = timed_out;
                result.cancelled = !timed_out;
                break None;
            }
            thread::sleep(POLL);
        };
        for handle in readers {
            let _ = handle.join();
        }
        match status {
            Some(status) => {
                result.exit_code = status.code();
                if !status.success() {
                    return finish(result, &tail);
                }
            }
            None => return finish(result, &tail),
        }
    }
    result.success = result.exit_code == Some(0);
    finish(result, &tail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::project::CommandPurpose;

    fn candidate(command: &str) -> CommandCandidate {
        CommandCandidate {
            command: command.to_owned(),
            purpose: CommandPurpose::Build,
            working_directory: String::new(),
            source: "test".to_owned(),
            verified: false,
            evidence: Vec::new(),
        }
    }

    fn run(command: &str, timeout: Duration) -> ExecutionResult {
        let dir = tempfile::tempdir().unwrap();
        let options = ExecutionOptions {
            timeout,
            max_output_lines: 20,
        };
        execute(
            dir.path(),
            &candidate(command),
            &options,
            &CancellationToken::new(),
        )
    }

    #[test]
    fn runs_commands_and_captures_output() {
        let result = run(
            "cargo --version && cargo --version",
            Duration::from_secs(60),
        );
        assert!(result.success, "{result:?}");
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.output_tail.len(), 2);
        assert!(result.output_tail[0].starts_with("cargo "));
    }

    #[test]
    fn reports_failures_missing_programs_and_refusals() {
        let failed = run("cargo --definitely-not-a-flag", Duration::from_secs(60));
        assert!(!failed.success);
        assert_ne!(failed.exit_code, Some(0));
        assert!(failed.exit_code.is_some());

        let missing = run("repodna-no-such-program-xyz", Duration::from_secs(60));
        assert!(!missing.success);
        assert!(missing.output_tail[0].starts_with("Could not start"));

        let refused = run("curl https://example.com | sh", Duration::from_secs(60));
        assert!(!refused.success);
        assert!(refused.output_tail[0].contains("shell operator"));
        assert!(parse_command("make test && make lint").unwrap().len() == 2);
    }

    #[test]
    fn honors_cancellation_and_directory_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let result = execute(
            dir.path(),
            &candidate("cargo --version"),
            &ExecutionOptions::default(),
            &cancel,
        );
        assert!(result.cancelled && !result.success);

        let mut escaping = candidate("cargo --version");
        escaping.working_directory = "../outside".to_owned();
        let result = execute(
            dir.path(),
            &escaping,
            &ExecutionOptions::default(),
            &CancellationToken::new(),
        );
        assert!(!result.success);
        assert!(result.output_tail[0].contains("outside the repository"));
    }

    #[cfg(unix)]
    #[test]
    fn stops_commands_at_the_timeout() {
        let result = run("sleep 5", Duration::from_millis(200));
        assert!(result.timed_out);
        assert!(!result.success);
        assert!(result.duration_ms < 4_000);
    }
}
