//! Safe extraction of ZIP and TAR archives for analysis.
//!
//! Archives are untrusted input. Extraction therefore:
//!
//! * normalizes every entry name and rejects absolute paths and `..` traversal,
//! * skips symbolic links, hard links, and device entries,
//! * skips version-control metadata (archives are analyzed as file snapshots),
//! * enforces limits on entry count, per-entry size, total size, and compression ratio,
//!   counting the bytes actually written rather than trusting declared sizes.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use repodna_core::paths::normalize_relative;
use tempfile::TempDir;

/// Supported archive formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    /// `.zip`
    Zip,
    /// `.tar`
    Tar,
    /// `.tar.gz` or `.tgz`
    TarGz,
}

/// Extraction limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveLimits {
    /// Maximum number of entries.
    pub max_entries: usize,
    /// Maximum size of one extracted file.
    pub max_entry_bytes: u64,
    /// Maximum total extracted size.
    pub max_total_bytes: u64,
    /// Maximum ratio between uncompressed and compressed size of a ZIP entry.
    pub max_compression_ratio: u64,
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            max_entries: 200_000,
            max_entry_bytes: 512 * 1024 * 1024,
            max_total_bytes: 4 * 1024 * 1024 * 1024,
            max_compression_ratio: 250,
        }
    }
}

/// An extracted archive. The temporary directory is removed when this value is dropped.
#[derive(Debug)]
pub struct ExtractedArchive {
    directory: TempDir,
    /// Directory to analyze: the single top-level folder if the archive has one.
    pub root: PathBuf,
    /// Files extracted.
    pub files: usize,
    /// Entries skipped, with the reason (e.g. `link.txt: symbolic link`).
    pub skipped: Vec<String>,
}

impl ExtractedArchive {
    /// The temporary extraction directory.
    pub fn directory(&self) -> &Path {
        self.directory.path()
    }
}

/// Errors that stop extraction.
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    /// The file could not be read.
    #[error("could not read archive {path}: {source}")]
    Io {
        /// Archive path.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: io::Error,
    },
    /// The archive is malformed.
    #[error("{path} is not a valid archive: {message}")]
    Malformed {
        /// Archive path.
        path: PathBuf,
        /// Description.
        message: String,
    },
    /// An entry name tries to escape the extraction directory.
    #[error("archive entry {0:?} would be written outside the extraction directory")]
    PathTraversal(String),
    /// A limit was exceeded.
    #[error("archive exceeds the {0} limit; it may be an archive bomb")]
    LimitExceeded(&'static str),
    /// The file extension is not a supported archive type.
    #[error("{0} is not a supported archive (.zip, .tar, .tar.gz, .tgz)")]
    Unsupported(PathBuf),
}

/// Detects the archive kind from the file name.
pub fn detect_archive(path: &Path) -> Option<ArchiveKind> {
    let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    if name.ends_with(".zip") {
        Some(ArchiveKind::Zip)
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        Some(ArchiveKind::TarGz)
    } else if name.ends_with(".tar") {
        Some(ArchiveKind::Tar)
    } else {
        None
    }
}

struct Extractor<'a> {
    destination: &'a Path,
    limits: ArchiveLimits,
    entries: usize,
    total: u64,
    files: usize,
    skipped: Vec<String>,
}

impl Extractor<'_> {
    /// Validates an entry name and returns its safe relative path, or `None` to skip it.
    fn target(&mut self, raw_name: &str) -> Result<Option<String>, ArchiveError> {
        self.entries += 1;
        if self.entries > self.limits.max_entries {
            return Err(ArchiveError::LimitExceeded("entry count"));
        }
        let name = raw_name.replace('\\', "/");
        let relative = normalize_relative(&name)
            .ok_or_else(|| ArchiveError::PathTraversal(raw_name.to_owned()))?;
        if relative.is_empty() {
            return Ok(None);
        }
        if relative
            .split('/')
            .any(|segment| matches!(segment, ".git" | ".hg" | ".svn"))
        {
            self.skipped
                .push(format!("{relative}: version-control metadata"));
            return Ok(None);
        }
        Ok(Some(relative))
    }

    fn write(&mut self, relative: &str, reader: &mut dyn Read) -> Result<(), ArchiveError> {
        let path = self.destination.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| ArchiveError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let mut file = File::create(&path).map_err(|source| ArchiveError::Io {
            path: path.clone(),
            source,
        })?;
        let mut limited = reader.take(self.limits.max_entry_bytes + 1);
        let mut buffer = [0u8; 64 * 1024];
        let mut written = 0u64;
        loop {
            let read = limited
                .read(&mut buffer)
                .map_err(|source| ArchiveError::Io {
                    path: path.clone(),
                    source,
                })?;
            if read == 0 {
                break;
            }
            written += read as u64;
            self.total += read as u64;
            if written > self.limits.max_entry_bytes {
                return Err(ArchiveError::LimitExceeded("per-file size"));
            }
            if self.total > self.limits.max_total_bytes {
                return Err(ArchiveError::LimitExceeded("total size"));
            }
            file.write_all(&buffer[..read])
                .map_err(|source| ArchiveError::Io {
                    path: path.clone(),
                    source,
                })?;
        }
        self.files += 1;
        Ok(())
    }
}

/// Extracts an archive into a new temporary directory.
pub fn extract(path: &Path, limits: ArchiveLimits) -> Result<ExtractedArchive, ArchiveError> {
    let kind = detect_archive(path).ok_or_else(|| ArchiveError::Unsupported(path.to_path_buf()))?;
    let directory = tempfile::Builder::new()
        .prefix("repodna-archive-")
        .tempdir()
        .map_err(|source| ArchiveError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let file = File::open(path).map_err(|source| ArchiveError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut extractor = Extractor {
        destination: directory.path(),
        limits,
        entries: 0,
        total: 0,
        files: 0,
        skipped: Vec::new(),
    };
    match kind {
        ArchiveKind::Zip => extract_zip(path, file, &mut extractor)?,
        ArchiveKind::Tar => extract_tar(path, file, &mut extractor)?,
        ArchiveKind::TarGz => {
            extract_tar(path, flate2::read::GzDecoder::new(file), &mut extractor)?
        }
    }
    let (files, skipped) = (extractor.files, extractor.skipped);
    let root = single_top_level_directory(directory.path())
        .unwrap_or_else(|| directory.path().to_path_buf());
    Ok(ExtractedArchive {
        directory,
        root,
        files,
        skipped,
    })
}

fn extract_zip(path: &Path, file: File, extractor: &mut Extractor<'_>) -> Result<(), ArchiveError> {
    let malformed = |message: String| ArchiveError::Malformed {
        path: path.to_path_buf(),
        message,
    };
    let mut archive = zip::ZipArchive::new(file).map_err(|error| malformed(error.to_string()))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| malformed(error.to_string()))?;
        let raw_name = entry.name().to_owned();
        let Some(relative) = extractor.target(&raw_name)? else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        if entry.is_symlink() {
            extractor.skipped.push(format!("{relative}: symbolic link"));
            continue;
        }
        let compressed = entry.compressed_size().max(1);
        if entry.size() / compressed > extractor.limits.max_compression_ratio
            && entry.size() > 1024 * 1024
        {
            return Err(ArchiveError::LimitExceeded("compression ratio"));
        }
        extractor.write(&relative, &mut entry)?;
    }
    Ok(())
}

fn extract_tar<R: Read>(
    path: &Path,
    reader: R,
    extractor: &mut Extractor<'_>,
) -> Result<(), ArchiveError> {
    let malformed = |message: String| ArchiveError::Malformed {
        path: path.to_path_buf(),
        message,
    };
    let mut archive = tar::Archive::new(reader);
    let entries = archive
        .entries()
        .map_err(|error| malformed(error.to_string()))?;
    for entry in entries {
        let mut entry = entry.map_err(|error| malformed(error.to_string()))?;
        let raw_name = entry
            .path()
            .map_err(|error| malformed(error.to_string()))?
            .to_string_lossy()
            .into_owned();
        let Some(relative) = extractor.target(&raw_name)? else {
            continue;
        };
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            continue;
        }
        if !entry_type.is_file() {
            extractor
                .skipped
                .push(format!("{relative}: {entry_type:?} entry"));
            continue;
        }
        extractor.write(&relative, &mut entry)?;
    }
    Ok(())
}

/// Returns the only entry of `directory` when it is a directory (e.g. `project-main/`).
fn single_top_level_directory(directory: &Path) -> Option<PathBuf> {
    let mut entries = fs::read_dir(directory).ok()?.filter_map(Result::ok);
    let first = entries.next()?;
    if entries.next().is_some() || !first.file_type().ok()?.is_dir() {
        return None;
    }
    Some(first.path())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, contents) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn write_archive(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn detects_supported_formats() {
        assert_eq!(detect_archive(Path::new("a.zip")), Some(ArchiveKind::Zip));
        assert_eq!(
            detect_archive(Path::new("a.TAR.GZ")),
            Some(ArchiveKind::TarGz)
        );
        assert_eq!(detect_archive(Path::new("a.tgz")), Some(ArchiveKind::TarGz));
        assert_eq!(detect_archive(Path::new("a.tar")), Some(ArchiveKind::Tar));
        assert_eq!(detect_archive(Path::new("a.rar")), None);
    }

    #[test]
    fn extracts_zip_and_uses_single_top_level_folder() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = zip_bytes(&[
            ("project-main/src/main.rs", b"fn main() {}"),
            ("project-main/README.md", b"# Demo"),
            ("project-main/.git/config", b"[core]"),
        ]);
        let archive = write_archive(dir.path(), "project.zip", &bytes);
        let extracted = extract(&archive, ArchiveLimits::default()).unwrap();
        assert_eq!(extracted.files, 2);
        assert!(extracted.root.ends_with("project-main"));
        assert!(extracted.root.join("src/main.rs").is_file());
        assert!(!extracted.root.join(".git").exists());
        assert_eq!(extracted.skipped.len(), 1);
    }

    #[test]
    fn rejects_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = zip_bytes(&[("../../evil.sh", b"rm -rf /")]);
        let archive = write_archive(dir.path(), "evil.zip", &bytes);
        let error = extract(&archive, ArchiveLimits::default()).unwrap_err();
        assert!(matches!(error, ArchiveError::PathTraversal(_)), "{error}");
        assert!(!dir.path().parent().unwrap().join("evil.sh").exists());
    }

    #[test]
    fn enforces_size_and_count_limits() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = zip_bytes(&[("a.txt", &[b'a'; 4096]), ("b.txt", b"b"), ("c.txt", b"c")]);
        let archive = write_archive(dir.path(), "big.zip", &bytes);
        let per_file = ArchiveLimits {
            max_entry_bytes: 1000,
            ..ArchiveLimits::default()
        };
        assert!(matches!(
            extract(&archive, per_file),
            Err(ArchiveError::LimitExceeded("per-file size"))
        ));
        let count = ArchiveLimits {
            max_entries: 2,
            ..ArchiveLimits::default()
        };
        assert!(matches!(
            extract(&archive, count),
            Err(ArchiveError::LimitExceeded("entry count"))
        ));
        let total = ArchiveLimits {
            max_total_bytes: 2000,
            ..ArchiveLimits::default()
        };
        assert!(matches!(
            extract(&archive, total),
            Err(ArchiveError::LimitExceeded("total size"))
        ));
    }

    #[test]
    fn rejects_highly_compressed_entries() {
        let dir = tempfile::tempdir().unwrap();
        let zeros = vec![0u8; 4 * 1024 * 1024];
        let bytes = zip_bytes(&[("bomb.bin", &zeros)]);
        let archive = write_archive(dir.path(), "bomb.zip", &bytes);
        let error = extract(&archive, ArchiveLimits::default()).unwrap_err();
        assert!(
            matches!(error, ArchiveError::LimitExceeded("compression ratio")),
            "{error}"
        );
    }

    #[test]
    fn extracts_tar_gz_and_skips_links() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("src.tar.gz");
        {
            let encoder = flate2::write::GzEncoder::new(
                File::create(&path).unwrap(),
                flate2::Compression::default(),
            );
            let mut builder = tar::Builder::new(encoder);
            let mut header = tar::Header::new_gnu();
            header.set_size(5);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, "app/hello.txt", &b"hello"[..])
                .unwrap();
            let mut link = tar::Header::new_gnu();
            link.set_entry_type(tar::EntryType::Symlink);
            link.set_size(0);
            builder
                .append_link(&mut link, "app/link", "/etc/passwd")
                .unwrap();
            builder.into_inner().unwrap().finish().unwrap();
        }
        let extracted = extract(&path, ArchiveLimits::default()).unwrap();
        assert_eq!(extracted.files, 1);
        assert!(extracted.root.join("hello.txt").is_file());
        assert!(!extracted.root.join("link").exists());
        assert_eq!(extracted.skipped.len(), 1);
    }

    #[test]
    fn reports_malformed_archives() {
        let dir = tempfile::tempdir().unwrap();
        let archive = write_archive(dir.path(), "broken.zip", b"not a zip");
        assert!(matches!(
            extract(&archive, ArchiveLimits::default()),
            Err(ArchiveError::Malformed { .. })
        ));
        assert!(matches!(
            extract(Path::new("file.rar"), ArchiveLimits::default()),
            Err(ArchiveError::Unsupported(_))
        ));
    }
}
