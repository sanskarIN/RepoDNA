//! Cloning remote repositories for analysis.

use std::path::Path;

use repodna_core::CancellationToken;

use crate::error::GitError;
use crate::runner::GitRunner;
use crate::url::{CloneTarget, UrlPolicy, resolve_clone_target};

/// Options for [`clone_repository`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CloneOptions {
    /// Limit history to this many commits (`--depth`). `None` clones full history.
    pub depth: Option<u32>,
    /// URL policy.
    pub policy: UrlPolicy,
}

/// Assemble Git's checkout options separately from the URL and destination.
///
/// The explicit `--` keeps a remote URL from being interpreted as a Git flag.
fn clone_args<'a>(
    target: &'a CloneTarget,
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
    if let Some(branch) = &target.branch {
        args.extend(["--single-branch", "--branch", branch.as_str()]);
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
    let destination = destination.to_string_lossy().into_owned();
    let depth = options.depth.map(|depth| depth.max(1).to_string());
    let args = clone_args(&target, depth.as_deref(), &destination);
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
        })
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
        let args = clone_args(&target, Some("3"), "destination");
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

        let ordinary = resolve_clone_target(
            "https://github.com/sanskarIN/RepoDNA",
            UrlPolicy::default(),
        )
        .unwrap();
        let args = clone_args(&ordinary, None, "destination");
        assert!(!args.contains(&"--branch"));
        assert!(!args.contains(&"--single-branch"));
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
