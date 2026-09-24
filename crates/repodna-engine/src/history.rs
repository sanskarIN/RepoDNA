//! The Git stage: repository state, commit history, contributors, and releases.

use std::collections::BTreeSet;
use std::path::Path;

use repodna_core::cancel::CancellationToken;
use repodna_core::config::Config;
use repodna_core::model::git::{CommitRef, GitReport, ReleaseInfo};
use repodna_core::model::identity::RemoteInfo;
use repodna_core::time::Timestamp;
use repodna_git::url::parse_remote;
use repodna_git::{
    GitError, GitRunner, History, HistoryOptions, LogOptions, RepositoryState, aggregate,
    read_branches, read_history, read_state, read_tags, releases,
};

/// Results of the Git stage.
#[derive(Debug)]
pub struct GitStage {
    /// The Git section (hotspots are added later).
    pub report: GitReport,
    /// The commit history, kept for evolution analysis.
    pub history: History,
    /// Repository state.
    pub state: RepositoryState,
    /// Remotes with credentials removed.
    pub remotes: Vec<RemoteInfo>,
    /// Releases recognized from tags.
    pub releases: Vec<ReleaseInfo>,
}

/// Replaces contributor names with numbered pseudonyms, ordered by commits.
pub fn anonymize_contributors(report: &mut GitReport) {
    for (index, contributor) in report.contributors.iter_mut().enumerate() {
        contributor.name = format!("Contributor {}", index + 1);
    }
}

/// Runs the Git stage for the working tree at `root`.
pub fn run_git_stage(
    git: &GitRunner,
    root: &Path,
    current_files: &BTreeSet<String>,
    config: &Config,
    reference_time: Timestamp,
    cancel: &CancellationToken,
) -> Result<GitStage, GitError> {
    let state = read_state(git, root, cancel)?;
    let history = read_history(
        git,
        root,
        LogOptions {
            max_commits: config.analysis.max_commits,
            detect_renames: true,
        },
        cancel,
    )?;
    let thresholds = &config.thresholds;
    let mut report = aggregate(
        &history,
        current_files,
        &HistoryOptions {
            reference_time,
            recent_days: thresholds.recent_days,
            dormant_days: thresholds.dormant_days,
            artifact_commits: usize::try_from(config.analysis.artifact_commits)
                .unwrap_or(usize::MAX),
            include_messages: config.privacy.include_commit_messages,
        },
    );
    let tags = read_tags(git, root, cancel)?;
    let releases = releases(&tags, &history);
    report.git_version = Some(git.version().to_owned());
    report.current_branch = state.current_branch.clone();
    report.default_branch = state.default_branch.clone();
    report.shallow = state.shallow;
    report.history_truncated = history.truncated;
    report.head = history.commits.first().map(|commit| CommitRef {
        hash: commit.hash.clone(),
        short: commit.hash.chars().take(7).collect(),
        timestamp: Timestamp::from_unix(commit.timestamp),
    });
    report.branches = read_branches(git, root, cancel)?;
    report.tags = tags;
    report.releases = releases.clone();
    if history.truncated {
        report.notes.push(format!(
            "History was limited to the most recent {} commits.",
            config.analysis.max_commits
        ));
    }
    if state.shallow {
        report
            .notes
            .push("The repository is a shallow clone; older history is not available.".to_owned());
    }
    if config.privacy.anonymize_contributors {
        anonymize_contributors(&mut report);
    }
    let remotes = state
        .remotes
        .iter()
        .map(|(name, url)| {
            let parsed = parse_remote(url);
            RemoteInfo {
                name: name.clone(),
                url: parsed.url,
                host: parsed.host,
                provider: parsed.provider,
            }
        })
        .collect();
    Ok(GitStage {
        report,
        history,
        state,
        remotes,
        releases,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_testkit::{GitRepo, git_available};

    #[test]
    fn reads_history_state_and_releases() {
        if !git_available() {
            return;
        }
        let repo = GitRepo::new();
        repo.write("src/lib.rs", "pub fn a() {}\n");
        repo.commit("initial", "Ana", "ana@example.test", "2024-01-01T10:00:00Z");
        repo.write("src/lib.rs", "pub fn a() {}\npub fn b() {}\n");
        repo.commit("add b", "Bo", "bo@example.test", "2024-02-01T10:00:00Z");
        repo.tag("v1.0.0", None);
        repo.git(&[
            "remote",
            "add",
            "origin",
            "https://user:pass@github.com/acme/widget.git",
        ]);
        let git = GitRunner::detect().unwrap();
        let files: BTreeSet<String> = ["src/lib.rs".to_owned()].into();
        let mut config = Config::default();
        config.privacy.anonymize_contributors = true;
        let stage = run_git_stage(
            &git,
            repo.path(),
            &files,
            &config,
            Timestamp::from_ymd(2024, 3, 1).unwrap(),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(stage.report.commit_count, 2);
        assert_eq!(stage.history.commits.len(), 2);
        assert_eq!(stage.releases.len(), 1);
        assert_eq!(stage.report.releases[0].tag, "v1.0.0");
        assert!(stage.report.head.is_some());
        assert_eq!(stage.report.contributors.len(), 2);
        assert!(
            stage
                .report
                .contributors
                .iter()
                .all(|c| c.name.starts_with("Contributor "))
        );
        assert_eq!(stage.remotes[0].url, "https://github.com/acme/widget.git");
        assert_eq!(stage.remotes[0].provider, "github");
    }
}
