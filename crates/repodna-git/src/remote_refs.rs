//! Bounded, read-only discovery of advertised remote branches and tags.
//!
//! The caller must use the same URL policy as cloning. No checkout, fetch,
//! submodule traversal, or provider API is required.

use std::collections::BTreeMap;
use std::io::{BufRead, Read};
use std::time::Duration;

use repodna_core::CancellationToken;

use crate::error::GitError;
use crate::runner::GitRunner;
use crate::url::{RequestedRevision, UrlPolicy, resolve_clone_target};

/// Maximum number of selectable references returned to a client.
pub const MAX_SELECTABLE_REFS: usize = 2_000;
const MAX_LINE: usize = 16 * 1024;
const MAX_BYTES: usize = 16 * 1024 * 1024;
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(20);

/// Namespace of an advertised reference. A tag and branch may share a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RemoteRefKind {
    /// Reference in refs/heads.
    Branch,
    /// Reference in refs/tags.
    Tag,
}

/// An advertised Git object. Annotated tags advertise a tag object, not
/// necessarily the final commit; this value must not be called a commit SHA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRef {
    /// Branch or tag namespace.
    pub kind: RemoteRefKind,
    /// Complete name, including any slash components.
    pub name: String,
    /// Advertised object ID (SHA-1 or SHA-256).
    pub object: String,
}

/// Bounded snapshot of an advertisement, not a guarantee of later checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRefs {
    /// Deterministic namespace/name order.
    pub refs: Vec<RemoteRef>,
    /// More unique references were advertised than the result retained.
    pub truncated: bool,
}

fn parse_line(line: &[u8]) -> Option<RemoteRef> {
    let line = std::str::from_utf8(line)
        .ok()?
        .trim_end_matches(['\r', '\n']);
    let (object, reference) = line.split_once('\t')?;
    if !matches!(object.len(), 40 | 64) || !object.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let (kind, name) = if let Some(name) = reference.strip_prefix("refs/heads/") {
        (RemoteRefKind::Branch, name)
    } else {
        let name = reference.strip_prefix("refs/tags/")?;
        (RemoteRefKind::Tag, name)
    };
    let requested = match kind {
        RemoteRefKind::Branch => RequestedRevision::Branch(name.to_owned()),
        RemoteRefKind::Tag => RequestedRevision::Tag(name.to_owned()),
    };
    requested.validate().ok()?;
    Some(RemoteRef {
        kind,
        name: name.to_owned(),
        object: object.to_ascii_lowercase(),
    })
}

fn parse_listing(
    reader: &mut dyn BufRead,
    cancel: &CancellationToken,
    limit: usize,
) -> Result<RemoteRefs, GitError> {
    let mut retained = BTreeMap::<(RemoteRefKind, String), String>::new();
    let mut truncated = false;
    let mut total = 0usize;
    let mut line = Vec::new();
    loop {
        if cancel.is_cancelled() {
            return Err(GitError::Cancelled);
        }
        line.clear();
        let size = (&mut *reader)
            .take((MAX_LINE + 1) as u64)
            .read_until(b'\n', &mut line)
            .map_err(GitError::Io)?;
        if size == 0 {
            break;
        }
        total = total.saturating_add(size);
        if size > MAX_LINE || total > MAX_BYTES {
            return Err(GitError::Parse(
                "remote reference advertisement exceeded its byte limit".to_owned(),
            ));
        }
        let Some(reference) = parse_line(&line) else {
            continue;
        };
        let key = (reference.kind, reference.name);
        if let Some(previous) = retained.get(&key) {
            if previous != &reference.object {
                return Err(GitError::Parse(
                    "remote advertised conflicting object IDs for a reference".to_owned(),
                ));
            }
            continue;
        }
        retained.insert(key, reference.object);
        if retained.len() > limit {
            retained.pop_last();
            truncated = true;
        }
    }
    if cancel.is_cancelled() {
        return Err(GitError::Cancelled);
    }
    Ok(RemoteRefs {
        refs: retained
            .into_iter()
            .map(|((kind, name), object)| RemoteRef { kind, name, object })
            .collect(),
        truncated,
    })
}

/// Discover branches and tags from a validated remote URL using native Git.
///
/// Network access occurs only when called. The result is a bounded snapshot;
/// the normal clone path must still validate the chosen checkout. Use a manual
/// selector for references not retained by the discovery limit.
pub fn discover_remote_refs(
    input: &str,
    policy: UrlPolicy,
    cancel: &CancellationToken,
) -> Result<RemoteRefs, GitError> {
    let target = resolve_clone_target(input, policy)?;
    if cancel.is_cancelled() {
        return Err(GitError::Cancelled);
    }
    let runner = GitRunner::detect()?.with_timeout(DISCOVERY_TIMEOUT);
    let mut result = None;
    runner.stream(
        None,
        &[
            "ls-remote",
            "--refs",
            "--heads",
            "--tags",
            "--",
            &target.repository_url,
        ],
        cancel,
        |reader| {
            result = Some(parse_listing(reader, cancel, MAX_SELECTABLE_REFS)?);
            Ok(())
        },
    )?;
    result.ok_or_else(|| GitError::Parse("remote reference listing was unavailable".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn record(object: char, reference: &str) -> String {
        format!("{}\t{reference}\n", object.to_string().repeat(40))
    }

    fn parse(text: &str, limit: usize) -> RemoteRefs {
        parse_listing(
            &mut Cursor::new(text.as_bytes()),
            &CancellationToken::new(),
            limit,
        )
        .unwrap()
    }

    #[test]
    fn distinguishes_namespaces_and_preserves_slash_names() {
        let text = [
            record('b', "refs/tags/feat/audio"),
            record('a', "refs/heads/feat/audio"),
            record('a', "refs/heads/feat/audio"),
            record('c', "refs/heads/main"),
            record('d', "refs/pull/17/head"),
            record('e', "refs/tags/invalid^{}"),
            record('f', "refs/heads/--invalid"),
        ]
        .concat();
        let refs = parse(&text, 20);
        assert!(!refs.truncated);
        assert_eq!(refs.refs.len(), 3);
        assert_eq!(refs.refs[0].kind, RemoteRefKind::Branch);
        assert_eq!(refs.refs[0].name, "feat/audio");
        assert_eq!(refs.refs[1].name, "main");
        assert_eq!(refs.refs[2].kind, RemoteRefKind::Tag);
        assert_eq!(refs.refs[2].object, "b".repeat(40));
    }

    #[test]
    fn result_limit_is_deterministic_across_advertisement_order() {
        let records = [
            record('c', "refs/heads/z"),
            record('a', "refs/heads/a"),
            record('b', "refs/heads/b"),
            record('d', "refs/tags/a"),
        ];
        let forward = parse(&records.concat(), 2);
        let reversed = parse(&records.into_iter().rev().collect::<String>(), 2);
        assert_eq!(forward, reversed);
        assert!(forward.truncated);
        assert_eq!(
            forward
                .refs
                .iter()
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn rejects_conflicting_duplicate_records() {
        let text = [
            record('a', "refs/heads/main"),
            record('b', "refs/heads/main"),
        ]
        .concat();
        assert!(matches!(
            parse_listing(&mut Cursor::new(text), &CancellationToken::new(), 10),
            Err(GitError::Parse(_))
        ));
    }

    #[test]
    fn rejects_oversized_records_and_honors_cancellation() {
        let mut oversized = Cursor::new(vec![b'x'; MAX_LINE + 1]);
        assert!(matches!(
            parse_listing(&mut oversized, &CancellationToken::new(), 2),
            Err(GitError::Parse(_))
        ));
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            parse_listing(&mut Cursor::new(b""), &cancel, 2),
            Err(GitError::Cancelled)
        ));
        assert!(matches!(
            discover_remote_refs("https://github.com/org/repo", UrlPolicy::default(), &cancel),
            Err(GitError::Cancelled)
        ));
    }

    #[test]
    fn invalid_remote_inputs_are_rejected_without_network() {
        for url in [
            "file:///tmp/repo",
            "https://127.0.0.1/org/repo",
            "https://github.com/org/repohttps://github.com/org/other",
            "https://github.com/org/repo/pull/1",
        ] {
            assert!(matches!(
                discover_remote_refs(url, UrlPolicy::default(), &CancellationToken::new()),
                Err(GitError::InvalidUrl(_))
            ));
        }
    }

    #[test]
    fn accepts_sha256_and_crlf_but_not_ambiguous_or_malformed_objects() {
        let text = format!(
            "{}\trefs/heads/feature/新機能\r\n123\trefs/heads/short\n{}\trefs/tags/v1\n",
            "A".repeat(64),
            "x".repeat(40)
        );
        let result = parse(&text, 10);
        assert_eq!(result.refs.len(), 1);
        assert_eq!(result.refs[0].object, "a".repeat(64));
        assert_eq!(result.refs[0].name, "feature/新機能");
    }
}
