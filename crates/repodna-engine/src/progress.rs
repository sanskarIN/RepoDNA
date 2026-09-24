//! Progress reporting for long analyses.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use repodna_core::config::Stage;
use repodna_core::model::metadata::AnalyzerStatus;

/// Something that happened during an analysis.
#[derive(Debug, Clone, PartialEq)]
pub enum ProgressEvent {
    /// A stage started.
    StageStarted(Stage),
    /// A stage finished.
    StageFinished {
        /// The stage.
        stage: Stage,
        /// How it ended.
        status: AnalyzerStatus,
        /// How long it took.
        duration: Duration,
    },
    /// Files were processed during a stage.
    Files {
        /// Files processed so far.
        done: usize,
        /// Files in total.
        total: usize,
    },
    /// A human-readable note.
    Message(String),
}

type Callback = dyn Fn(&ProgressEvent) + Send + Sync;

/// Receives progress events. The default discards them.
#[derive(Clone, Default)]
pub struct Progress(Option<Arc<Callback>>);

impl Progress {
    /// Sends events to `callback`.
    pub fn new(callback: impl Fn(&ProgressEvent) + Send + Sync + 'static) -> Self {
        Self(Some(Arc::new(callback)))
    }

    /// Reports an event.
    pub fn emit(&self, event: ProgressEvent) {
        if let Some(callback) = &self.0 {
            callback(&event);
        }
    }
}

impl fmt::Debug for Progress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.0.is_some() {
            "Progress(callback)"
        } else {
            "Progress(none)"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn forwards_events_to_the_callback() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let progress = Progress::new(move |event| sink.lock().unwrap().push(event.clone()));
        progress.emit(ProgressEvent::StageStarted(Stage::Git));
        Progress::default().emit(ProgressEvent::Message("ignored".into()));
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert_eq!(format!("{progress:?}"), "Progress(callback)");
    }
}
