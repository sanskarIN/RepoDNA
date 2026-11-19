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
fn consider_remote_ref(requested: &str, advertised: &str, best: &mut Option<RemoteTreeMatch>) {
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

/// Parse the bounded Git advertisement stream. Keeping this pure allows
/// deterministic fixtures without relaxing the production runner's disabled
/// file:// transport.
fn select_remote_ref(
    reader: &mut dyn BufRead,
    requested: &str,
) -> Result<Option<RemoteTreeMatch>, GitError> {
    let mut best = None;
    let mut total = 0usize;
    loop {
        let mut line = Vec::new();
        let size = (&mut *reader)
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
        let Some((object, reference)) = line.trim_end_matches(['\r', '\n']).split_once('\t') else {
            continue;
        };
        if matches!(object.len(), 40 | 64) && object.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            consider_remote_ref(requested, reference, &mut best);
        }
    }
    Ok(best)
}

/// Resolve only advertised Git refs through the hardened, cancellable runner.
fn discover_tree_ref(
    runner: &GitRunner,
    repository_url: &str,
    requested: &str,
    cancel: &CancellationToken,
) -> Result<RemoteTreeMatch, GitError> {
    let mut selected = None;
    runner.stream(
        None,
        &[
            "ls-remote",
            "--refs",
            "--heads",
            "--tags",
            "--",
            repository_url,
        ],
        cancel,
        |reader| {
            selected = select_remote_ref(reader, requested)?;
            Ok(())
        },
    )?;
    selected.ok_or_else(|| {
        GitError::InvalidUrl(
            "the GitHub /tree/ path does not match an advertised branch or tag; use a branch-root URL or select --branch/--tag explicitly"
                .to_owned(),
        )
    })
}

/// Convert an advertised ref to a typed checkout request so a same-name tag
/// cannot silently replace a branch (or vice versa).
fn selected_revision(selected: &RemoteTreeMatch) -> RequestedRevision {
    if selected.branch {
        RequestedRevision::Branch(selected.reference.clone())
    } else {
        RequestedRevision::Tag(selected.reference.clone())
    }
}

/// Git's --short symbolic-ref output may be qualified as heads/name when a
/// tag shares the branch name. The full symbolic ref is unambiguous.
fn verify_branch_checkout(
    runner: &GitRunner,
    destination: &Path,
    expected: &str,
    cancel: &CancellationToken,
) -> Result<(), GitError> {
    let actual = runner.text(destination, &["symbolic-ref", "--quiet", "HEAD"], cancel)?;
    if actual == format!("refs/heads/{expected}") {
        Ok(())
    } else {
        Err(GitError::InvalidUrl(
            "the selected branch did not produce a branch checkout".to_owned(),
        ))
    }
}

/// Require a real directory, never silently choose a shorter ref for a typo.
fn verify_tree_directory(
    runner: &GitRunner,
    checkout: &Path,
    directory: &str,
    cancel: &CancellationToken,
) -> Result<(), GitError> {
    // Git quotes names containing double quotes even with core.quotePath=false.
    // -z is the only unambiguous format: compare raw, NUL-delimited paths rather
    // than Git's human-readable escaped output.
    let entries = runner.output(
        checkout,
        &["ls-tree", "-z", "-d", "--name-only", "HEAD", "--", directory],
        cancel,
    )?;
    if entries
        .split(|byte| *byte == 0)
        .any(|entry| entry == directory.as_bytes())
    {
        Ok(())
    } else {
        Err(GitError::InvalidUrl(
            "the GitHub /tree/ directory does not exist at the checked-out revision".to_owned(),
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
    let target = resolve_clone_target(url, options.policy)?;
    let mut selected_directory = None;
    let mut revision = options
        .revision
        .or_else(|| target.commit.clone().map(RequestedRevision::Commit));
    // Explicit selectors bypass remote discovery; branch-root URLs without
    // slashes retain the original clone path and its error reporting.
    if revision.is_none()
        && let Some(candidate) = target.branch.as_deref().filter(|name| name.contains('/'))
    {
        let selected = discover_tree_ref(runner, &target.repository_url, candidate, cancel)?;
        // Determine checkout kind before taking ownership of the directory.
        // A same-name tag must never silently replace the selected branch.
        revision = Some(selected_revision(&selected));
        selected_directory = selected.directory;
    }
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
        verify_branch_checkout(runner, destination, expected, cancel)?;
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
    fn malformed_or_oversized_remote_ref_advertisements_are_rejected() {
        let mut malformed =
            std::io::Cursor::new(b"not-a-sha\trefs/heads/main\n0123\trefs/heads/main\n");
        assert_eq!(select_remote_ref(&mut malformed, "main").unwrap(), None);

        let too_long = vec![b'x'; MAX_REMOTE_REF_LINE + 1];
        let mut oversized = std::io::Cursor::new(too_long);
        assert!(matches!(
            select_remote_ref(&mut oversized, "main"),
            Err(GitError::Parse(_))
        ));
    }

    #[test]
    fn tree_directory_must_exist_at_selected_revision() {
        if !git_available() {
            return;
        }
        let repository = GitRepo::new();
        repository.write("src/engine/main.rs", "fn main() {}");
        repository.write(".github/workflows/ci.yml", "name: CI");
        repository.write("docs/design notes/intro.md", "# Notes");
        // Quotes are legal Unix filenames but forbidden on Windows.
        #[cfg(unix)]
        repository.write("docs/said \"hello\"/intro.md", "# Quoted");
        repository.write("docs/日本語/intro.md", "# Unicode");
        repository.commit(
            "add source directory",
            "Test",
            "test@example.invalid",
            "2025-01-01T00:00:00Z",
        );
        let runner = GitRunner::detect().unwrap();
        let cancel = CancellationToken::new();
        verify_tree_directory(&runner, repository.path(), "src/engine", &cancel).unwrap();
        verify_tree_directory(&runner, repository.path(), ".github", &cancel).unwrap();
        verify_tree_directory(&runner, repository.path(), ".github/workflows", &cancel).unwrap();
        verify_tree_directory(&runner, repository.path(), "docs/design notes", &cancel).unwrap();
        #[cfg(unix)]
        verify_tree_directory(&runner, repository.path(), "docs/said \"hello\"", &cancel).unwrap();
        verify_tree_directory(&runner, repository.path(), "docs/日本語", &cancel).unwrap();
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
    fn remote_ref_discovery_preserves_tag_and_branch_checkout_kinds() {
        if !git_available() {
            return;
        }
        let repo = GitRepo::new();
        repo.write("src/engine/main.rs", "fn main() {}");
        repo.commit(
            "create nested source",
            "Test",
            "test@example.invalid",
            "2025-01-01T00:00:00Z",
        );
        repo.git(&["branch", "feat"]);
        repo.tag("feat/audio", None);
        repo.git(&["branch", "other/audio"]);
        let runner = GitRunner::detect().unwrap();
        let cancel = CancellationToken::new();
        let listing = repo.git(&["ls-remote", "--refs", "--heads", "--tags", "--", "."]);
        let mut reader = std::io::Cursor::new(listing.as_bytes());

        let selected = select_remote_ref(&mut reader, "feat/audio/src/engine")
            .unwrap()
            .unwrap();
        assert_eq!(selected.reference, "feat/audio");
        assert_eq!(selected.directory.as_deref(), Some("src/engine"));
        assert_eq!(
            selected_revision(&selected),
            RequestedRevision::Tag("feat/audio".to_owned())
        );
        verify_tree_directory(
            &runner,
            repo.path(),
            selected.directory.as_deref().unwrap(),
            &cancel,
        )
        .unwrap();

        let mut reader = std::io::Cursor::new(listing.as_bytes());
        let selected = select_remote_ref(&mut reader, "other/audio/src/engine")
            .unwrap()
            .unwrap();
        assert_eq!(
            selected_revision(&selected),
            RequestedRevision::Branch("other/audio".to_owned())
        );
    }

    #[test]
    fn discovered_tag_and_branch_clone_the_expected_commit_and_directory() {
        if !git_available() {
            return;
        }
        let repo = GitRepo::new();
        repo.write("src/engine/main.rs", "first");
        let first = repo.commit(
            "first",
            "Test",
            "test@example.invalid",
            "2025-01-01T00:00:00Z",
        );
        repo.tag("feat/audio", None);
        repo.write("src/engine/main.rs", "second");
        let second = repo.commit(
            "second",
            "Test",
            "test@example.invalid",
            "2025-01-02T00:00:00Z",
        );
        repo.git(&["branch", "other/audio"]);
        let runner = GitRunner::detect().unwrap();
        let cancel = CancellationToken::new();
        let source = CloneTarget {
            repository_url: repo.path().to_string_lossy().into_owned(),
            branch: None,
            commit: None,
        };
        let destination = tempfile::tempdir().unwrap();

        let listing = repo.git(&["ls-remote", "--refs", "--heads", "--tags", "--", "."]);
        let mut reader = std::io::Cursor::new(listing.as_bytes());
        let selected = select_remote_ref(&mut reader, "feat/audio/src/engine")
            .unwrap()
            .unwrap();
        let tagged = destination.path().join("tagged");
        let tagged_text = tagged.to_string_lossy().into_owned();
        let tag_revision = selected_revision(&selected);
        repo.git(&clone_args(
            &source,
            Some(&tag_revision),
            None,
            &tagged_text,
        ));
        finish_detached_checkout(&runner, &tagged, &tag_revision, &cancel).unwrap();
        verify_tree_directory(
            &runner,
            &tagged,
            selected.directory.as_deref().unwrap(),
            &cancel,
        )
        .unwrap();
        assert_eq!(
            runner
                .text(&tagged, &["rev-parse", "HEAD"], &cancel)
                .unwrap(),
            first
        );
        assert_eq!(
            std::fs::read_to_string(tagged.join("src/engine/main.rs")).unwrap(),
            "first"
        );

        let mut reader = std::io::Cursor::new(listing.as_bytes());
        let selected = select_remote_ref(&mut reader, "other/audio/src/engine")
            .unwrap()
            .unwrap();
        let branched = destination.path().join("branched");
        let branched_text = branched.to_string_lossy().into_owned();
        let branch_revision = selected_revision(&selected);
        repo.git(&clone_args(
            &source,
            Some(&branch_revision),
            None,
            &branched_text,
        ));
        verify_branch_checkout(&runner, &branched, "other/audio", &cancel).unwrap();
        verify_tree_directory(
            &runner,
            &branched,
            selected.directory.as_deref().unwrap(),
            &cancel,
        )
        .unwrap();
        assert_eq!(
            runner
                .text(&branched, &["rev-parse", "HEAD"], &cancel)
                .unwrap(),
            second
        );
        assert_eq!(
            std::fs::read_to_string(branched.join("src/engine/main.rs")).unwrap(),
            "second"
        );
    }

    #[test]
    fn checkout_with_same_named_branch_and_tag_verifies_full_symbolic_ref() {
        if !git_available() {
            return;
        }
        let repo = GitRepo::new();
        repo.write("file.txt", "contents");
        repo.commit(
            "create main",
            "Test",
            "test@example.invalid",
            "2025-01-01T00:00:00Z",
        );
        repo.tag("main", None);
        let runner = GitRunner::detect().unwrap();
        let cancel = CancellationToken::new();
        // Git shortens a branch to heads/main when a tag called main exists.
        assert_eq!(
            runner
                .text(
                    repo.path(),
                    &["symbolic-ref", "--quiet", "--short", "HEAD"],
                    &cancel
                )
                .unwrap(),
            "heads/main"
        );
        verify_branch_checkout(&runner, repo.path(), "main", &cancel).unwrap();
        assert!(matches!(
            verify_branch_checkout(&runner, repo.path(), "wrong", &cancel),
            Err(GitError::InvalidUrl(_))
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
