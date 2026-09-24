//! # repodna-parser
//!
//! Language detection and lexical analysis for RepoDNA.
//!
//! RepoDNA does not claim compiler-level understanding of source code. The built-in
//! analyzers are *lexical*: a comment- and string-aware scanner feeds data-driven rules
//! for imports, symbols, markers, and approximate complexity. Each language reports its
//! [`ParserCapability`](repodna_core::model::languages::ParserCapability) so results are
//! always labeled with the depth of analysis behind them.

pub mod spec;

pub use spec::LanguageSpec;
