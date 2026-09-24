//! # repodna-discovery
//!
//! Finds and classifies the files of a repository without modifying it.
//!
//! Discovery is read-only and conservative: it respects `.gitignore` rules, never follows
//! symbolic links, skips version-control metadata, reads files only up to a configured
//! size, and extracts archives with strict limits against path traversal and archive bombs.

pub mod archive;
pub mod classify;
pub mod content;
pub mod walk;

pub use archive::{
    ArchiveError, ArchiveKind, ArchiveLimits, ExtractedArchive, detect_archive, extract,
};
pub use classify::{Classification, ClassificationOverrides, classify};
pub use content::{FileContent, ReadOutcome, looks_binary, looks_generated, read_file};
pub use walk::{DiscoveredFile, Discovery, DiscoveryError, DiscoveryOptions, discover};
