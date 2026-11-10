//! Repository state: HEAD, branches, tags, remotes, and working-tree status.

use std::path::{Path, PathBuf};

use repodna_core::CancellationToken;
use repodna_core::Timestamp;
use repodna_core::model::git::{BranchInfo, TagInfo};

use crate::error::GitError;
use crate::runner::GitRunner;

const FIELD: char = '\u{1f}';

/// Basic facts about a repository.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepositoryState {
    /// Commit checked out, `None` for a repository without commits.
    pub head: Option<String>,
    /// Branch checked out, `None` when HEAD is detached.
    pub current_branch: Option<String>,
    /// Default branch, when it can be determined.
    pub default_branch: Option<String>,
    /// `true` for shallow clones.
    pub shallow: bool,
    /// `true` when tracked files have uncommitted changes; `None` if unknown.
    pub dirty: Option<bool>,
    /// Remotes as `(name, url)`, URLs unsanitized (callers must sanitize before storing).
    pub remotes: Vec<(String, String)>,
}

/// Returns the top-level directory of the working tree containing `path`, or `None` if
/// `path` is not inside a Git repository.
pub fn repository_root(
    runner: &GitRunner,
    path: &Path,
    cancel: &CancellationToken,
) -> Result<Option<PathBuf>, GitError> {
    match runner.text(path, &["rev-parse", "--show-toplevel"], cancel) {
        Ok(root) if !root.is_empty() => Ok(Some(PathBuf::from(root))),
        Ok(_) | Err(GitError::NotARepository(_)) => Ok(None),
        Err(GitError::CommandFailed { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

fn optional_text(
    runner: &GitRunner,
    repo: &Path,
    args: &[&str],
    cancel: &CancellationToken,
) -> Result<Option<String>, GitError> {
    match runner.text(repo, args, cancel) {
        Ok(text) if !text.is_empty() => Ok(Some(text)),
        Ok(_) | Err(GitError::CommandFailed { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Reads HEAD, branch, shallow, dirty, and remote information.
pub fn read_state(
    runner: &GitRunner,
    repo: &Path,
    cancel: &CancellationToken,
) -> Result<RepositoryState, GitError> {
    let head = optional_text(
        runner,
        repo,
        &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"],
        cancel,
    )?;
    let current_branch = optional_text(
        runner,
        repo,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        cancel,
    )?;
    let origin_head = optional_text(
        runner,
        repo,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
        cancel,
    )?;
    let branches = read_branches(runner, repo, cancel).unwrap_or_default();
    let has_branch = |name: &str| branches.iter().any(|b| !b.remote && b.name == name);
    let default_branch = origin_head
        .map(|name| name.strip_prefix("origin/").unwrap_or(&name).to_owned())
        .or_else(|| {
            ["main", "master", "trunk", "develop"]
                .into_iter()
                .find(|n| has_branch(n))
                .map(str::to_owned)
        })
        .or_else(|| current_branch.clone());
    let shallow = optional_text(
        runner,
        repo,
        &["rev-parse", "--is-shallow-repository"],
        cancel,
    )?
    .as_deref()
        == Some("true");
    // We only need to know if Git reported any tracked changes. Do not
    // allocate the full porcelain output; the runner drains the remainder
    // after the consumer returns, so Git cannot block on a full stdout pipe.
    let mut has_tracked_changes = false;
    let dirty = match runner.stream(
        Some(repo),
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=no",
            "--ignore-submodules=all",
            "-z",
        ],
        cancel,
        |reader| {
            has_tracked_changes = !reader.fill_buf().map_err(GitError::Io)?.is_empty();
            Ok(())
        },
    ) {
        Ok(()) => Some(has_tracked_changes),
        Err(GitError::Cancelled) => return Err(GitError::Cancelled),
        Err(_) => None,
    };
    let remotes = match runner.text(
        repo,
        &["config", "--get-regexp", r"^remote\..*\.url$"],
        cancel,
    ) {
        Ok(text) => text
            .lines()
            .filter_map(|line| {
                let (key, url) = line.split_once(' ')?;
                let name = key.strip_prefix("remote.")?.strip_suffix(".url")?;
                Some((name.to_owned(), url.trim().to_owned()))
            })
            .collect(),
        Err(GitError::Cancelled) => return Err(GitError::Cancelled),
        Err(_) => Vec::new(),
    };
    Ok(RepositoryState {
        head,
        current_branch,
        default_branch,
        shallow,
        dirty,
        remotes,
    })
}

/// Visit the lines emitted by a Git ref-listing command without retaining all
/// of its raw text. Lossy UTF-8 decoding matches the previous GitRunner::text
/// behavior, and Git's newline-terminated record format is preserved.
fn stream_ref_lines(
    runner: &GitRunner,
    repo: &Path,
    args: &[&str],
    cancel: &CancellationToken,
    mut consume: impl FnMut(&str),
) -> Result<(), GitError> {
    runner.stream(Some(repo), args, cancel, |reader| {
        let mut line = Vec::new();
        loop {
            line.clear();
            if reader.read_until(b'\n', &mut line).map_err(GitError::Io)? == 0 {
                break;
            }
            if line.last() == Some(&b'\n') {
                line.pop();
            }
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            consume(String::from_utf8_lossy(&line).as_ref());
        }
        Ok(())
    })
}

fn parse_branch_line(line: &str) -> Option<BranchInfo> {
    let mut fields = line.split(FIELD);
    let reference = fields.next()?;
    let head = fields.next()?.to_owned();
    let updated = Timestamp::from_unix(fields.next()?.trim().parse().ok()?);
    // A branch such as "feature/HEAD" is legal. Only ignore real symbolic
    // references (for example, origin/HEAD -> origin/main).
    if !fields.next()?.is_empty() {
        return None;
    }
    if let Some(name) = reference.strip_prefix("refs/heads/") {
        Some(BranchInfo {
            name: name.to_owned(),
            remote: false,
            head,
            updated,
        })
    } else {
        reference
            .strip_prefix("refs/remotes/")
            .map(|name| BranchInfo {
                name: name.to_owned(),
                remote: true,
                head,
                updated,
            })
    }
}

/// Reads local and remote-tracking branches.
pub fn read_branches(
    runner: &GitRunner,
    repo: &Path,
    cancel: &CancellationToken,
) -> Result<Vec<BranchInfo>, GitError> {
    let mut branches = Vec::new();
    stream_ref_lines(
        runner,
        repo,
        &[
            "for-each-ref",
            "--format=%(refname)%1f%(objectname)%1f%(committerdate:unix)%1f%(symref)",
            "refs/heads",
            "refs/remotes",
        ],
        cancel,
        |line| {
            if let Some(branch) = parse_branch_line(line) {
                branches.push(branch);
            }
        },
    )?;
    branches.sort_by(|a, b| a.remote.cmp(&b.remote).then_with(|| a.name.cmp(&b.name)));
    Ok(branches)
}

fn parse_tag_line(line: &str) -> Option<TagInfo> {
    let fields: Vec<&str> = line.split(FIELD).collect();
    if fields.len() < 5 {
        return None;
    }
    let annotated = fields[1] == "tag";
    let commit = if annotated && !fields[3].is_empty() {
        fields[3]
    } else {
        fields[2]
    };
    Some(TagInfo {
        name: fields[0].to_owned(),
        commit: commit.to_owned(),
        date: Timestamp::from_unix(fields[4].trim().parse().unwrap_or(0)),
        annotated,
    })
}

/// Reads tags, peeling annotated tags to the commit they point to.
pub fn read_tags(
    runner: &GitRunner,
    repo: &Path,
    cancel: &CancellationToken,
) -> Result<Vec<TagInfo>, GitError> {
    let mut tags = Vec::new();
    stream_ref_lines(
        runner,
        repo,
        &[
            "for-each-ref",
            "--format=%(refname:short)%1f%(objecttype)%1f%(objectname)%1f%(*objectname)%1f%(creatordate:unix)",
            "refs/tags",
        ],
        cancel,
        |line| {
            if let Some(tag) = parse_tag_line(line) {
                tags.push(tag);
            }
        },
    )?;
    tags.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.name.cmp(&b.name)));
    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_testkit::{GitRepo, git_available};

    #[test]
    fn ref_line_parsers_ignore_malformed_records() {
        assert!(parse_branch_line("").is_none());
        assert!(parse_branch_line("refs/heads/main").is_none());
        assert!(parse_branch_line("refs/remotes/origin/HEAD\u{1f}123\u{1f}0\u{1f}refs/remotes/origin/main").is_none());
        let branch = parse_branch_line("refs/heads/feature/long\u{1f}abc123\u{1f}1710000000\u{1f}")
            .expect("valid branch");
        assert_eq!(branch.name, "feature/long");
        assert!(!branch.remote);
        // "HEAD" inside a branch name does not make it a symbolic ref.
        let legal_head_suffix =
            parse_branch_line("refs/heads/feature/HEAD\u{1f}abc123\u{1f}1710000000\u{1f}")
                .expect("legal branch ending in HEAD");
        assert_eq!(legal_head_suffix.name, "feature/HEAD");
        assert!(!legal_head_suffix.remote);
        assert_eq!(branch.head, "abc123");
        let remote = parse_branch_line("refs/remotes/origin/topic\u{1f}deadbeef\u{1f}1710000000\u{1f}")
            .expect("valid remote branch");
        assert_eq!(remote.name, "origin/topic");
        assert!(remote.remote);

        assert!(parse_tag_line("").is_none());
        assert!(parse_tag_line("v1.0.0\u{1f}tag").is_none());
        let tag = parse_tag_line("v1.0.0\u{1f}tag\u{1f}tagobj\u{1f}commitobj\u{1f}1710000000")
            .expect("valid annotated tag");
        assert_eq!(tag.commit, "commitobj");
        assert!(tag.annotated);
        let lightweight = parse_tag_line("v1.0.1\u{1f}commit\u{1f}commit2\u{1f}\u{1f}1710000000")
            .expect("valid lightweight tag");
        assert_eq!(lightweight.commit, "commit2");
        assert!(!lightweight.annotated);
    }

    #[test]
    fn reads_state_branches_and_tags() {
        if !git_available() {
            return;
        }
        let runner = GitRunner::detect().unwrap();
        let cancel = CancellationToken::new();
        let repo = GitRepo::new();
        repo.write("a.txt", "a");
        let first = repo.commit(
            "first",
            "Ada",
            "ada@example.invalid",
            "2024-01-01T00:00:00Z",
        );
        repo.tag("v0.1.0", None);
        repo.write("a.txt", "b");
        let second = repo.commit(
            "second",
            "Ada",
            "ada@example.invalid",
            "2024-02-01T00:00:00Z",
        );
        repo.tag("v1.0.0", Some("2024-02-02T00:00:00Z"));
        repo.git(&["branch", "feature"]);
        repo.git(&["branch", "feature/HEAD"]);
        repo.git(&["symbolic-ref", "refs/remotes/origin/HEAD", "refs/heads/main"]);
        repo.git(&[
            "remote",
            "add",
            "origin",
            "https://user:secret@example.com/o/r.git",
        ]);

        let state = read_state(&runner, repo.path(), &cancel).unwrap();
        assert_eq!(state.head.as_deref(), Some(second.as_str()));
        assert_eq!(state.current_branch.as_deref(), Some("main"));
        assert_eq!(state.default_branch.as_deref(), Some("main"));
        assert!(!state.shallow);
        assert_eq!(state.dirty, Some(false));
        assert_eq!(state.remotes.len(), 1);
        repo.write("a.txt", "dirty");
        assert_eq!(
            read_state(&runner, repo.path(), &cancel).unwrap().dirty,
            Some(true)
        );

        let branches = read_branches(&runner, repo.path(), &cancel).unwrap();
        let names: Vec<_> = branches.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, vec!["feature", "feature/HEAD", "main"]);
        assert!(branches.iter().all(|branch| !branch.remote));

        let tags = read_tags(&runner, repo.path(), &cancel).unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "v0.1.0");
        assert_eq!(tags[0].commit, first);
        assert!(!tags[0].annotated);
        assert_eq!(tags[1].commit, second, "annotated tags are peeled");
        assert!(tags[1].annotated);
        assert_eq!(tags[1].date.date_string(), "2024-02-02");

        let root = repository_root(&runner, &repo.path().join("."), &cancel).unwrap();
        assert!(root.is_some());
    }

    #[test]
    fn dirty_status_ignores_untracked_files_and_handles_many_tracked_changes() {
        if !git_available() {
            return;
        }
        let runner = GitRunner::detect().unwrap();
        let cancel = CancellationToken::new();
        let repo = GitRepo::new();
        for index in 0..96 {
            repo.write(&format!("tracked/file-{index:03}.txt"), "clean");
        }
        repo.commit(
            "tracked files",
            "Ada",
            "ada@example.invalid",
            "2024-01-01T00:00:00Z",
        );

        repo.write("not-tracked.txt", "untracked");
        let clean = read_state(&runner, repo.path(), &cancel).unwrap();
        assert_eq!(clean.dirty, Some(false));

        for index in 0..96 {
            repo.write(&format!("tracked/file-{index:03}.txt"), "modified");
        }
        let dirty = read_state(&runner, repo.path(), &cancel).unwrap();
        assert_eq!(dirty.dirty, Some(true));
    }

    #[test]
    fn handles_empty_repositories_and_plain_directories() {
        if !git_available() {
            return;
        }
        let runner = GitRunner::detect().unwrap();
        let cancel = CancellationToken::new();
        let repo = GitRepo::new();
        let state = read_state(&runner, repo.path(), &cancel).unwrap();
        assert!(state.head.is_none());
        let plain = tempfile::tempdir().unwrap();
        assert!(
            repository_root(&runner, plain.path(), &cancel)
                .unwrap()
                .is_none()
        );
    }
}
