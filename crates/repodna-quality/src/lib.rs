//! Code-quality signals for RepoDNA.
//!
//! Every signal here is a measurement with a documented method, not a verdict: complexity
//! is counted lexically, duplication is found on normalized token streams, and dead-code
//! candidates are files nothing in the repository appears to use. Findings say what was
//! measured, which threshold was crossed, and what the measurement cannot see.

pub mod complexity;
pub mod deadcode;
pub mod duplication;
pub mod markers;
pub mod similarity;
pub mod tokens;

pub use tokens::TokenStream;

use repodna_core::model::structure::LineCounts;
use repodna_parser::FileAnalysis;

/// A first-party code file given to quality analysis.
#[derive(Debug, Clone, Copy)]
pub struct QualityFile<'a> {
    /// Repository-relative path.
    pub path: &'a str,
    /// Language identifier, when detected.
    pub language: Option<&'a str>,
    /// `true` for test code, which is excluded from complexity and duplication metrics.
    pub test: bool,
    /// Line counts.
    pub lines: LineCounts,
    /// Lexical analysis results, when the language has an analyzer.
    pub analysis: Option<&'a FileAnalysis>,
    /// Normalized tokens, when the file was tokenized for duplication detection.
    pub tokens: Option<&'a TokenStream>,
}
