//! Cloning remote repositories for analysis.

use std::io::{BufRead, Read};
use std::path::Path;

use repodna_core::CancellationToken;

use crate::error::GitError;
use crate::runner::GitRunner;
use crate::url::{CloneTarget, RequestedRevision, UrlPolicy, resolve_clone_target};

/// Options for [`clone_repository`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CloneOptions {
    /// Limit history to this many commits (`--depth`). `None` clones full history.
    pub depth: Option<u32>,
    /// URL policy.
    pub policy: UrlPolicy,
    /// Explicit revision takes precedence over a GitHub browser URL selection.
    pub revision: Option<RequestedRevision>,
}

/// Assemble Git's checkout options separately from the URL and destination.
///
/// The explicit `--` keeps a remote URL from being interpreted as a Git flag.
fn clone_args<'a>(
    target: &'a CloneTarget,
    revision: Option<&'a RequestedRevision>,
    depth: Option<&'a str>,
    destination: &'a str,
) -> Vec<&'a str> {
    let mut args = vec![
        "-c",
        "core.symlinks=false",
        "clone",
        "--quiet",
        "--no-recurse-submodules",
    ];
    match revision {
        Some(RequestedRevision::Branch(name) | RequestedRevision::Tag(name)) => {
            args.extend(["--single-branch", "--branch", name.as_str()]);
            if matches!(revision, Some(RequestedRevision::Tag(_))) {
                args.push("--no-checkout");
            }
        }
        Some(RequestedRevision::Commit(_)) => args.push("--no-checkout"),
        None => {
            if let Some(branch) = &target.branch {
                args.extend(["--single-branch", "--branch", branch.as_str()]);
            }
        }
    }
    if let Some(depth) = depth {
        args.extend(["--depth", depth]);
    }
    args.extend(["--", &target.repository_url, destination]);
    args
}

// Limit memory even when the remote advertises hostile or unexpectedly many refs.
const MAX_REMOTE_REF_LINE: usize = 16 * 1024;
const MAX_REMOTE_REFS: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteTreeMatch {
    reference: String,
    directory: Option<String>,
    branch: bool,
}

/// GitHub /tree/ links cannot delimit slash-containing refs from directories.
/// Prefer the longest advertised ref; prefer a branch for equal tag/branch names.
fn consider_remote_ref(
    requested: &str,
    advertised: &str,
    best: &mut Option<RemoteTreeMatch>,
) {
    let (branch, reference) = if let Some(name) = advertised.strip_prefix("refs/heads/") {
        (true, name)
    } else if let Some(name) = advertised.strip_prefix("refs/tags/") {
        (false, name)
    } else {
        return;
    };
    let Some(remaining) = requested.strip_prefix(reference) else {
        return;
    };
    let directory = if remaining.is_empty() {
        None
    } else if let Some(directory) = remaining.strip_prefix('/') {
        if directory.is_empty() {
            return;
        }
        Some(directory.to_owned())
    } else {
        return;
    };
    if best.as_ref().is_none_or(|current| {
        reference.len() > current.reference.len()
            || (reference.len() == current.reference.len() && branch && !current.branch)
    }) {
        *best = Some(RemoteTreeMatch {
            reference: reference.to_owned(),
            directory,
            branch,
        });
    }
}

/// Resolve only advertised Git refs, with bounded, cancellable Git output.
fn discover_tree_ref(
    runner: &GitRunner,
    repository_url: &str,
    requested: &str,
    cancel: &CancellationToken,
) -> Result<RemoteTreeMatch, GitError> {
    let mut best = None;
    let mut total = 0usize;
    runner.stream(
        None,
        &["ls-remote", "--refs", "--heads", "--tags", "--", repository_url],
        cancel,
        |reader| {
            loop {
                let mut line = Vec::new();
                let size = reader
                    .take((MAX_REMOTE_REF_LINE + 1) as u64)
                    .read_until(b'\n', &mut line)
                    .map_err(GitError::Io)?;
                if size == 0 {
                    break;
                }
                total = total.saturating_add(size);
                if size > MAX_REMOTE_REF_LINE || total > MAX_REMOTE_REFS {
                    return Err(GitError::Parse(
                        "remote reference discovery exceeded its size limit; use --branch or --tag explicitly"
                            .to_owned(),
                    ));
                }
                let Ok(line) = std::str::from_utf8(&line) else {
                    continue;
                };
                let Some((object, reference)) =
                    line.trim_end_matches(['\r', '\n']).split_once('\t')
                else {
                    continue;
                };
                if matches!(object.len(), 40 | 64)
                    && object.bytes().all(|byte| byte.is_ascii_hexdigit())
                {
                    consider_remote_ref(requested, reference, &mut best);
                }
            }
            Ok(())
        },
    )?;
    best.ok_or_else(|| {
        GitError::InvalidUrl(
            "the GitHub /tree/ path does not match an advertised branch or tag; use a branch-root URL or select --branch/--tag explicitly"
                .to_owned(),
        )
    })
}

/// Require a real directory, never silently choose a shorter ref for a typo.
fn verify_tree_directory(
    runner: &GitRunner,
    checkout: &Path,
    directory: &str,
    cancel: &CancellationToken,
) -> Result<(), GitError> {
    let entries = runner.text(
        checkout,
        &["ls-tree", "-d", "--name-only", "HEAD", "--", directory],
        cancel,
    )?;
    if entries.lines().any(|entry| entry == directory) {
        Ok(())
    } else {
        Err(GitError::InvalidUrl(
            "the GitHub /tree/ directory does not exist at the checked-out revision"
                .to_owned(),
        ))
    }
}

/// Clones `url` into `destination` (which must not exist or be empty).
///
/// The clone checks out symbolic links as plain files (`core.symlinks=false`) so nothing in
/// the analyzed tree can point outside it, skips submodules, and never prompts for
/// credentials. Private repositories work when credentials are configured for Git itself
/// (credential helper or SSH agent); RepoDNA never stores them.
pub fn clone_repository(
    runner: &GitRunner,
    url: &str,
    destination: &Path,
    options: CloneOptions,
    cancel: &CancellationToken,
) -> Result<(), GitError> {
    let mut target = resolve_clone_target(url, options.policy)?;
    let mut selected_directory = None;
    // Explicit selectors bypass remote discovery; branch-root URLs without
    // slashes retain the original clone path and its error reporting.
    if options.revision.is_none() {
        if let Some(candidate) = target.branch.as_deref().filter(|name| name.contains('/')) {
            let selected = discover_tree_ref(runner, &target.repository_url, candidate, cancel)?;
            selected_directory = selected.directory;
            target.branch = Some(selected.reference);
        }
    }
    let revision = options
        .revision
        .or_else(|| target.commit.clone().map(RequestedRevision::Commit));
    if let Some(revision) = &revision {
        revision.validate()?;
    }
    let destination_text = destination.to_string_lossy().into_owned();
    let depth = options.depth.map(|depth| depth.max(1).to_string());
    let args = clone_args(
        &target,
        revision.as_ref(),
        depth.as_deref(),
        &destination_text,
    );
    // Name the failure after what the user asked for, not after the options it ran with.
    runner
        .run_global(&args, cancel)
        .map_err(|error| match error {
            GitError::CommandFailed { status, stderr, .. } => GitError::CommandFailed {
                command: "git clone".to_owned(),
                status,
                stderr,
            },
            other => other,
        })?;

    // Explicit branch selection must remain on that branch. A similarly named
    // tag is not a substitute; Git's --branch accepts both by design.
    if let Some(RequestedRevision::Branch(expected)) = &revision {
        let actual = runner.text(
            destination,
            &["symbolic-ref", "--quiet", "--short", "HEAD"],
            cancel,
        )?;
        if actual != *expected {
            return Err(GitError::InvalidUrl(
                "the selected branch did not produce a branch checkout".to_owned(),
            ));
        }
    }
    if let Some(revision @ (RequestedRevision::Tag(_) | RequestedRevision::Commit(_))) = &revision {
        finish_detached_checkout(runner, destination, revision, cancel)?;
    }
    // A GitHub /tree/ URL can refer to a branch or a tag. Git clone --branch
    // handles both, and every successful clone must have a real commit at HEAD.
    runner.text(
        destination,
        &["rev-parse", "--verify", "HEAD^{commit}"],
        cancel,
    )?;
    if let Some(directory) = selected_directory.as_deref() {
        verify_tree_directory(runner, destination, directory, cancel)?;
    }
    Ok(())
}

/// Pin a tag or exact commit without attributing detached HEAD to a branch.
fn finish_detached_checkout(
    runner: &GitRunner,
    destination: &Path,
    revision: &RequestedRevision,
    cancel: &CancellationToken,
) -> Result<(), GitError> {
    let (reference, expected) = match revision {
        RequestedRevision::Tag(name) => {
            let reference = format!("refs/tags/{name}");
            let peeled = format!("{reference}^{{commit}}");
            let expected = runner
                .text(destination, &["rev-parse", "--verify", &peeled], cancel)
                .map_err(|error| match error {
                    GitError::CommandFailed { .. } => GitError::InvalidUrl(
                        "the selected tag is not available in the cloned repository".to_owned(),
                    ),
                    other => other,
                })?;
            (reference, expected)
        }
        RequestedRevision::Commit(sha) => {
            let peeled = format!("{sha}^{{commit}}");
            let expected = runner.text(destination, &["rev-parse", "--verify", &peeled], cancel)
                .map_err(|error| match error {
                    GitError::CommandFailed { .. } => GitError::InvalidUrl(
                        "the requested commit is not reachable from fetched references; try a branch containing it or a full-history clone".to_owned(),
                    ),
                    other => other,
                })?;
            (sha.clone(), expected)
        }
        RequestedRevision::Branch(_) => return Ok(()),
    };
    runner.stream(
        Some(destination),
        &[
            "-c",
            "core.symlinks=false",
            "checkout",
            "--force",
            "--detach",
            &reference,
        ],
        cancel,
        |reader| {
            std::io::copy(reader, &mut std::io::sink()).map_err(GitError::Io)?;
            Ok(())
        },
    )?;
    let actual = runner.text(
        destination,
        &["rev-parse", "--verify", "HEAD^{commit}"],
        cancel,
    )?;
    if actual != expected {
        return Err(GitError::InvalidUrl(
            "the checked-out commit does not match the selected revision".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_testkit::{GitRepo, git_available};

    #[test]
    fn tree_ref_discovery_prefers_longest_advertised_prefix() {
        let requested = "feat/audio/src/mixer";
        let mut best = None;
        for reference in [
            "refs/heads/feat",
            "refs/tags/feat/audio",
            "refs/heads/feat/audio",
            "refs/heads/feature",
            "refs/tags/unrelated",
        ] {
            consider_remote_ref(requested, reference, &mut best);
        }
        assert_eq!(
            best,
            Some(RemoteTreeMatch {
                reference: "feat/audio".to_owned(),
                directory: Some("src/mixer".to_owned()),
                branch: true,
            })
        );
        consider_remote_ref(requested, "refs/heads/feat/audio/src", &mut best);
        assert_eq!(
            best,
            Some(RemoteTreeMatch {
                reference: "feat/audio/src".to_owned(),
                directory: Some("mixer".to_owned()),
                branch: true,
            })
        );
        consider_remote_ref(requested, "refs/heads/feat/audio/src/mixer", &mut best);
        assert_eq!(
            best,
            Some(RemoteTreeMatch {
                reference: requested.to_owned(),
                directory: None,
                branch: true,
            })
        );
    }

    #[test]
    fn tree_ref_discovery_never_guesses_unadvertised_revisions() {
        let mut best = None;
        for reference in [
            "refs/heads/maintain",
            "refs/heads/main/subdir-more",
            "refs/tags/main2",
            "refs/pull/123/head",
        ] {
            consider_remote_ref("main/subdir", reference, &mut best);
        }
        assert_eq!(best, None);
        consider_remote_ref("main/subdir", "refs/tags/main", &mut best);
        assert_eq!(
            best,
            Some(RemoteTreeMatch {
                reference: "main".to_owned(),
                directory: Some("subdir".to_owned()),
                branch: false,
            })
        );
    }

    #[test]
    fn tree_directory_must_exist_at_selected_revision() {
        if !git_available() {
            return;
        }
        let repository = GitRepo::new();
        repository.write("src/engine/main.rs", "fn main() {}");
        repository.commit(
            "add source directory",
            "Test",
            "test@example.invalid",
            "2025-01-01T00:00:00Z",
        );
        let runner = GitRunner::detect().unwrap();
        let cancel = CancellationToken::new();
        verify_tree_directory(&runner, repository.path(), "src/engine", &cancel).unwrap();
        assert!(matches!(
            verify_tree_directory(&runner, repository.path(), "src/missing", &cancel),
            Err(GitError::InvalidUrl(_))
        ));
        assert!(matches!(
            verify_tree_directory(&runner, repository.path(), "src/engine/main.rs", &cancel),
            Err(GitError::InvalidUrl(_))
        ));
    }

    #[test]
    fn cancelled_ref_discovery_never_contacts_the_remote() {
        if !git_available() {
            return;
        }
        let runner = GitRunner::detect().unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            discover_tree_ref(
                &runner,
                "https://github.com/sanskarIN/RepoDNA",
                "feature/branch",
                &cancel
            ),
            Err(GitError::Cancelled)
        ));
    }

    #[test]
    fn git_clone_selects_github_tree_branch_without_cloning_browser_url() {
        let target = resolve_clone_target(
            "https://github.com/sanskarIN/RepoDNA/tree/release-v1.4.0-development",
            UrlPolicy::default(),
        )
        .unwrap();
        let args = clone_args(&target, None, Some("3"), "destination");
        assert_eq!(
            args.as_slice(),
            &[
                "-c",
                "core.symlinks=false",
                "clone",
                "--quiet",
                "--no-recurse-submodules",
                "--single-branch",
                "--branch",
                "release-v1.4.0-development",
                "--depth",
                "3",
                "--",
                "https://github.com/sanskarIN/RepoDNA",
                "destination",
            ]
        );

        let ordinary =
            resolve_clone_target("https://github.com/sanskarIN/RepoDNA", UrlPolicy::default())
                .unwrap();
        let args = clone_args(&ordinary, None, None, "destination");
        assert!(!args.contains(&"--branch"));
        assert!(!args.contains(&"--single-branch"));
    }

    #[test]
    fn git_clone_does_not_forward_github_root_browser_navigation() {
        let target = resolve_clone_target(
            "https://github.com/org/project?tab=readme#overview",
            UrlPolicy::default(),
        )
        .unwrap();
        let args = clone_args(&target, None, None, "destination");
        assert_eq!(
            args.as_slice(),
            &[
                "-c",
                "core.symlinks=false",
                "clone",
                "--quiet",
                "--no-recurse-submodules",
                "--",
                "https://github.com/org/project",
                "destination",
            ]
        );
    }

    #[test]
    fn explicit_revision_overrides_tree_url_and_defers_commit_checkout() {
        let target = resolve_clone_target(
            "https://github.com/sanskarIN/sonumio/tree/feat/initial-audio-platform",
            UrlPolicy::default(),
        )
        .unwrap();
        let explicit = RequestedRevision::Branch("main".to_owned());
        let args = clone_args(&target, Some(&explicit), None, "destination");
        assert!(args.windows(2).any(|pair| pair == ["--branch", "main"]));
        assert!(!args.contains(&"feat/initial-audio-platform"));

        let exact = RequestedRevision::Commit("0123456789abcdef0123456789abcdef01234567".into());
        let args = clone_args(&target, Some(&exact), None, "destination");
        assert!(args.contains(&"--no-checkout"));
        assert!(!args.contains(&"--branch"));
        assert_eq!(args[args.len() - 2], target.repository_url);

        let tag = RequestedRevision::Tag("v1.5.0".into());
        let args = clone_args(&target, Some(&tag), Some("2"), "destination");
        assert!(args.contains(&"--no-checkout"));
        assert!(args.windows(2).any(|pair| pair == ["--branch", "v1.5.0"]));
    }

    #[test]
    fn detached_tag_and_commit_checkout_verify_real_git_head() {
        if !git_available() {
            return;
        }
        let repo = GitRepo::new();
        repo.write("state.txt", "first");
        let first = repo.commit(
            "first",
            "Test",
            "test@example.invalid",
            "2025-01-01T00:00:00Z",
        );
        repo.tag("v0.1.0", None);
        repo.write("state.txt", "second");
        let second = repo.commit(
            "second",
            "Test",
            "test@example.invalid",
            "2025-01-02T00:00:00Z",
        );
        let runner = GitRunner::detect().unwrap();
        let cancel = CancellationToken::new();
        finish_detached_checkout(
            &runner,
            repo.path(),
            &RequestedRevision::Tag("v0.1.0".to_owned()),
            &cancel,
        )
        .unwrap();
        assert_eq!(
            runner
                .text(repo.path(), &["rev-parse", "HEAD"], &cancel)
                .unwrap(),
            first
        );
        assert!(
            runner
                .text(repo.path(), &["symbolic-ref", "HEAD"], &cancel)
                .is_err()
        );
        finish_detached_checkout(
            &runner,
            repo.path(),
            &RequestedRevision::Commit(second.clone()),
            &cancel,
        )
        .unwrap();
        assert_eq!(
            runner
                .text(repo.path(), &["rev-parse", "HEAD"], &cancel)
                .unwrap(),
            second
        );
        assert!(
            finish_detached_checkout(
                &runner,
                repo.path(),
                &RequestedRevision::Commit("0123456789abcdef0123456789abcdef01234567".into()),
                &cancel,
            )
            .is_err()
        );
    }

    #[test]
    fn detached_checkout_preserves_cancelled_git_operations() {
        if !git_available() {
            return;
        }
        let repo = GitRepo::new();
        repo.write("state.txt", "initial");
        let commit = repo.commit(
            "initial",
            "Test",
            "test@example.invalid",
            "2025-01-01T00:00:00Z",
        );
        repo.tag("v0.1.0", None);
        let runner = GitRunner::detect().unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();

        for revision in [
            RequestedRevision::Tag("v0.1.0".to_owned()),
            RequestedRevision::Commit(commit),
        ] {
            assert!(
                matches!(
                    finish_detached_checkout(&runner, repo.path(), &revision, &cancel),
                    Err(GitError::Cancelled)
                ),
                "cancellation must not be reported as an unavailable revision"
            );
        }
    }

    #[test]
    fn invalid_urls_are_rejected_before_git_runs() {
        let Ok(runner) = GitRunner::detect() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let result = clone_repository(
            &runner,
            "ext::sh -c id",
            &dir.path().join("clone"),
            CloneOptions::default(),
            &CancellationToken::new(),
        );
        assert!(matches!(result, Err(GitError::InvalidUrl(_))));
        assert!(!dir.path().join("clone").exists());
    }

    #[test]
    fn a_failed_clone_names_the_clone_and_says_what_to_do() {
        let Ok(runner) = GitRunner::detect() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        // Nothing listens on the discard port of this machine.
        let error = clone_repository(
            &runner,
            "https://127.0.0.1:9/missing.git",
            &dir.path().join("clone"),
            CloneOptions {
                policy: UrlPolicy {
                    allow_private_hosts: true,
                    ..UrlPolicy::default()
                },
                ..CloneOptions::default()
            },
            &CancellationToken::new(),
        )
        .unwrap_err();
        assert!(error.to_string().starts_with("git clone failed"), "{error}");
        assert!(error.hint().is_some(), "{error}");
    }
}
