//! A local program that reads the prompt on standard input and writes the answer to
//! standard output, such as `ollama run <model>`. It runs without a shell. RepoDNA
//! treats it as local; what the program itself does with the prompt is up to the program.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::AiError;
use crate::provider::{AiProvider, Completion, CompletionRequest};
use crate::text::single_line;

/// Largest answer read from standard output, in bytes.
const MAX_STDOUT: u64 = 2 * 1024 * 1024;
/// Largest error output kept, in bytes.
const MAX_STDERR: u64 = 64 * 1024;

/// Runs a local command per request.
#[derive(Debug, Clone)]
pub struct CommandProvider {
    program: String,
    args: Vec<String>,
    model: String,
    timeout: u64,
}

impl CommandProvider {
    /// Creates a provider for `program` with `args`. `model` labels the answers; it
    /// defaults to the program's file name.
    pub fn new(
        program: String,
        args: Vec<String>,
        model: Option<String>,
        timeout_seconds: u64,
    ) -> Self {
        let model = model.unwrap_or_else(|| {
            std::path::Path::new(&program).file_name().map_or_else(
                || program.clone(),
                |name| name.to_string_lossy().into_owned(),
            )
        });
        Self {
            program,
            args,
            model,
            timeout: timeout_seconds,
        }
    }
}

fn read_limited(mut source: impl Read + Send + 'static, limit: u64) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = (&mut source).take(limit).read_to_end(&mut buffer);
        // Drain the rest so the program never blocks on a full pipe.
        let _ = std::io::copy(&mut source, &mut std::io::sink());
        buffer
    })
}

impl AiProvider for CommandProvider {
    fn id(&self) -> &'static str {
        "command"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn remote(&self) -> bool {
        false
    }

    fn destination(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn complete(&self, request: &CompletionRequest<'_>) -> Result<Completion, AiError> {
        let mut child = Command::new(&self.program)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                AiError::Command(format!("could not start `{}`: {error}", self.program))
            })?;
        let prompt = format!("{}\n\n{}\n", request.system, request.user);
        let mut stdin = child.stdin.take();
        let writer = thread::spawn(move || {
            if let Some(stdin) = stdin.as_mut() {
                // A program may exit before reading everything; that is reported by
                // its exit status, not here.
                let _ = stdin.write_all(prompt.as_bytes());
            }
        });
        let stdout = child.stdout.take().map(|out| read_limited(out, MAX_STDOUT));
        let stderr = child.stderr.take().map(|err| read_limited(err, MAX_STDERR));

        let started = Instant::now();
        let limit = Duration::from_secs(self.timeout.max(1));
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {}
                Err(error) => return Err(AiError::Command(error.to_string())),
            }
            let cancelled = request.cancel.is_some_and(|token| token.is_cancelled());
            if cancelled || started.elapsed() >= limit {
                let _ = child.kill();
                let _ = child.wait();
                return Err(if cancelled {
                    AiError::Cancelled
                } else {
                    AiError::Timeout(self.timeout)
                });
            }
            thread::sleep(Duration::from_millis(25));
        };
        let _ = writer.join();
        let output = stdout
            .and_then(|handle| handle.join().ok())
            .unwrap_or_default();
        let errors = stderr
            .and_then(|handle| handle.join().ok())
            .unwrap_or_default();
        if !status.success() {
            let detail = single_line(&String::from_utf8_lossy(&errors), 300);
            return Err(AiError::Command(format!(
                "`{}` exited with {status}{}",
                self.program,
                if detail.is_empty() {
                    String::new()
                } else {
                    format!(": {detail}")
                }
            )));
        }
        let text = String::from_utf8_lossy(&output).into_owned();
        if text.trim().is_empty() {
            return Err(AiError::Command(format!(
                "`{}` produced no output",
                self.program
            )));
        }
        Ok(Completion {
            text,
            input_tokens: None,
            output_tokens: None,
        })
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use repodna_core::CancellationToken;

    fn request<'a>(cancel: Option<&'a CancellationToken>) -> CompletionRequest<'a> {
        CompletionRequest {
            system: "rules",
            user: "task",
            max_output_tokens: 100,
            cancel,
        }
    }

    fn shell(script: &str, timeout: u64) -> CommandProvider {
        CommandProvider::new("sh".into(), vec!["-c".into(), script.into()], None, timeout)
    }

    #[test]
    fn sends_the_prompt_on_standard_input() {
        let provider = shell("tr a-z A-Z", 10);
        assert_eq!(provider.model(), "sh");
        assert_eq!(provider.destination(), "sh -c tr a-z A-Z");
        let completion = provider.complete(&request(None)).unwrap();
        assert_eq!(completion.text, "RULES\n\nTASK\n");
        assert_eq!(completion.input_tokens, None);
    }

    #[test]
    fn reports_failures_timeouts_and_cancellation() {
        let failed = shell("cat >/dev/null; echo broken >&2; exit 3", 10)
            .complete(&request(None))
            .unwrap_err();
        assert!(matches!(failed, AiError::Command(ref m) if m.contains("broken")));
        let silent = shell("cat >/dev/null", 10)
            .complete(&request(None))
            .unwrap_err();
        assert!(matches!(silent, AiError::Command(ref m) if m.contains("no output")));
        let slow = shell("sleep 5", 1).complete(&request(None)).unwrap_err();
        assert!(matches!(slow, AiError::Timeout(1)));
        let token = CancellationToken::new();
        token.cancel();
        let cancelled = shell("sleep 5", 10)
            .complete(&request(Some(&token)))
            .unwrap_err();
        assert!(matches!(cancelled, AiError::Cancelled));
        let missing = CommandProvider::new("repodna-no-such-program".into(), Vec::new(), None, 5)
            .complete(&request(None))
            .unwrap_err();
        assert!(matches!(missing, AiError::Command(ref m) if m.contains("could not start")));
    }
}
