//! Reading historical trees and file contents without checking anything out.
//!
//! The Time Machine inspects past revisions through `git ls-tree` and
//! `git cat-file --batch`, so historical analysis never touches the working tree.

use std::io::{BufRead, Read};
use std::path::Path;

use repodna_core::CancellationToken;

use crate::error::GitError;
use crate::runner::{GitRunner, validate_revision};

/// A file in a historical tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    /// Repository-relative path.
    pub path: String,
    /// Size in bytes.
    pub size: u64,
    /// File mode, e.g. `100644` or `120000` for a symbolic link.
    pub mode: String,
    /// Object name of the blob; files with the same contents have the same name.
    pub object: String,
}

/// Parse a single NUL-delimited `git ls-tree -l -z` record.
fn parse_tree_entry(record: &[u8]) -> Option<TreeEntry> {
    if record.is_empty() {
        return None;
    }
    let text = String::from_utf8_lossy(record);
    let (header, path) = text.split_once('\t')?;
    let mut fields = header.split_whitespace();
    let (Some(mode), Some(kind), Some(object), Some(size)) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return None;
    };
    if kind != "blob" {
        return None;
    }
    Some(TreeEntry {
        path: path.to_owned(),
        size: size.parse().unwrap_or(0),
        mode: mode.to_owned(),
        object: object.to_owned(),
    })
}

/// Visit NUL-delimited Git tree records while checking cancellation before
/// the next read and after each emitted entry.
fn visit_tree_entries(
    reader: &mut dyn BufRead,
    cancel: &CancellationToken,
    mut consume: impl FnMut(TreeEntry),
) -> Result<(), GitError> {
    let mut record = Vec::new();
    loop {
        if cancel.is_cancelled() {
            return Err(GitError::Cancelled);
        }
        record.clear();
        if reader.read_until(0, &mut record).map_err(GitError::Io)? == 0 {
            break;
        }
        if record.last() == Some(&0) {
            record.pop();
        }
        if let Some(entry) = parse_tree_entry(&record) {
            consume(entry);
        }
        if cancel.is_cancelled() {
            return Err(GitError::Cancelled);
        }
    }
    Ok(())
}

/// Lists every file (blob) in `revision`.
///
/// Parse the NUL-delimited stream incrementally instead of buffering the
/// entire raw tree listing. Memory used for decoding is proportional to the
/// largest single entry, plus the final list of file metadata.
pub fn list_tree(
    runner: &GitRunner,
    repo: &Path,
    revision: &str,
    cancel: &CancellationToken,
) -> Result<Vec<TreeEntry>, GitError> {
    validate_revision(revision)?;
    let mut entries = Vec::new();
    runner.stream(
        Some(repo),
        &["ls-tree", "-r", "-l", "-z", "--full-tree", revision],
        cancel,
        |reader| visit_tree_entries(reader, cancel, |entry| entries.push(entry)),
    )?;
    Ok(entries)
}

/// Limits for [`read_blobs`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlobLimits {
    /// Blobs larger than this are skipped.
    pub max_blob_bytes: u64,
    /// Reading stops (remaining blobs are skipped) after this many bytes.
    pub max_total_bytes: u64,
}

impl Default for BlobLimits {
    fn default() -> Self {
        Self {
            max_blob_bytes: 1024 * 1024,
            max_total_bytes: 256 * 1024 * 1024,
        }
    }
}

fn within_blob_budget(kind: &str, size: u64, total: u64, limits: BlobLimits) -> bool {
    kind == "blob"
        && size <= limits.max_blob_bytes
        && total <= limits.max_total_bytes
        && size <= limits.max_total_bytes - total
}

/// Interpret one `git cat-file --batch` reply at a time, giving the caller a
/// cancellation checkpoint before each record and after each emitted blob.
fn visit_blob_records(
    reader: &mut dyn BufRead,
    requested: &[&String],
    limits: BlobLimits,
    cancel: &CancellationToken,
    mut consume: impl FnMut(String, Vec<u8>),
) -> Result<(), GitError> {
    let mut total = 0u64;
    for path in requested {
        if cancel.is_cancelled() {
            return Err(GitError::Cancelled);
        }
        let mut header = String::new();
        if reader.read_line(&mut header).map_err(GitError::Io)? == 0 {
            break;
        }
        let header = header.trim_end();
        if header.ends_with(" missing") || header.ends_with(" ambiguous") {
            continue;
        }
        let mut fields = header.split(' ');
        let (Some(_object), Some(kind), Some(size)) = (fields.next(), fields.next(), fields.next())
        else {
            return Err(GitError::Parse(format!(
                "unexpected cat-file header {header:?}"
            )));
        };
        let size: u64 = size
            .parse()
            .map_err(|_| GitError::Parse(format!("invalid blob size in {header:?}")))?;
        let keep = within_blob_budget(kind, size, total, limits);
        let mut content = Vec::new();
        if keep {
            content = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
            reader
                .take(size)
                .read_to_end(&mut content)
                .map_err(GitError::Io)?;
            total += size;
        } else {
            std::io::copy(&mut reader.take(size), &mut std::io::sink()).map_err(GitError::Io)?;
        }
        // Each object is followed by a newline.
        let mut newline = [0u8; 1];
        reader.read_exact(&mut newline).map_err(GitError::Io)?;
        if keep {
            consume((**path).clone(), content);
        }
        if cancel.is_cancelled() {
            return Err(GitError::Cancelled);
        }
    }
    // A batch may produce extra output; drain to avoid blocking Git.
    if cancel.is_cancelled() {
        return Err(GitError::Cancelled);
    }
    std::io::copy(reader, &mut std::io::sink()).map_err(GitError::Io)?;
    Ok(())
}

/// Reads the contents of `paths` at `revision`. Paths that do not exist, contain line
/// breaks, or exceed the limits are omitted from the result.
pub fn read_blobs(
    runner: &GitRunner,
    repo: &Path,
    revision: &str,
    paths: &[String],
    limits: BlobLimits,
    cancel: &CancellationToken,
) -> Result<Vec<(String, Vec<u8>)>, GitError> {
    validate_revision(revision)?;
    cancel.check().map_err(|_| GitError::Cancelled)?;
    let requested: Vec<&String> = paths
        .iter()
        .filter(|path| !path.contains(['\n', '\r']))
        .collect();
    if requested.is_empty() {
        return Ok(Vec::new());
    }
    let mut input = Vec::new();
    for path in &requested {
        if cancel.is_cancelled() {
            return Err(GitError::Cancelled);
        }
        input.extend_from_slice(revision.as_bytes());
        input.push(b':');
        input.extend_from_slice(path.as_bytes());
        input.push(b'\n');
    }
    let mut results = Vec::new();
    runner.stream_with_input(
        Some(repo),
        &["cat-file", "--batch"],
        Some(input),
        cancel,
        |reader| {
            visit_blob_records(reader, &requested, limits, cancel, |path, content| {
                results.push((path, content));
            })
        },
    )?;
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_testkit::{GitRepo, git_available};

    #[test]
    fn tree_entry_parser_ignores_invalid_records_and_keeps_special_paths() {
        assert!(parse_tree_entry(b"").is_none());
        assert!(parse_tree_entry(b"invalid").is_none());
        assert!(parse_tree_entry(b"040000 tree deadbeef -\troot").is_none());
        let entry = parse_tree_entry(b"100644 blob abc123 7\tdocs/name with space.md")
            .expect("valid ls-tree record");
        assert_eq!(entry.path, "docs/name with space.md");
        assert_eq!(entry.size, 7);
        assert_eq!(entry.object, "abc123");
    }

    #[test]
    fn tree_stream_stops_after_the_first_cancelled_entry() {
        let cancel = CancellationToken::new();
        let mut reader = std::io::Cursor::new(
            b"100644 blob abc123 1\tfirst.txt\x00100644 blob def456 1\tsecond.txt\0",
        );
        let mut visited = Vec::new();
        let outcome = visit_tree_entries(&mut reader, &cancel, |entry| {
            visited.push(entry.path);
            cancel.cancel();
        });
        assert!(matches!(outcome, Err(GitError::Cancelled)));
        assert_eq!(visited, vec!["first.txt"]);
    }

    #[test]
    fn streams_large_tree_listing_without_losing_entries() {
        if !git_available() {
            return;
        }
        let runner = GitRunner::detect().unwrap();
        let repo = GitRepo::new();
        for index in 0..300 {
            repo.write(&format!("files/file-{index:03}.txt"), "x");
        }
        repo.commit(
            "many files",
            "Ada",
            "ada@example.invalid",
            "2024-01-01T00:00:00Z",
        );
        let entries = list_tree(&runner, repo.path(), "HEAD", &CancellationToken::new()).unwrap();
        assert_eq!(entries.len(), 300);
        assert_eq!(entries.first().unwrap().path, "files/file-000.txt");
        assert_eq!(entries.last().unwrap().path, "files/file-299.txt");
        assert!(entries.iter().all(|entry| entry.size == 1));
    }

    #[test]
    fn blob_stream_stops_after_first_cancelled_record() {
        let cancel = CancellationToken::new();
        let names = ["first.txt".to_owned(), "second.txt".to_owned()];
        let requested: Vec<&String> = names.iter().collect();
        let mut reader = std::io::Cursor::new(b"abc blob 1\nX\ndef blob 1\nY\n");
        let mut visited = Vec::new();
        let result = visit_blob_records(
            &mut reader,
            &requested,
            BlobLimits::default(),
            &cancel,
            |path, content| {
                visited.push((path, content));
                cancel.cancel();
            },
        );
        assert!(matches!(result, Err(GitError::Cancelled)));
        assert_eq!(visited, vec![("first.txt".to_owned(), b"X".to_vec())]);
    }

    #[test]
    fn blob_budget_checked_without_overflow() {
        let limits = BlobLimits {
            max_blob_bytes: u64::MAX,
            max_total_bytes: u64::MAX,
        };
        assert!(within_blob_budget("blob", 2, u64::MAX - 2, limits));
        assert!(!within_blob_budget("blob", 3, u64::MAX - 2, limits));
        assert!(!within_blob_budget("blob", 1, u64::MAX, limits));
        assert!(!within_blob_budget("tree", 1, 0, limits));
        assert!(!within_blob_budget(
            "blob",
            1,
            11,
            BlobLimits {
                max_blob_bytes: 10,
                max_total_bytes: 10,
            }
        ));
    }

    #[test]
    fn lists_and_reads_historical_files() {
        if !git_available() {
            return;
        }
        let runner = GitRunner::detect().unwrap();
        let cancel = CancellationToken::new();
        let repo = GitRepo::new();
        repo.write("src/lib.rs", "pub fn a() {}\n");
        repo.write("docs/with space.md", "# Docs\n");
        let first = repo.commit(
            "first",
            "Ada",
            "ada@example.invalid",
            "2024-01-01T00:00:00Z",
        );
        repo.write("src/lib.rs", "pub fn b() {}\n");
        repo.write("big.bin", vec![b'x'; 5000]);
        repo.commit(
            "second",
            "Ada",
            "ada@example.invalid",
            "2024-02-01T00:00:00Z",
        );

        let old_tree = list_tree(&runner, repo.path(), &first, &cancel).unwrap();
        let paths: Vec<_> = old_tree.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["docs/with space.md", "src/lib.rs"]);
        assert_eq!(old_tree[1].size, 14);
        let object = runner
            .text(
                repo.path(),
                &["rev-parse", &format!("{first}:src/lib.rs")],
                &cancel,
            )
            .unwrap();
        assert_eq!(old_tree[1].object, object);
        let new_tree = list_tree(&runner, repo.path(), "HEAD", &cancel).unwrap();
        let changed = new_tree.iter().find(|e| e.path == "src/lib.rs").unwrap();
        let unchanged = new_tree
            .iter()
            .find(|e| e.path == "docs/with space.md")
            .unwrap();
        assert_ne!(changed.object, old_tree[1].object);
        assert_eq!(unchanged.object, old_tree[0].object);

        let requested = vec![
            "src/lib.rs".to_owned(),
            "missing.txt".to_owned(),
            "docs/with space.md".to_owned(),
        ];
        let blobs = read_blobs(
            &runner,
            repo.path(),
            &first,
            &requested,
            BlobLimits::default(),
            &cancel,
        )
        .unwrap();
        assert_eq!(blobs.len(), 2);
        assert_eq!(
            blobs[0],
            ("src/lib.rs".to_owned(), b"pub fn a() {}\n".to_vec())
        );
        assert_eq!(blobs[1].0, "docs/with space.md");

        let limits = BlobLimits {
            max_blob_bytes: 100,
            ..BlobLimits::default()
        };
        let head = read_blobs(
            &runner,
            repo.path(),
            "HEAD",
            &["big.bin".to_owned(), "src/lib.rs".to_owned()],
            limits,
            &cancel,
        )
        .unwrap();
        assert_eq!(
            head.len(),
            1,
            "oversized blobs are skipped but the stream stays in sync"
        );
        assert_eq!(head[0].1, b"pub fn b() {}\n".to_vec());

        // A total budget can skip a later file without losing stream alignment.
        let bounded = read_blobs(
            &runner,
            repo.path(),
            "HEAD",
            &["docs/with space.md".to_owned(), "src/lib.rs".to_owned()],
            BlobLimits {
                max_blob_bytes: 100,
                max_total_bytes: 7,
            },
            &cancel,
        )
        .unwrap();
        assert_eq!(
            bounded,
            vec![("docs/with space.md".to_owned(), b"# Docs\n".to_vec())]
        );
    }

    #[test]
    fn rejects_option_like_revisions() {
        if !git_available() {
            return;
        }
        let runner = GitRunner::detect().unwrap();
        let repo = GitRepo::new();
        assert!(matches!(
            list_tree(
                &runner,
                repo.path(),
                "--output=x",
                &CancellationToken::new()
            ),
            Err(GitError::InvalidRevision(_))
        ));
    }
}
