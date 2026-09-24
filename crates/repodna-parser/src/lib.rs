//! # repodna-parser
//!
//! Language detection and lexical analysis for RepoDNA.
//!
//! RepoDNA does not claim compiler-level understanding of source code. The built-in
//! analyzers are *lexical*: a comment- and string-aware scanner feeds data-driven rules
//! for imports, symbols, markers, and approximate complexity. Each language reports its
//! [`ParserCapability`](repodna_core::model::languages::ParserCapability) so results are
//! always labeled with the depth of analysis behind them.

pub mod builtin;
pub mod imports;
pub mod markers;
pub mod registry;
pub mod scanner;
pub mod spec;
pub mod symbols;
pub mod tokens;

pub use builtin::builtin_languages;
pub use imports::{RawImport, extract_imports, extract_package};
pub use markers::{RawMarker, extract_markers};
pub use registry::{LanguageRegistry, shebang_interpreter};
pub use scanner::{LineKind, ScannedFile, ScannedLine, scan};
pub use spec::LanguageSpec;
pub use symbols::{ParsedSymbol, SymbolAnalysis, extract_symbols};
pub use tokens::{Token, tokenize};
