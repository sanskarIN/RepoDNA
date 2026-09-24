//! Architecture analysis for RepoDNA.
//!
//! This crate infers modules from the directory layout and package boundaries, resolves
//! the imports extracted by `repodna-parser` to files inside the repository, and builds
//! file- and module-level dependency graphs. From those graphs it derives cycles, layers,
//! fan-in and fan-out, instability, betweenness centrality, and architecture findings.
//!
//! Everything here is static and lexical: imports that are computed at runtime, generated
//! code, and reflection are invisible, and the report says so in its method notes.

pub mod entrypoints;
pub mod findings;
pub mod graph;
pub mod interactions;
pub mod modules;
pub mod resolve;

use repodna_parser::FileAnalysis;

/// A file given to architecture analysis.
#[derive(Debug, Clone, Copy)]
pub struct SourceFile<'a> {
    /// Repository-relative path.
    pub path: &'a str,
    /// Language identifier, when detected.
    pub language: Option<&'a str>,
    /// Code lines.
    pub code_lines: u64,
    /// `true` for first-party source and test code. Only these files form modules and
    /// graph edges; other files are still resolvable targets for references.
    pub first_party: bool,
    /// `true` for test code.
    pub test: bool,
    /// Lexical analysis results, when the language has an analyzer.
    pub analysis: Option<&'a FileAnalysis>,
}
