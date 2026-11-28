//! Analyses started through the API run in background threads; clients poll their state.

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use repodna_app::{AnalyzeOptions, AppPaths, ErrorKind, run_analysis};
use repodna_core::CancellationToken;
use repodna_core::hash::to_hex;
use repodna_core::model::metadata::AnalyzerStatus;
use repodna_core::time::Timestamp;
use repodna_engine::{Progress, ProgressEvent};
use serde::Serialize;

/// Most analyses running at once.
pub const MAX_RUNNING: usize = 2;
/// Total job states retained for polling, including active jobs.
const KEEP_FINISHED: usize = 50;
const MAX_WARNINGS: usize = 64;
const MAX_WARNING_BYTES: usize = 2048;
const OMITTED_WARNINGS: &str = "Additional analysis warnings were omitted from live job status.";

/// Where a job is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Still running.
    Running,
    /// Finished and stored.
    Completed,
    /// Stopped by an error.
    Failed,
    /// Stopped on request.
    Cancelled,
}

/// A job's observable state.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobState {
    /// Identifier.
    pub id: String,
    /// What is being analyzed.
    pub input: String,
    /// Status.
    pub status: JobStatus,
    /// Stage in progress.
    pub stage: Option<String>,
    /// Stages finished so far, with their outcome.
    pub stages: Vec<(String, String)>,
    /// Files processed in the current stage.
    pub files_done: usize,
    /// Files in the current stage.
    pub files_total: usize,
    /// Stored repository, when finished.
    pub repository_id: Option<String>,
    /// Stored analysis, when finished.
    pub scan_id: Option<String>,
    /// Error message, when failed.
    pub error: Option<String>,
    /// Warnings that did not stop the analysis.
    pub warnings: Vec<String>,
    /// When the job started.
    pub started_at: Timestamp,
    /// When it finished.
    pub finished_at: Option<Timestamp>,
}

struct Job {
    state: Mutex<JobState>,
    cancel: CancellationToken,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// All jobs of one server.
#[derive(Default)]
pub struct Jobs {
    jobs: Mutex<VecDeque<Arc<Job>>>,
}

impl Jobs {
    /// Starts analyzing `options.input` in the background. Fails when too many analyses
    /// are already running.
    pub fn start(&self, paths: AppPaths, options: AnalyzeOptions) -> Result<JobState, String> {
        let input = options.display_input();
        self.start_worker(input, move |job| run(&job, &paths, &options))
    }

    fn start_worker(
        &self,
        input: String,
        worker: impl FnOnce(Arc<Job>) + Send + 'static,
    ) -> Result<JobState, String> {
        let mut jobs = lock(&self.jobs);
        let running = jobs
            .iter()
            .filter(|job| lock(&job.state).status == JobStatus::Running)
            .count();
        if running >= MAX_RUNNING {
            return Err(format!(
                "{running} analyses are already running; try again when one finishes"
            ));
        }
        // IDs are opaque and unique across process restarts. A stale UI must
        // never accidentally attach to a different job with the same ordinal.
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes)
            .map_err(|_| "cannot allocate a unique analysis job identifier".to_owned())?;
        let id = format!("job-{}", to_hex(&bytes));
        let job = Arc::new(Job {
            state: Mutex::new(JobState {
                id,
                input,
                status: JobStatus::Running,
                stage: None,
                stages: Vec::new(),
                files_done: 0,
                files_total: 0,
                repository_id: None,
                scan_id: None,
                error: None,
                warnings: Vec::new(),
                started_at: Timestamp::now(),
                finished_at: None,
            }),
            cancel: CancellationToken::new(),
        });
        let initial = lock(&job.state).clone();
        let worker_job = Arc::clone(&job);
        std::thread::Builder::new()
            .name("repodna-analysis".to_owned())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| worker(Arc::clone(&worker_job))));
                // A panicking worker or one that returns without finalizing must
                // not permanently consume one of the limited scan slots.
                let mut state = lock(&worker_job.state);
                if state.status == JobStatus::Running {
                    state.status = JobStatus::Failed;
                    state.stage = None;
                    state.finished_at = Some(Timestamp::now());
                    state.error = Some(if result.is_err() {
                        "The analysis worker stopped unexpectedly.".to_owned()
                    } else {
                        "The analysis worker returned without a result.".to_owned()
                    });
                }
            })
            .map_err(|error| format!("cannot start an analysis worker: {error}"))?;
        jobs.push_back(job);
        while jobs.len() > KEEP_FINISHED {
            match jobs
                .iter()
                .position(|job| lock(&job.state).status != JobStatus::Running)
            {
                Some(index) => {
                    jobs.remove(index);
                }
                None => break,
            }
        }
        Ok(initial)
    }

    fn find(&self, id: &str) -> Option<Arc<Job>> {
        lock(&self.jobs)
            .iter()
            .find(|job| lock(&job.state).id == id)
            .cloned()
    }

    /// A job's state.
    pub fn get(&self, id: &str) -> Option<JobState> {
        self.find(id).map(|job| lock(&job.state).clone())
    }

    /// Recent jobs, newest first.
    pub fn list(&self) -> Vec<JobState> {
        lock(&self.jobs)
            .iter()
            .rev()
            .map(|job| lock(&job.state).clone())
            .collect()
    }

    /// Asks a job to stop. Returns `false` when there is no such job.
    pub fn cancel(&self, id: &str) -> bool {
        match self.find(id) {
            Some(job) => {
                job.cancel.cancel();
                true
            }
            None => false,
        }
    }

    /// Asks every job to stop.
    pub fn cancel_all(&self) {
        for job in lock(&self.jobs).iter() {
            job.cancel.cancel();
        }
    }
}

/// Job polling exposes a bounded summary. Full analysis warnings still belong
/// to the analysis result, not an ever-growing progress response.
fn record_warning(warnings: &mut Vec<String>, text: &str) {
    if warnings.last().is_some_and(|last| last == OMITTED_WARNINGS) {
        return;
    }
    let text = if text.len() > MAX_WARNING_BYTES {
        let mut end = MAX_WARNING_BYTES - 3;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &text[..end])
    } else {
        text.to_owned()
    };
    if warnings.contains(&text) {
        return;
    }
    // Reserve one slot for the explicit omission notice.
    if warnings.len() == MAX_WARNINGS - 1 {
        warnings.push(OMITTED_WARNINGS.to_owned());
    } else {
        warnings.push(text);
    }
}

fn run(job: &Arc<Job>, paths: &AppPaths, options: &AnalyzeOptions) {
    let observer = Arc::clone(job);
    let progress = Progress::new(move |event| {
        let mut state = lock(&observer.state);
        if state.status != JobStatus::Running {
            return;
        }
        match event {
            ProgressEvent::StageStarted(stage) => {
                state.stage = Some(stage.label().to_owned());
                state.files_done = 0;
                state.files_total = 0;
            }
            ProgressEvent::Files { done, total } => {
                state.files_done = *done;
                state.files_total = *total;
            }
            ProgressEvent::StageFinished { stage, status, .. } => {
                if *status != AnalyzerStatus::Skipped {
                    let outcome = serde_json::to_value(status)
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_owned))
                        .unwrap_or_default();
                    state.stages.push((stage.label().to_owned(), outcome));
                }
                state.stage = None;
            }
            ProgressEvent::Message(text) => record_warning(&mut state.warnings, text),
        }
    });
    let result = run_analysis(paths, options, progress, &job.cancel);
    let mut state = lock(&job.state);
    state.stage = None;
    state.finished_at = Some(Timestamp::now());
    match result {
        Ok(outcome) => {
            state.status = JobStatus::Completed;
            for warning in outcome.warnings {
                record_warning(&mut state.warnings, &warning);
            }
            if let Some(scan) = outcome.scan {
                state.repository_id = Some(scan.repository_id);
                state.scan_id = Some(scan.id);
            }
        }
        Err(error) if error.kind == ErrorKind::Cancelled => {
            state.status = JobStatus::Cancelled;
        }
        Err(error) => {
            state.status = JobStatus::Failed;
            state.error = Some(match error.hint {
                Some(hint) => format!("{} ({hint})", error.message),
                None => error.message,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_app::ConfigOptions;
    use std::time::{Duration, Instant};

    fn wait(jobs: &Jobs, id: &str) -> JobState {
        let started = Instant::now();
        loop {
            let state = jobs.get(id).unwrap();
            if state.status != JobStatus::Running || started.elapsed() > Duration::from_secs(60) {
                return state;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn panicking_and_unfinalized_workers_release_their_slots() {
        let jobs = Jobs::default();
        let first = jobs
            .start_worker("first".to_owned(), |_| panic!("synthetic worker panic"))
            .unwrap();
        let first = wait(&jobs, &first.id);
        assert_eq!(first.status, JobStatus::Failed);
        assert!(first.finished_at.is_some());
        assert_eq!(
            first.error.as_deref(),
            Some("The analysis worker stopped unexpectedly.")
        );
        let second = jobs.start_worker("second".to_owned(), |_| {}).unwrap();
        let second = wait(&jobs, &second.id);
        assert_eq!(second.status, JobStatus::Failed);
        assert_eq!(
            second.error.as_deref(),
            Some("The analysis worker returned without a result.")
        );
        assert_ne!(first.id, second.id);
    }

    #[test]
    fn concurrency_limit_and_cancel_do_not_evict_running_workers() {
        use std::sync::mpsc;

        let jobs = Jobs::default();
        let (release_first, first_gate) = mpsc::channel();
        let (release_second, second_gate) = mpsc::channel();
        let first = jobs
            .start_worker("first".to_owned(), move |job| {
                first_gate.recv().unwrap();
                assert!(job.cancel.is_cancelled());
            })
            .unwrap();
        let second = jobs
            .start_worker("second".to_owned(), move |_| {
                second_gate.recv().unwrap();
            })
            .unwrap();
        let error = jobs.start_worker("third".to_owned(), |_| {}).unwrap_err();
        assert!(error.contains("already running"));
        assert!(jobs.cancel(&first.id));
        assert_eq!(jobs.get(&first.id).unwrap().status, JobStatus::Running);
        assert_eq!(jobs.list().len(), MAX_RUNNING);
        release_first.send(()).unwrap();
        release_second.send(()).unwrap();
        assert_eq!(wait(&jobs, &first.id).status, JobStatus::Failed);
        assert_eq!(wait(&jobs, &second.id).status, JobStatus::Failed);
        let next = jobs.start_worker("next".to_owned(), |_| {}).unwrap();
        assert_eq!(wait(&jobs, &next.id).status, JobStatus::Failed);
    }

    #[test]
    fn independent_registries_do_not_reuse_job_ids() {
        let first = Jobs::default();
        let second = Jobs::default();
        let a = first.start_worker("one".to_owned(), |_| {}).unwrap();
        let b = second.start_worker("two".to_owned(), |_| {}).unwrap();
        assert_ne!(a.id, b.id);
        assert!(first.get(&b.id).is_none());
        assert!(second.get(&a.id).is_none());
        assert!(a.id.starts_with("job-"));
        assert_eq!(a.id.len(), 36);
    }

    #[test]
    fn retains_bounded_unique_warning_text_with_valid_unicode() {
        let mut warnings = Vec::new();
        let long = "界".repeat(MAX_WARNING_BYTES);
        record_warning(&mut warnings, &long);
        record_warning(&mut warnings, &long);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].len() <= MAX_WARNING_BYTES);
        assert!(warnings[0].ends_with("..."));
        for index in 0..MAX_WARNINGS * 2 {
            record_warning(&mut warnings, &format!("warning {index}"));
        }
        assert_eq!(warnings.len(), MAX_WARNINGS);
        assert_eq!(warnings.last().map(String::as_str), Some(OMITTED_WARNINGS));
        let snapshot = warnings.clone();
        record_warning(&mut warnings, "one more");
        assert_eq!(warnings, snapshot);
    }

    #[test]
    fn bounded_history_removes_oldest_finished_job() {
        let jobs = Jobs::default();
        let mut ids = Vec::new();
        for index in 0..KEEP_FINISHED + 3 {
            let job = jobs
                .start_worker(format!("job {index}"), |_| {})
                .unwrap();
            assert_eq!(wait(&jobs, &job.id).status, JobStatus::Failed);
            ids.push(job.id);
        }
        assert_eq!(jobs.list().len(), KEEP_FINISHED);
        assert!(jobs.get(&ids[0]).is_none());
        assert!(jobs.get(&ids[2]).is_none());
        assert!(jobs.get(&ids[3]).is_some());
        assert_eq!(jobs.list()[0].id, *ids.last().unwrap());
    }

    #[test]
    fn malformed_remote_is_not_exposed_in_job_state_or_failure() {
        let home = tempfile::tempdir().unwrap();
        let paths = AppPaths::in_directory(home.path());
        let options = AnalyzeOptions {
            input: concat!(
                "https://github.com/org/repo/tree/feature/runtime",
                "https://user:synthetic-secret@github.com/org/repo"
            )
            .to_owned(),
            no_store: true,
            config: ConfigOptions {
                ignore_user_config: true,
                ..ConfigOptions::default()
            },
            ..AnalyzeOptions::default()
        };
        let jobs = Jobs::default();
        let started = jobs.start(paths, options).unwrap();
        assert_eq!(started.input, "invalid remote repository URL");
        assert!(
            !serde_json::to_string(&started)
                .unwrap()
                .contains("synthetic-secret")
        );
        let finished = wait(&jobs, &started.id);
        assert_eq!(finished.status, JobStatus::Failed);
        assert!(
            finished
                .error
                .as_deref()
                .unwrap()
                .contains("multiple repository URLs"),
            "{:?}",
            finished.error
        );
        let serialized = serde_json::to_string(&jobs.list()).unwrap();
        assert!(!serialized.contains("synthetic-secret"));
    }

    #[test]
    fn runs_analyses_in_the_background() {
        let home = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        repodna_testkit::write_tree(repo.path(), &[("main.py", "print('hi')\n")]);
        let paths = AppPaths::in_directory(home.path());
        let options = AnalyzeOptions {
            input: repo.path().to_string_lossy().into_owned(),
            config: ConfigOptions {
                ignore_user_config: true,
                ..ConfigOptions::default()
            },
            ..AnalyzeOptions::default()
        };
        let jobs = Jobs::default();
        let started = jobs.start(paths.clone(), options.clone()).unwrap();
        assert_eq!(started.status, JobStatus::Running);
        let finished = wait(&jobs, &started.id);
        assert_eq!(
            finished.status,
            JobStatus::Completed,
            "{:?}",
            finished.error
        );
        assert!(finished.scan_id.is_some());
        assert!(!finished.stages.is_empty());
        assert_eq!(jobs.list().len(), 1);

        let missing = AnalyzeOptions {
            input: home.path().join("missing").to_string_lossy().into_owned(),
            ..options
        };
        let failed = jobs.start(paths, missing).unwrap();
        let failed = wait(&jobs, &failed.id);
        assert_eq!(failed.status, JobStatus::Failed);
        assert!(failed.error.unwrap().contains("does not exist"));
        assert!(!jobs.cancel("job-999"));
    }
}
