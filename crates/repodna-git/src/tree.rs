//! Reading historical trees and file contents without checking anything out.
//!
//! The Time Machine inspects past revisions through `git ls-tree` and
//! `git cat-file --batch`, so historical analysis never touches the working tree.

use std::io::Read;
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
}

/// Lists every file (blob) in `revision`.
pub fn list_tree(
    runner: &GitRunner,
    repo: &Path,
    revision: &str,
    cancel: &CancellationToken,
) -> Result<Vec<TreeEntry>, GitError> {
    validate_revision(revision)?;
    let output = runner.output(
        repo,
        &["ls-tree", "-r", "-l", "-z", "--full-tree", revision],
        cancel,
    )?;
    let mut entries = Vec::new();
    for record in output.split(|&byte| byte == 0) {
        if record.is_empty() {
            continue;
        }
        let text = String::from_utf8_lossy(record);
        let Some((header, path)) = text.split_once('\t') else {
            continue;
        };
        let mut fields = header.split_whitespace();
        let (Some(mode), Some(kind), Some(_object), Some(size)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if kind != "blob" {
            continue;
        }
        entries.push(TreeEntry {
            path: path.to_owned(),
            size: size.parse().unwrap_or(0),
            mode: mode.to_owned(),
        });
    }
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
    let requested: Vec<&String> = paths
        .iter()
        .filter(|path| !path.contains(['\n', '\r']))
        .collect();
    if requested.is_empty() {
        return Ok(Vec::new());
    }
    let mut input = Vec::new();
    for path in &requested {
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
            let mut total = 0u64;
            for path in &requested {
                let mut header = String::new();
                if reader.read_line(&mut header).map_err(GitError::Io)? == 0 {
                    break;
                }
                let header = header.trim_end();
                if header.ends_with(" missing") || header.ends_with(" ambiguous") {
                    continue;
                }
                let mut fields = header.split(' ');
                let (Some(_object), Some(kind), Some(size)) =
                    (fields.next(), fields.next(), fields.next())
                else {
                    return Err(GitError::Parse(format!(
                        "unexpected cat-file header {header:?}"
                    )));
                };
                let size: u64 = size
                    .parse()
                    .map_err(|_| GitError::Parse(format!("invalid blob size in {header:?}")))?;
                let keep = kind == "blob"
                    && size <= limits.max_blob_bytes
                    && total + size <= limits.max_total_bytes;
                if keep {
                    let mut content = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
                    reader
                        .take(size)
                        .read_to_end(&mut content)
                        .map_err(GitError::Io)?;
                    total += size;
                    results.push(((*path).clone(), content));
                } else {
                    std::io::copy(&mut reader.take(size), &mut std::io::sink())
                        .map_err(GitError::Io)?;
                }
                // Each object is followed by a newline.
                let mut newline = [0u8; 1];
                reader.read_exact(&mut newline).map_err(GitError::Io)?;
            }
            std::io::copy(reader, &mut std::io::sink()).map_err(GitError::Io)?;
            Ok(())
        },
    )?;
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_testkit::{GitRepo, git_available};

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
