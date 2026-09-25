//! Analyses started through the API run in background threads; clients poll their state.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use repodna_app::{AnalyzeOptions, AppPaths, ErrorKind, run_analysis};
use repodna_core::CancellationToken;
use repodna_core::model::metadata::AnalyzerStatus;
use repodna_core::time::Timestamp;
use repodna_engine::{Progress, ProgressEvent};
use serde::Serialize;

/// Most analyses running at once.
pub const MAX_RUNNING: usize = 2;
/// Finished jobs kept for polling.
const KEEP_FINISHED: usize = 50;

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
    next: AtomicU64,
}

impl Jobs {
    /// Starts analyzing `options.input` in the background. Fails when too many analyses
    /// are already running.
    pub fn start(&self, paths: AppPaths, options: AnalyzeOptions) -> Result<JobState, String> {
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
        let number = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let job = Arc::new(Job {
            state: Mutex::new(JobState {
                id: format!("job-{number}"),
                input: options.input.clone(),
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
        jobs.push_back(Arc::clone(&job));
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
        drop(jobs);
        let initial = lock(&job.state).clone();
        std::thread::spawn(move || run(&job, &paths, &options));
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

fn run(job: &Arc<Job>, paths: &AppPaths, options: &AnalyzeOptions) {
    let observer = Arc::clone(job);
    let progress = Progress::new(move |event| {
        let mut state = lock(&observer.state);
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
            ProgressEvent::Message(text) => state.warnings.push(text.clone()),
        }
    });
    let result = run_analysis(paths, options, progress, &job.cancel);
    let mut state = lock(&job.state);
    state.stage = None;
    state.finished_at = Some(Timestamp::now());
    match result {
        Ok(outcome) => {
            state.status = JobStatus::Completed;
            state.warnings.extend(outcome.warnings);
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
