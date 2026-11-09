//! Streaming parser for `git log --raw --numstat -z`.
//!
//! Each commit starts with a framed delimiter (two NUL bytes followed by ASCII
//! record separator 0x1E) produced by the format string. A bare record separator
//! can occur inside Git filenames or commit subjects, but Git filenames cannot
//! contain NUL. Header fields are separated by unit separator (0x1F) and
//! terminated by NUL. The
//! diff section then lists raw entries (`:modes shas STATUS\0path\0`, with two paths for
//! renames and copies) followed by numstat entries (`added\tdeleted\tpath\0`, or an empty
//! path followed by `old\0new\0` for renames). Merge commits have no diff section.

use std::io::BufRead;
use std::path::Path;

use repodna_core::CancellationToken;

use crate::error::GitError;
use crate::runner::GitRunner;

const RECORD: u8 = 0x1e;
const RECORD_PREFIX: &[u8] = b"\0\0\x1e";
const FIELD: char = '\u{1f}';

/// How a file changed in a commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeStatus {
    /// Added.
    Added,
    /// Modified.
    Modified,
    /// Deleted.
    Deleted,
    /// Renamed (possibly with modifications).
    Renamed,
    /// Copied from another file.
    Copied,
    /// File type changed (e.g. file to symlink).
    TypeChanged,
    /// Any other status.
    Other,
}

/// A file change within a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// Path after the change.
    pub path: String,
    /// Path before a rename or copy.
    pub old_path: Option<String>,
    /// Change status.
    pub status: ChangeStatus,
    /// Lines added (zero for binary files).
    pub insertions: u32,
    /// Lines deleted (zero for binary files).
    pub deletions: u32,
    /// `true` when Git reported the file as binary.
    pub binary: bool,
}

/// A parsed commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCommit {
    /// Full hash.
    pub hash: String,
    /// Parent hashes.
    pub parents: Vec<String>,
    /// Author name after `.mailmap`.
    pub author_name: String,
    /// Author e-mail after `.mailmap`. Used only to derive a pseudonymous identifier.
    pub author_email: String,
    /// Author timestamp (Unix seconds).
    pub timestamp: i64,
    /// Author's UTC offset in minutes.
    pub offset_minutes: i32,
    /// Subject line.
    pub subject: String,
    /// File changes (empty for merge commits).
    pub changes: Vec<FileChange>,
}

impl ParsedCommit {
    /// Returns `true` for merge commits.
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }

    /// Total lines added.
    pub fn insertions(&self) -> u64 {
        self.changes
            .iter()
            .map(|change| u64::from(change.insertions))
            .sum()
    }

    /// Total lines deleted.
    pub fn deletions(&self) -> u64 {
        self.changes
            .iter()
            .map(|change| u64::from(change.deletions))
            .sum()
    }
}

/// Options for reading history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogOptions {
    /// Maximum commits to read (0 = unlimited).
    pub max_commits: u64,
    /// Detect renames (`-M`).
    pub detect_renames: bool,
}

impl Default for LogOptions {
    fn default() -> Self {
        Self {
            max_commits: 0,
            detect_renames: true,
        }
    }
}

/// Commits read from history, newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    /// Commits in `git log` order (newest first).
    pub commits: Vec<ParsedCommit>,
    /// `true` when `max_commits` stopped reading before the root commit.
    pub truncated: bool,
}

/// Reads the history reachable from `HEAD`.
pub fn read_history(
    runner: &GitRunner,
    repo: &Path,
    options: LogOptions,
    cancel: &CancellationToken,
) -> Result<History, GitError> {
    let format = "--format=%x00%x00%x1e%H%x1f%P%x1f%aN%x1f%aE%x1f%at%x1f%ai%x1f%s";
    // Request one extra commit to detect truncation, but Git --max-count
    // accepts only a signed 32-bit integer. Leave Git uncapped if the requested
    // limit plus its sentinel cannot be represented, and apply the limit after
    // reading history. Never overflow a caller-supplied u64 bound.
    let limit = options
        .max_commits
        .checked_add(1)
        .filter(|count| options.max_commits > 0 && *count <= i32::MAX as u64)
        .map(|count| format!("--max-count={count}"));
    let mut args = vec![
        "log",
        "--no-color",
        "--no-textconv",
        "--no-ext-diff",
        "--use-mailmap",
        "--raw",
        "--numstat",
        "-z",
        format,
    ];
    if options.detect_renames {
        args.push("-M");
    } else {
        args.push("--no-renames");
    }
    if let Some(limit) = &limit {
        args.push(limit);
    }
    args.push("HEAD");
    args.push("--");

    let mut commits = Vec::new();
    runner.stream(Some(repo), &args, cancel, |reader| {
        parse_stream(reader, cancel, &mut |commit| commits.push(commit))
    })?;
    let truncated = options.max_commits > 0 && commits.len() as u64 > options.max_commits;
    if truncated {
        commits.truncate(usize::try_from(options.max_commits).unwrap_or(usize::MAX));
    }
    Ok(History { commits, truncated })
}

/// Parses a `git log` stream, calling `emit` for each commit.
pub fn parse_stream(
    reader: &mut dyn BufRead,
    cancel: &CancellationToken,
    emit: &mut dyn FnMut(ParsedCommit),
) -> Result<(), GitError> {
    let mut record = Vec::new();
    let mut count = 0usize;
    loop {
        let read = reader
            .read_until(RECORD, &mut record)
            .map_err(GitError::Io)?;
        let framed_boundary = record.ends_with(RECORD_PREFIX);
        if framed_boundary {
            record.truncate(record.len() - RECORD_PREFIX.len());
        } else if read > 0 {
            // A lone 0x1E is part of a filename, author name, or subject,
            // not the start of another commit. Keep buffering this record.
            continue;
        }
        if !record.iter().all(u8::is_ascii_whitespace) {
            emit(parse_record(&record)?);
            count += 1;
            if count.is_multiple_of(256) && cancel.is_cancelled() {
                return Err(GitError::Cancelled);
            }
        }
        record.clear();
        if read == 0 {
            break;
        }
    }
    Ok(())
}

/// Parses `+0530` / `-0800` style offsets into minutes.
fn parse_offset(text: &str) -> i32 {
    let token = text.rsplit(' ').next().unwrap_or_default();
    let (sign, digits) = match token.as_bytes().first() {
        Some(b'+') => (1, &token[1..]),
        Some(b'-') => (-1, &token[1..]),
        _ => return 0,
    };
    if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return 0;
    }
    let hours: i32 = digits[..2].parse().unwrap_or(0);
    let minutes: i32 = digits[2..].parse().unwrap_or(0);
    sign * (hours * 60 + minutes)
}

fn status_from(code: &str) -> ChangeStatus {
    match code.as_bytes().first() {
        Some(b'A') => ChangeStatus::Added,
        Some(b'M') => ChangeStatus::Modified,
        Some(b'D') => ChangeStatus::Deleted,
        Some(b'R') => ChangeStatus::Renamed,
        Some(b'C') => ChangeStatus::Copied,
        Some(b'T') => ChangeStatus::TypeChanged,
        _ => ChangeStatus::Other,
    }
}

fn parse_record(record: &[u8]) -> Result<ParsedCommit, GitError> {
    let text = String::from_utf8_lossy(record);
    let (header, body) = text.split_once('\0').unwrap_or((&text, ""));
    let fields: Vec<&str> = header.split(FIELD).collect();
    if fields.len() < 7 {
        return Err(GitError::Parse(format!(
            "commit header has {} fields, expected 7",
            fields.len()
        )));
    }
    let hash = fields[0].trim().to_owned();
    if hash.len() < 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(GitError::Parse(format!("invalid commit hash {hash:?}")));
    }
    let timestamp = fields[4]
        .trim()
        .parse::<i64>()
        .map_err(|_| GitError::Parse(format!("invalid timestamp {:?}", fields[4])))?;
    let mut commit = ParsedCommit {
        hash,
        parents: fields[1].split_whitespace().map(str::to_owned).collect(),
        author_name: fields[2].trim().to_owned(),
        author_email: fields[3].trim().to_owned(),
        timestamp,
        offset_minutes: parse_offset(fields[5].trim()),
        subject: fields[6..].join(" ").trim().to_owned(),
        changes: Vec::new(),
    };
    parse_changes(body, &mut commit.changes);
    Ok(commit)
}

fn parse_changes(body: &str, changes: &mut Vec<FileChange>) {
    let tokens: Vec<&str> = body.split('\0').collect();
    let mut index = 0;
    let mut stats: Vec<(Option<(u32, u32)>, String)> = Vec::new();
    while index < tokens.len() {
        let token = tokens[index].trim_start_matches(['\n', '\r']);
        index += 1;
        if token.is_empty() {
            continue;
        }
        if let Some(raw) = token.strip_prefix(':') {
            let status = raw.split_whitespace().last().unwrap_or_default();
            let status_kind = status_from(status);
            let (old_path, path) =
                if matches!(status_kind, ChangeStatus::Renamed | ChangeStatus::Copied) {
                    let old = tokens.get(index).copied().unwrap_or_default().to_owned();
                    let new = tokens
                        .get(index + 1)
                        .copied()
                        .unwrap_or_default()
                        .to_owned();
                    index += 2;
                    (Some(old), new)
                } else {
                    let path = tokens.get(index).copied().unwrap_or_default().to_owned();
                    index += 1;
                    (None, path)
                };
            changes.push(FileChange {
                path,
                old_path,
                status: status_kind,
                insertions: 0,
                deletions: 0,
                binary: false,
            });
        } else if let Some((counts, rest)) = split_numstat(token) {
            let path = if rest.is_empty() {
                // Rename or copy: the next two tokens are the old and new paths.
                let new = tokens
                    .get(index + 1)
                    .copied()
                    .unwrap_or_default()
                    .to_owned();
                index += 2;
                new
            } else {
                rest.to_owned()
            };
            stats.push((counts, path));
        }
    }
    // Numstat entries follow raw entries in the same order; match by position and verify
    // the path, falling back to a path lookup if the orders ever disagree.
    for (position, (counts, path)) in stats.into_iter().enumerate() {
        let target = match changes.get(position) {
            Some(change) if change.path == path => Some(position),
            _ => changes.iter().position(|change| change.path == path),
        };
        if let Some(target) = target {
            let change = &mut changes[target];
            match counts {
                Some((insertions, deletions)) => {
                    change.insertions = insertions;
                    change.deletions = deletions;
                }
                None => change.binary = true,
            }
        }
    }
}

/// Splits `added\tdeleted\tpath` into counts (None for binary `-\t-`) and the path.
fn split_numstat(token: &str) -> Option<(Option<(u32, u32)>, &str)> {
    let mut parts = token.splitn(3, '\t');
    let added = parts.next()?;
    let deleted = parts.next()?;
    let rest = parts.next()?;
    if added == "-" && deleted == "-" {
        return Some((None, rest));
    }
    let added = added.parse().ok()?;
    let deleted = deleted.parse().ok()?;
    Some((Some((added, deleted)), rest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_testkit::{GitRepo, git_available};

    fn parse(bytes: &[u8]) -> Vec<ParsedCommit> {
        let mut commits = Vec::new();
        let mut reader = std::io::Cursor::new(bytes);
        parse_stream(&mut reader, &CancellationToken::new(), &mut |c| {
            commits.push(c)
        })
        .unwrap();
        commits
    }

    #[test]
    fn parses_raw_and_numstat_records() {
        let hash_a = "a".repeat(40);
        let hash_b = "b".repeat(40);
        let mut stream = Vec::new();
        stream.extend_from_slice(format!("\0\0\x1e{hash_b}\x1f{hash_a}\x1fAda\x1fada@example.invalid\x1f1700000000\x1f2023-11-14 22:13:20 +0530\x1frename and binary\0\n").as_bytes());
        stream.extend_from_slice(b":100644 100644 1111111 2222222 R070\0a.txt\0b.txt\0:000000 100644 0000000 3333333 A\0bin.dat\0");
        stream.extend_from_slice(b"1\t0\t\0a.txt\0b.txt\0-\t-\tbin.dat\0");
        stream.extend_from_slice(format!("\0\0\x1e{hash_a}\x1f\x1fAda\x1fada@example.invalid\x1f1699999000\x1f2023-11-14 21:56:40 -0800\x1ffirst\0\n").as_bytes());
        stream.extend_from_slice(b":000000 100644 0000000 1111111 A\0a.txt\0:000000 100644 0000000 4444444 A\0d/sp ace.txt\0");
        stream.extend_from_slice(b"2\t0\ta.txt\x001\t0\td/sp ace.txt\x00");
        let commits = parse(&stream);
        assert_eq!(commits.len(), 2);
        let newest = &commits[0];
        assert_eq!(newest.parents, vec![hash_a.clone()]);
        assert_eq!(newest.offset_minutes, 330);
        assert_eq!(newest.subject, "rename and binary");
        assert_eq!(newest.changes.len(), 2);
        assert_eq!(newest.changes[0].status, ChangeStatus::Renamed);
        assert_eq!(newest.changes[0].old_path.as_deref(), Some("a.txt"));
        assert_eq!(newest.changes[0].path, "b.txt");
        assert_eq!(newest.changes[0].insertions, 1);
        assert!(newest.changes[1].binary);
        let oldest = &commits[1];
        assert!(oldest.parents.is_empty());
        assert_eq!(oldest.offset_minutes, -480);
        assert_eq!(oldest.changes[1].path, "d/sp ace.txt");
        assert_eq!(oldest.insertions(), 3);
    }

    #[test]
    fn record_separator_inside_path_or_subject_does_not_split_commits() {
        let hash = "a".repeat(40);
        let mut stream = format!(
            "\0\0\x1e{hash}\x1f\x1fAda\x1fada@example.invalid\x1f1700000000\x1f2023-11-14 22:13:20 +0000\x1fsubject\x1ewith marker\0\n"
        )
        .into_bytes();
        stream.extend_from_slice(b":000000 100644 0000000 1111111 A\0odd\x1ename.txt\0");
        stream.extend_from_slice(b"1\t0\todd\x1ename.txt\0");

        let commits = parse(&stream);
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].subject, "subject\u{1e}with marker");
        assert_eq!(commits[0].changes.len(), 1);
        assert_eq!(commits[0].changes[0].path, "odd\u{1e}name.txt");
        assert_eq!(commits[0].changes[0].insertions, 1);
    }

    #[cfg(unix)]
    #[test]
    fn reads_real_git_history_with_record_separators_in_paths() {
        if !git_available() {
            return;
        }
        let repo = GitRepo::new();
        let path = "src/odd\u{1e}name.txt";
        repo.write(path, "first\n");
        repo.commit(
            "first",
            "Ada",
            "ada@example.invalid",
            "2024-01-01T00:00:00Z",
        );
        repo.write(path, "first\nsecond\n");
        repo.commit(
            "second",
            "Ada",
            "ada@example.invalid",
            "2024-02-01T00:00:00Z",
        );

        let history = read_history(
            &GitRunner::detect().unwrap(),
            repo.path(),
            LogOptions::default(),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(history.commits.len(), 2);
        assert_eq!(history.commits[0].changes[0].path, path);
        assert_eq!(history.commits[1].changes[0].path, path);
    }

    #[test]
    fn rejects_malformed_headers() {
        let mut reader = std::io::Cursor::new(b"\0\0\x1enot a header\0".to_vec());
        let result = parse_stream(&mut reader, &CancellationToken::new(), &mut |_| {});
        assert!(matches!(result, Err(GitError::Parse(_))));
    }

    #[test]
    fn reads_real_history_with_renames_and_limits() {
        if !git_available() {
            return;
        }
        let repo = GitRepo::new();
        repo.write("a.txt", "hello\nworld\n");
        repo.commit(
            "first",
            "Ada",
            "ada@example.invalid",
            "2024-01-01T10:00:00+05:30",
        );
        repo.rename("a.txt", "docs/b.txt");
        repo.write("docs/b.txt", "hello\nworld\nmore\n");
        repo.commit(
            "move",
            "Grace",
            "grace@example.invalid",
            "2024-02-01T10:00:00Z",
        );
        repo.remove("docs/b.txt");
        repo.write("c.txt", "c\n");
        repo.commit(
            "delete",
            "Ada",
            "ada@example.invalid",
            "2024-03-01T10:00:00Z",
        );

        let runner = GitRunner::detect().unwrap();
        let history = read_history(
            &runner,
            repo.path(),
            LogOptions::default(),
            &CancellationToken::new(),
        )
        .unwrap();
        assert!(!history.truncated);
        let subjects: Vec<_> = history.commits.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects, vec!["delete", "move", "first"]);
        let moved = &history.commits[1];
        assert_eq!(moved.changes[0].status, ChangeStatus::Renamed);
        assert_eq!(moved.changes[0].old_path.as_deref(), Some("a.txt"));
        assert_eq!(moved.changes[0].path, "docs/b.txt");
        assert_eq!(history.commits[2].offset_minutes, 330);

        let limited = read_history(
            &runner,
            repo.path(),
            LogOptions {
                max_commits: 2,
                ..LogOptions::default()
            },
            &CancellationToken::new(),
        )
        .unwrap();
        assert!(limited.truncated);
        assert_eq!(limited.commits.len(), 2);

        // Very large u64 limits cannot be passed to Git's signed --max-count
        // argument. They must behave as an effectively unlimited request here.
        for max_commits in [i32::MAX as u64, u64::MAX - 1, u64::MAX] {
            let oversized = read_history(
                &runner,
                repo.path(),
                LogOptions {
                    max_commits,
                    ..LogOptions::default()
                },
                &CancellationToken::new(),
            )
            .unwrap();
            assert!(!oversized.truncated);
            assert_eq!(oversized.commits.len(), 3);
        }
    }
}
