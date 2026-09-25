//! The Git stage: repository state, commit history, contributors, and releases.

use std::collections::BTreeSet;
use std::path::Path;

use repodna_core::cancel::CancellationToken;
use repodna_core::confidence::Confidence;
use repodna_core::config::{Config, Thresholds};
use repodna_core::evidence::Evidence;
use repodna_core::finding::{Finding, FindingCategory};
use repodna_core::model::git::{CommitRef, GitReport, ReleaseInfo};
use repodna_core::model::identity::RemoteInfo;
use repodna_core::severity::Severity;
use repodna_core::text::count;
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

/// High-churn areas reported (largest share first).
const MAX_CHURN_FINDINGS: usize = 3;

/// File changes needed in the recent window before churn shares are meaningful.
const MIN_RECENT_FILE_CHANGES: u32 = 10;

/// Commits needed before contributor concentration is reported.
const MIN_COMMITS_FOR_OWNERSHIP: u64 = 20;

/// Share of commits by one contributor at which concentration is reported.
const OWNERSHIP_SHARE: f64 = 0.75;

/// Derives findings from the Git section: areas with most of the recent churn, and commits
/// concentrated in one contributor identity. Wording describes history, not people.
pub fn git_findings(report: &GitReport, thresholds: &Thresholds) -> Vec<Finding> {
    let mut findings = Vec::new();
    let recent_changes: u32 = report
        .file_history
        .iter()
        .map(|record| record.recent_commits)
        .sum();
    let active_areas = report
        .directory_activity
        .iter()
        .filter(|area| area.recent_churn_share > 0.0)
        .count();
    if recent_changes >= MIN_RECENT_FILE_CHANGES && active_areas >= 2 {
        // Directory activity lists parents and their children (`crates` and `crates/core`);
        // only the most specific areas are compared, so a parent holding everything does
        // not trivially qualify.
        let is_parent = |path: &str| {
            let prefix = format!("{path}/");
            report
                .directory_activity
                .iter()
                .any(|other| other.path.starts_with(&prefix))
        };
        let mut areas: Vec<_> = report
            .directory_activity
            .iter()
            .filter(|area| area.recent_churn_share >= thresholds.high_churn_share)
            .filter(|area| area.path == "(root)" || !is_parent(&area.path))
            .collect();
        areas.sort_by(|a, b| {
            b.recent_churn_share
                .total_cmp(&a.recent_churn_share)
                .then_with(|| a.path.cmp(&b.path))
        });
        for area in areas.into_iter().take(MAX_CHURN_FINDINGS) {
            let root = area.path == "(root)";
            let place = if root {
                "files at the repository root".to_owned()
            } else {
                format!("{}/", area.path)
            };
            let percent = area.recent_churn_share * 100.0;
            let mut finding = Finding::new("activity.high-churn-area", &area.path, FindingCategory::Activity, Severity::Info, Confidence::High, format!("{percent:.0}% of recent changes are in {place}"))
                .summary(format!("{percent:.0}% of the lines added or deleted in the last {} days of history touched {place}.", thresholds.recent_days))
                .rationale("Areas under heavy change are where reviews, tests, and documentation matter most right now, and where parallel work is most likely to conflict.")
                .method("Lines added plus deleted per directory (first two levels) in the recent window, divided by all churn in that window.")
                .evidence(Evidence::metric_with_threshold("git.directory.recent-churn-share", area.recent_churn_share, thresholds.high_churn_share, "ratio"))
                .limitation("Mechanical changes such as renames, formatting, and regenerated files inflate churn.")
                .next_step("Check that tests and documentation for this area keep up with the changes.");
            if !root {
                finding = finding
                    .evidence(Evidence::directory(area.path.as_str()))
                    .path(area.path.clone());
            }
            findings.push(finding);
        }
    }
    let ownership = &report.ownership;
    if report.commit_count >= MIN_COMMITS_FOR_OWNERSHIP
        && ownership.top_contributor_share >= OWNERSHIP_SHARE
    {
        let title = if ownership.contributors <= 1 {
            "All commits come from one contributor identity".to_owned()
        } else {
            format!(
                "{:.0}% of commits come from one contributor identity",
                ownership.top_contributor_share * 100.0
            )
        };
        findings.push(
            Finding::new("contributors.concentration", "repository", FindingCategory::Contributors, Severity::Info, Confidence::High, title)
                .summary(if ownership.contributors <= 1 {
                    format!("One contributor identity made {}.", count(report.commit_count, "commit", "commits"))
                } else {
                    format!(
                        "{} made {}; {} of them authored at least half.",
                        count(u64::from(ownership.contributors), "contributor identity", "contributor identities"),
                        count(report.commit_count, "commit", "commits"),
                        ownership.contributors_for_half_of_commits
                    )
                })
                .rationale("When most changes come from one person, knowledge of the code is likely concentrated. This describes the history, not the quality of anyone's work.")
                .method("Commits per contributor identity (grouped by e-mail address after .mailmap) over the analyzed history.")
                .evidence(Evidence::metric_with_threshold("git.ownership.top-share", ownership.top_contributor_share, OWNERSHIP_SHARE, "ratio"))
                .limitation("Squash merges, pair programming, bots, and people using several e-mail addresses distort attribution.")
                .next_step("If continuity matters, document the key areas and spread reviews across more people."),
        );
    }
    findings
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
    // A repository without commits has no HEAD to read history from.
    let history = if state.head.is_some() {
        read_history(
            git,
            root,
            LogOptions {
                max_commits: config.analysis.max_commits,
                detect_renames: true,
            },
            cancel,
        )?
    } else {
        History::default()
    };
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

    #[test]
    fn derives_churn_and_ownership_findings() {
        use repodna_core::model::git::{DirectoryActivity, FileHistoryRecord, OwnershipSummary};
        let area = |path: &str, share: f64| DirectoryActivity {
            path: path.into(),
            commits: 5,
            churn: 100,
            authors: 1,
            last_changed: Timestamp::UNIX_EPOCH,
            recent_churn_share: share,
        };
        let mut report = GitReport {
            commit_count: 40,
            directory_activity: vec![
                area("src", 0.9),
                area("src/api", 0.6),
                area("(root)", 0.35),
                area("docs", 0.05),
            ],
            ownership: OwnershipSummary {
                contributors: 3,
                contributors_for_half_of_commits: 1,
                top_contributor_share: 0.8,
                note: String::new(),
            },
            ..GitReport::default()
        };
        report.file_history = vec![FileHistoryRecord {
            path: "src/api/a.rs".into(),
            commits: 12,
            authors: 1,
            insertions: 10,
            deletions: 0,
            first_seen: Timestamp::UNIX_EPOCH,
            last_changed: Timestamp::UNIX_EPOCH,
            recent_commits: 12,
            previous_paths: Vec::new(),
        }];
        let findings = git_findings(&report, &Thresholds::default());
        let titles: Vec<&str> = findings.iter().map(|f| f.title.as_str()).collect();
        assert_eq!(
            titles,
            vec![
                "60% of recent changes are in src/api/",
                "35% of recent changes are in files at the repository root",
                "80% of commits come from one contributor identity"
            ]
        );
        assert_eq!(findings[0].paths, vec!["src/api"]);
        assert!(findings[1].paths.is_empty());

        report.file_history[0].recent_commits = 2;
        report.commit_count = 5;
        assert!(git_findings(&report, &Thresholds::default()).is_empty());
    }

    #[test]
    fn handles_repositories_without_commits() {
        if !git_available() {
            return;
        }
        let repo = GitRepo::new();
        repo.write("README.md", "# New\n");
        let git = GitRunner::detect().unwrap();
        let stage = run_git_stage(
            &git,
            repo.path(),
            &BTreeSet::new(),
            &Config::default(),
            Timestamp::from_ymd(2024, 3, 1).unwrap(),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(stage.report.commit_count, 0);
        assert!(stage.report.head.is_none());
        assert_eq!(stage.report.notes.len(), 1);
    }
}
