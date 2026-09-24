//! Data-driven language specifications.
//!
//! Every language RepoDNA understands is described by a [`LanguageSpec`]: how to recognize
//! its files, its comment and string syntax, and optional rules for imports, symbols, and
//! complexity. The same generic scanner processes every language, which keeps behavior
//! consistent and lets new languages be added declaratively (see [`crate::declarative`]).

use regex::Regex;
use repodna_core::model::languages::{LanguageKind, ParserCapability};
use repodna_core::model::structure::SymbolKind;

/// A line-comment introducer such as `//` or `#`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineComment {
    /// The token that starts the comment.
    pub token: String,
    /// When `true`, the token only starts a comment at the beginning of a line or after
    /// whitespace (so `$#` in shell or `a#b` are not comments).
    pub requires_boundary: bool,
}

/// A block-comment delimiter pair such as `/*` … `*/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockComment {
    /// Opening token.
    pub open: String,
    /// Closing token.
    pub close: String,
    /// Whether comments of this kind nest (Rust, Swift, Kotlin, Dart, Haskell).
    pub nested: bool,
}

/// A string-literal rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringRule {
    /// Opening delimiter, e.g. `"` or `"""`.
    pub open: String,
    /// Closing delimiter.
    pub close: String,
    /// Escape character, if escapes are processed inside the literal.
    pub escape: Option<char>,
    /// Whether the literal may span lines. Single-line literals end at a newline even if
    /// unterminated, which keeps one malformed line from masking the rest of a file.
    pub multiline: bool,
}

impl StringRule {
    /// A single-line literal with backslash escapes.
    pub fn simple(delimiter: &str) -> Self {
        Self {
            open: delimiter.to_owned(),
            close: delimiter.to_owned(),
            escape: Some('\\'),
            multiline: false,
        }
    }

    /// A multi-line literal with backslash escapes.
    pub fn multiline(delimiter: &str) -> Self {
        Self {
            multiline: true,
            ..Self::simple(delimiter)
        }
    }

    /// A multi-line literal without escapes.
    pub fn raw(open: &str, close: &str) -> Self {
        Self {
            open: open.to_owned(),
            close: close.to_owned(),
            escape: None,
            multiline: true,
        }
    }
}

/// Comment and string syntax of a language.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Syntax {
    /// Line-comment introducers.
    pub line_comments: Vec<LineComment>,
    /// Block-comment delimiters.
    pub block_comments: Vec<BlockComment>,
    /// String-literal rules, tried in order (longer delimiters must come first).
    pub strings: Vec<StringRule>,
    /// Rust-style `'a'` character literals that must not be confused with lifetimes.
    pub rust_char_literals: bool,
    /// Rust raw strings such as `r#"…"#`.
    pub rust_raw_strings: bool,
}

/// What kind of dependency an import pattern expresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportKind {
    /// `import` / `use` / `require` / `source`.
    Import,
    /// `#include "local.h"`.
    Include,
    /// `#include <system.h>`.
    SystemInclude,
    /// Module declaration pulling in another file (Rust `mod x;`).
    Module,
    /// A reference from markup (script `src`, stylesheet `href`, CSS `@import`).
    Reference,
}

/// Specialized import extractors for syntax that single-line patterns cannot capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportExtractor {
    /// Only the language's regular-expression patterns.
    Patterns,
    /// Rust `use` trees (possibly multi-line), `mod x;`, and `extern crate`.
    Rust,
    /// Go `import "x"` and parenthesized import blocks.
    Go,
    /// Python `import a, b` and `from x import (y, z)`.
    Python,
}

/// A regular expression whose capture group `group` holds an import specifier.
#[derive(Debug, Clone)]
pub struct ImportPattern {
    /// The expression, applied to comment-free lines with string contents intact.
    pub regex: Regex,
    /// Capture group holding the specifier.
    pub group: usize,
    /// Kind of import.
    pub kind: ImportKind,
}

/// A regular expression whose capture group `group` holds a symbol name.
#[derive(Debug, Clone)]
pub struct SymbolPattern {
    /// The expression, applied to masked lines (comments removed, string contents blanked).
    pub regex: Regex,
    /// Capture group holding the name.
    pub group: usize,
    /// Kind of symbol.
    pub kind: SymbolKind,
}

/// How function bodies are delimited, used to find where a function ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyStyle {
    /// `{` … `}`.
    Braces,
    /// Indentation (Python).
    Indentation,
    /// Keyword blocks closed with `end` (Ruby, Lua, Elixir).
    EndKeyword,
    /// The language has no function bodies to measure.
    None,
}

/// Rules for approximating cyclomatic complexity.
#[derive(Debug, Clone, Default)]
pub struct ComplexityRules {
    /// Keywords that introduce a decision point, matched on word boundaries.
    pub keywords: Vec<String>,
    /// Operators that introduce a decision point, e.g. `&&`.
    pub operators: Vec<String>,
    /// Whether ` ? ` counts as a ternary decision point.
    pub ternary: bool,
    /// Compiled word-boundary expression for `keywords` (built by [`ComplexityRules::compile`]).
    pub keyword_regex: Option<Regex>,
}

impl ComplexityRules {
    /// Creates rules and compiles the keyword expression.
    ///
    /// Word-like operators such as `and` are matched on word boundaries like keywords, so
    /// they never match inside identifiers such as `random`.
    pub fn new(keywords: &[&str], operators: &[&str], ternary: bool) -> Self {
        let (word_operators, symbol_operators): (Vec<&str>, Vec<&str>) = operators
            .iter()
            .partition(|operator| operator.chars().all(|c| c.is_ascii_alphabetic()));
        let mut rules = Self {
            keywords: keywords
                .iter()
                .chain(word_operators.iter())
                .map(|k| (*k).to_owned())
                .collect(),
            operators: symbol_operators.iter().map(|o| (*o).to_owned()).collect(),
            ternary,
            keyword_regex: None,
        };
        rules.compile();
        rules
    }

    /// (Re)compiles the keyword expression.
    pub fn compile(&mut self) {
        self.keyword_regex = if self.keywords.is_empty() {
            None
        } else {
            let alternatives: Vec<String> =
                self.keywords.iter().map(|k| regex::escape(k)).collect();
            Regex::new(&format!(r"\b(?:{})\b", alternatives.join("|"))).ok()
        };
    }

    /// Returns `true` when no decision points are defined.
    pub fn is_empty(&self) -> bool {
        self.keywords.is_empty() && self.operators.is_empty() && !self.ternary
    }
}

/// A complete language specification.
#[derive(Debug, Clone)]
pub struct LanguageSpec {
    /// Stable identifier, e.g. `typescript`.
    pub id: String,
    /// Display name, e.g. `TypeScript`.
    pub name: String,
    /// Language family.
    pub kind: LanguageKind,
    /// Lowercase file extensions without the dot.
    pub extensions: Vec<String>,
    /// Exact file names, e.g. `Makefile`.
    pub filenames: Vec<String>,
    /// File-name prefixes, e.g. `Dockerfile.` for `Dockerfile.dev`.
    pub filename_prefixes: Vec<String>,
    /// Interpreter names recognized in `#!` lines, e.g. `python3`.
    pub shebangs: Vec<String>,
    /// Comment and string syntax.
    pub syntax: Syntax,
    /// Import extraction strategy.
    pub import_extractor: ImportExtractor,
    /// Import patterns.
    pub imports: Vec<ImportPattern>,
    /// Pattern capturing a package or namespace declaration.
    pub package: Option<Regex>,
    /// Function and method patterns.
    pub functions: Vec<SymbolPattern>,
    /// Type, module, and other symbol patterns.
    pub types: Vec<SymbolPattern>,
    /// How function bodies are delimited.
    pub body: BodyStyle,
    /// Complexity rules.
    pub complexity: ComplexityRules,
    /// Whether the language participates in duplication detection.
    pub duplication: bool,
}

impl LanguageSpec {
    /// Creates a detection-only specification.
    pub fn new(id: &str, name: &str, kind: LanguageKind) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            kind,
            extensions: Vec::new(),
            filenames: Vec::new(),
            filename_prefixes: Vec::new(),
            shebangs: Vec::new(),
            syntax: Syntax::default(),
            import_extractor: ImportExtractor::Patterns,
            imports: Vec::new(),
            package: None,
            functions: Vec::new(),
            types: Vec::new(),
            body: BodyStyle::None,
            complexity: ComplexityRules::default(),
            duplication: false,
        }
    }

    /// The analysis depth this specification supports.
    pub fn capability(&self) -> ParserCapability {
        let has_syntax = !self.syntax.line_comments.is_empty()
            || !self.syntax.block_comments.is_empty()
            || !self.syntax.strings.is_empty();
        if !self.functions.is_empty() || !self.types.is_empty() {
            ParserCapability::Lexical
        } else if has_syntax || matches!(self.kind, LanguageKind::Data | LanguageKind::Prose) {
            ParserCapability::LineCount
        } else {
            ParserCapability::Detection
        }
    }
}
