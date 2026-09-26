//! Progress on standard error: one line per finished stage, and on terminals a live line
//! with real counts ("Discovery 1,203 / 4,812 files"). There are no estimated or fake
//! progress bars.

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use repodna_core::model::metadata::AnalyzerStatus;
use repodna_engine::{Progress, ProgressEvent};

use crate::term::{Style, thousands};

#[derive(Default)]
struct State {
    live: bool,
    current: Option<String>,
}

/// Where progress goes.
#[derive(Clone)]
pub struct Reporter {
    style: Style,
    live: bool,
    state: Arc<Mutex<State>>,
}

fn seconds(duration: Duration) -> String {
    let secs = duration.as_secs_f64();
    if secs < 10.0 {
        format!("{secs:.1}s")
    } else {
        format!("{secs:.0}s")
    }
}

impl Reporter {
    /// A reporter; `live` enables in-place updates (only on terminals).
    pub fn new(style: Style, live: bool) -> Self {
        Self {
            style,
            live,
            state: Arc::new(Mutex::new(State::default())),
        }
    }

    fn clear_live(&self, state: &mut State, err: &mut impl Write) {
        if state.live {
            let _ = write!(err, "\r\u{1b}[2K");
            state.live = false;
        }
    }

    /// Prints a line, clearing any live line first.
    pub fn line(&self, text: &str) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut err = std::io::stderr().lock();
        self.clear_live(&mut state, &mut err);
        let _ = writeln!(err, "{text}");
    }

    fn event(&self, event: &ProgressEvent) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut err = std::io::stderr().lock();
        match event {
            ProgressEvent::StageStarted(stage) => {
                state.current = Some(stage.label().to_owned());
                if self.live {
                    self.clear_live(&mut state, &mut err);
                    let _ = write!(err, "  {} {}…", self.style.dim("·"), stage.label());
                    state.live = true;
                }
            }
            ProgressEvent::Files { done, total } => {
                if self.live {
                    self.clear_live(&mut state, &mut err);
                    let label = state.current.clone().unwrap_or_else(|| "Files".to_owned());
                    let _ = write!(
                        err,
                        "  {} {label} {} / {} files",
                        self.style.dim("·"),
                        thousands(*done as u64),
                        thousands(*total as u64)
                    );
                    state.live = true;
                }
            }
            ProgressEvent::StageFinished {
                stage,
                status,
                duration,
            } => {
                self.clear_live(&mut state, &mut err);
                let symbol = match status {
                    AnalyzerStatus::Completed => self.style.green("✓"),
                    AnalyzerStatus::Partial => self.style.yellow("!"),
                    AnalyzerStatus::Failed => self.style.red("✗"),
                    AnalyzerStatus::Cancelled => self.style.yellow("■"),
                    AnalyzerStatus::Skipped => return,
                };
                let note = match status {
                    AnalyzerStatus::Partial => " (partial)",
                    AnalyzerStatus::Failed => " (failed)",
                    AnalyzerStatus::Cancelled => " (cancelled)",
                    _ => "",
                };
                let _ = writeln!(
                    err,
                    "  {symbol} {:<34} {}",
                    format!("{}{note}", stage.label()),
                    self.style.dim(&seconds(*duration))
                );
                state.current = None;
            }
            ProgressEvent::Message(text) => {
                self.clear_live(&mut state, &mut err);
                let _ = writeln!(err, "    {}", self.style.dim(text));
            }
        }
        let _ = err.flush();
    }

    /// An engine progress sink that prints through this reporter.
    pub fn progress(&self) -> Progress {
        let reporter = self.clone();
        Progress::new(move |event| reporter.event(event))
    }

    /// Clears any live line (for example before printing an error).
    pub fn finish(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut err = std::io::stderr().lock();
        self.clear_live(&mut state, &mut err);
        let _ = err.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_durations() {
        assert_eq!(seconds(Duration::from_millis(420)), "0.4s");
        assert_eq!(seconds(Duration::from_secs(42)), "42s");
    }
}
