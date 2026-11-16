//! Cloning remote repositories for analysis.

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
    let target = resolve_clone_target(url, options.policy)?;
    let revision = options.revision.or_else(|| target.commit.clone().map(RequestedRevision::Commit));
    if let Some(revision) = &revision {
        revision.validate()?;
    }
    let destination_text = destination.to_string_lossy().into_owned();
    let depth = options.depth.map(|depth| depth.max(1).to_string());
    let args = clone_args(&target, revision.as_ref(), depth.as_deref(), &destination_text);
    // Name the failure after what the user asked for, not after the options it ran with.
    runner.run_global(&args, cancel).map_err(|error| match error {
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
        let actual = runner.text(destination, &["symbolic-ref", "--quiet", "--short", "HEAD"], cancel)?;
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
    runner.text(destination, &["rev-parse", "--verify", "HEAD^{commit}"], cancel)?;
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
            let expected = runner.text(destination, &["rev-parse", "--verify", &peeled], cancel)
                .map_err(|_| GitError::InvalidUrl(
                    "the selected tag is not available in the cloned repository".to_owned(),
                ))?;
            (reference, expected)
        }
        RequestedRevision::Commit(sha) => {
            let peeled = format!("{sha}^{{commit}}");
            let expected = runner.text(destination, &["rev-parse", "--verify", &peeled], cancel)
                .map_err(|error| match error {
                    GitError::Cancelled => GitError::Cancelled,
                    other => {
                        let _ = other;
                        GitError::InvalidUrl(
                            "the requested commit is not reachable from fetched references; try a branch containing it or a full-history clone".to_owned(),
                        )
                    }
                })?;
            (sha.clone(), expected)
        }
        RequestedRevision::Branch(_) => return Ok(()),
    };
    runner.stream(
        Some(destination),
        &["-c", "core.symlinks=false", "checkout", "--force", "--detach", &reference],
        cancel,
        |reader| {
            std::io::copy(reader, &mut std::io::sink()).map_err(GitError::Io)?;
            Ok(())
        },
    )?;
    let actual = runner.text(destination, &["rev-parse", "--verify", "HEAD^{commit}"], cancel)?;
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
