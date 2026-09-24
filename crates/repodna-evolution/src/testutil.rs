//! Helpers for building synthetic histories in tests.

use repodna_core::time::Timestamp;
use repodna_git::{ChangeStatus, FileChange, History, ParsedCommit};

/// A change: `+path` adds, `-path` deletes, `~path` modifies, `old>new` renames.
pub fn change(spec: &str) -> FileChange {
    let (status, path, old_path) = if let Some(path) = spec.strip_prefix('+') {
        (ChangeStatus::Added, path, None)
    } else if let Some(path) = spec.strip_prefix('-') {
        (ChangeStatus::Deleted, path, None)
    } else if let Some((old, new)) = spec.split_once('>') {
        (ChangeStatus::Renamed, new, Some(old.to_owned()))
    } else {
        (ChangeStatus::Modified, spec.trim_start_matches('~'), None)
    };
    FileChange {
        path: path.to_owned(),
        old_path,
        status,
        insertions: 10,
        deletions: if status == ChangeStatus::Added { 0 } else { 2 },
        binary: false,
    }
}

/// A commit on `date` (`YYYY-MM-DD`) by `author` with the given changes.
pub fn commit(hash: &str, date: &str, author: &str, changes: &[&str]) -> ParsedCommit {
    let mut parts = date.split('-').map(|part| part.parse::<u32>().unwrap_or(1));
    let (year, month, day) = (
        parts.next().unwrap_or(2020),
        parts.next().unwrap_or(1),
        parts.next().unwrap_or(1),
    );
    let timestamp =
        Timestamp::from_ymd(i64::from(year), month, day).map_or(0, Timestamp::unix) + 12 * 3600;
    ParsedCommit {
        hash: hash.to_owned(),
        parents: Vec::new(),
        author_name: author.to_owned(),
        author_email: format!("{author}@example.test"),
        timestamp,
        offset_minutes: 0,
        subject: format!("commit {hash}"),
        changes: changes.iter().map(|spec| change(spec)).collect(),
    }
}

/// A history from commits given oldest first (stored newest first, like `git log`).
pub fn history(mut commits: Vec<ParsedCommit>) -> History {
    commits.reverse();
    History {
        commits,
        truncated: false,
    }
}
