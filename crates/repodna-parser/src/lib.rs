//! # repodna-parser
//!
//! Language detection and lexical analysis for RepoDNA.
//!
//! RepoDNA does not claim compiler-level understanding of source code. The built-in
//! analyzers are *lexical*: a comment- and string-aware scanner feeds data-driven rules
//! for imports, symbols, markers, and approximate complexity. Each language reports its
//! [`ParserCapability`](repodna_core::model::languages::ParserCapability) so results are
//! always labeled with the depth of analysis behind them.

pub mod analyze;
pub mod builtin;
pub mod declarative;
pub mod imports;
pub mod markers;
pub mod registry;
pub mod scanner;
pub mod spec;
pub mod symbols;
pub mod tokens;

pub use analyze::{
    ANALYZER_VERSION, FileAnalysis, RawReference, analyze_and_tokenize, analyze_source, count_lines,
};
pub use builtin::builtin_languages;
pub use declarative::{DefinitionError, load_directory, parse_definition};
pub use imports::{RawImport, extract_imports, extract_package};
pub use markers::{RawMarker, extract_markers};
pub use registry::{LanguageRegistry, shebang_interpreter};
pub use scanner::{LineKind, ScannedFile, ScannedLine, scan};
pub use spec::LanguageSpec;
pub use symbols::{ParsedSymbol, SymbolAnalysis, extract_symbols};
pub use tokens::{Token, tokenize};
