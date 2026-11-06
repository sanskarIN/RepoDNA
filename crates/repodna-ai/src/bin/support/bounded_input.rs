//! Shared bounded JSON input reads for offline native-model CLIs.
//!
//! Size metadata is advisory: a file may grow after the initial check. Read
//! at most the configured budget plus one byte, and reject non-regular inputs
//! so a pipe or device cannot block an offline audit indefinitely.

use std::error::Error;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

fn read_with_cap(reader: impl Read, max_bytes: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub(super) fn read_bounded_file(
    path: &Path,
    max_bytes: u64,
    too_large_message: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err("input must be a regular file".into());
    }
    if metadata.len() > max_bytes {
        return Err(too_large_message.into());
    }
    let bytes = read_with_cap(File::open(path)?, max_bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(too_large_message.into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_reads_from_an_unbounded_source() {
        // Simulates a source that keeps producing data after a stale size check.
        let bytes = read_with_cap(io::repeat(b'x'), 8).expect("bounded read");
        assert_eq!(bytes.len(), 9);
    }

    #[test]
    fn accepts_exact_limit_and_refuses_oversize_or_directory() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let input = dir.path().join("input.json");
        fs::write(&input, b"12345678").expect("write file");
        assert_eq!(
            read_bounded_file(&input, 8, "input exceeds 8 bytes").unwrap(),
            b"12345678"
        );
        assert!(
            read_bounded_file(&input, 7, "input exceeds 7 bytes")
                .unwrap_err()
                .to_string()
                .contains("exceeds 7 bytes")
        );
        assert!(
            read_bounded_file(dir.path(), 8, "input exceeds 8 bytes")
                .unwrap_err()
                .to_string()
                .contains("regular file")
        );
    }
}
