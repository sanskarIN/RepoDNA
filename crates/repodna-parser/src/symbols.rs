//! Symbol detection, function extents, approximate cyclomatic complexity, and nesting.
//!
//! Complexity follows McCabe's definition — one plus the number of decision points — with
//! decision points recognized lexically (keywords such as `if` or `case`, short-circuit
//! operators, and ternaries). Each line's decision points are attributed to the innermost
//! function that contains it, so nested functions and closures are not double-counted.
//! Results are approximations and are labeled as such everywhere they are shown.

use std::collections::BinaryHeap;
use std::sync::LazyLock;

use regex::Regex;
use repodna_core::model::structure::SymbolKind;
use serde::{Deserialize, Serialize};

use crate::scanner::{LineKind, ScannedLine};
use crate::spec::{BodyStyle, ComplexityRules, LanguageSpec, Pattern};

/// A symbol found by lexical analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedSymbol {
    /// Symbol name.
    pub name: String,
    /// Symbol kind.
    pub kind: SymbolKind,
    /// First line (1-based).
    pub line: u32,
    /// Last line (1-based, inclusive).
    pub end_line: u32,
    /// Approximate cyclomatic complexity (functions and methods with bodies only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complexity: Option<u32>,
    /// Deepest block nesting inside the function body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nesting: Option<u32>,
}

impl ParsedSymbol {
    /// Returns `true` for functions and methods.
    pub fn is_function(&self) -> bool {
        matches!(self.kind, SymbolKind::Function | SymbolKind::Method)
    }

    /// Length in lines.
    pub fn length(&self) -> u32 {
        self.end_line.saturating_sub(self.line) + 1
    }
}

/// Symbols and complexity of one file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolAnalysis {
    /// Symbols in order of appearance.
    pub symbols: Vec<ParsedSymbol>,
    /// Decision points outside any function (top-level script code).
    pub module_complexity: u32,
    /// Deepest nesting inside any function.
    pub max_nesting: u32,
}

/// Maximum symbols recorded per file.
const MAX_SYMBOLS: usize = 5_000;

/// Words that control-flow patterns can capture but that are never function names.
const CONTROL_WORDS: &[&str] = &[
    "if",
    "else",
    "elif",
    "for",
    "foreach",
    "while",
    "do",
    "switch",
    "case",
    "catch",
    "try",
    "return",
    "throw",
    "new",
    "delete",
    "sizeof",
    "typeof",
    "await",
    "yield",
    "function",
    "import",
    "export",
    "super",
    "this",
    "using",
    "lock",
    "synchronized",
    "defined",
    "when",
    "match",
    "loop",
    "not",
    "and",
    "or",
    "in",
    "of",
    "with",
    "assert",
    "unless",
    "until",
    "begin",
    "end",
    "then",
    "fi",
    "done",
    "esac",
    "select",
    "guard",
    "defer",
    "go",
];

/// Languages in which an indented function definition is a method of an enclosing type.
const METHOD_BY_INDENT: &[&str] = &[
    "rust",
    "cpp",
    "javascript",
    "typescript",
    "python",
    "php",
    "swift",
    "dart",
    "ruby",
    "groovy",
    "scala",
    "kotlin",
    "solidity",
];

/// Languages whose function patterns are loose enough that a match without a body is more
/// likely a prototype or a call than a definition.
const REQUIRE_BODY: &[&str] = &["c", "cpp", "objective-c", "java", "csharp", "dart"];

enum Body {
    Braces { end: usize, nesting: u32 },
    Expression { end: usize },
    Declaration,
}

fn indentation(text: &str) -> usize {
    text.chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

fn line_number(index: usize) -> u32 {
    u32::try_from(index + 1).unwrap_or(u32::MAX)
}

/// Finds the body of a brace-delimited function whose signature starts at `(line, column)`.
fn brace_body(lines: &[ScannedLine], start_line: usize, start_column: usize) -> Body {
    let mut paren = 0i32;
    let mut bracket = 0i32;
    let mut seen_params = false;
    let mut expression = false;
    for (line_index, line) in lines.iter().enumerate().skip(start_line) {
        if line_index > start_line + 30 {
            return Body::Declaration;
        }
        let bytes = line.masked.as_bytes();
        let from = if line_index == start_line {
            start_column.min(bytes.len())
        } else {
            0
        };
        let mut column = from;
        while column < bytes.len() {
            let at_top = paren <= 0 && bracket <= 0;
            match bytes[column] {
                b'(' => {
                    paren += 1;
                    seen_params = true;
                }
                b')' => paren -= 1,
                b'[' => bracket += 1,
                b']' => bracket -= 1,
                b'{' if at_top => return match_braces(lines, line_index, column),
                b';' if at_top => {
                    return if expression {
                        Body::Expression { end: line_index }
                    } else {
                        Body::Declaration
                    };
                }
                b'=' if at_top => {
                    let next = bytes.get(column + 1).copied();
                    let previous = column.checked_sub(1).map(|i| bytes[i]);
                    if next == Some(b'>') {
                        expression = true;
                        column += 1;
                    } else if seen_params
                        && next != Some(b'=')
                        && !matches!(previous, Some(b'!' | b'<' | b'>' | b'='))
                    {
                        // Expression-bodied function (`fun f() = …`, `def f = …`).
                        expression = true;
                    }
                }
                _ => {}
            }
            column += 1;
        }
        if expression && paren <= 0 && bracket <= 0 {
            // Expression body without braces ends with its line unless it continues.
            let trimmed = line.masked.trim_end();
            if !trimmed.ends_with("=>") && !trimmed.ends_with('=') && !trimmed.ends_with(',') {
                return Body::Expression { end: line_index };
            }
        }
    }
    Body::Declaration
}

fn match_braces(lines: &[ScannedLine], open_line: usize, open_column: usize) -> Body {
    let mut depth = 0u32;
    let mut max_depth = 0u32;
    for (line_index, line) in lines.iter().enumerate().skip(open_line) {
        let bytes = line.masked.as_bytes();
        let from = if line_index == open_line {
            open_column
        } else {
            0
        };
        for &byte in bytes.iter().skip(from) {
            match byte {
                b'{' => {
                    depth += 1;
                    max_depth = max_depth.max(depth);
                }
                b'}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Body::Braces {
                            end: line_index,
                            nesting: max_depth.saturating_sub(1),
                        };
                    }
                }
                _ => {}
            }
        }
    }
    Body::Braces {
        end: lines.len().saturating_sub(1),
        nesting: max_depth.saturating_sub(1),
    }
}

fn indentation_body(lines: &[ScannedLine], start_line: usize) -> Body {
    let base = indentation(&lines[start_line].masked);
    let mut signature_end = start_line;
    let mut paren = 0i32;
    for (line_index, line) in lines.iter().enumerate().skip(start_line).take(30) {
        for byte in line.masked.bytes() {
            match byte {
                b'(' | b'[' => paren += 1,
                b')' | b']' => paren -= 1,
                _ => {}
            }
        }
        if paren <= 0 {
            signature_end = line_index;
            if !line.masked.trim_end().ends_with(':') {
                // One-line definition such as `def f(): return 1`.
                return Body::Expression { end: line_index };
            }
            break;
        }
    }
    let mut end = signature_end;
    let mut body_indent: Option<usize> = None;
    let mut max_level = 0u32;
    for (line_index, line) in lines.iter().enumerate().skip(signature_end + 1) {
        if line.kind != LineKind::Code || line.masked.trim().is_empty() {
            // Blank lines, comments, and lines inside multi-line strings do not end blocks.
            continue;
        }
        let indent = indentation(&line.masked);
        if indent <= base {
            break;
        }
        let body = *body_indent.get_or_insert(indent);
        let unit = body.saturating_sub(base).max(1);
        let level = u32::try_from(indent.saturating_sub(body) / unit).unwrap_or(u32::MAX);
        max_level = max_level.max(level);
        end = line_index;
    }
    Body::Braces {
        end,
        nesting: max_level,
    }
}

static RUBY_OPENER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:def|class|module|if|unless|while|until|case|begin|for)\b")
        .unwrap_or_else(|error| panic!("invalid pattern: {error}"))
});
static RUBY_ENDLESS_DEF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*def\s+[\w.?!]+(?:\([^)]*\))?\s*=[^=~]")
        .unwrap_or_else(|error| panic!("invalid pattern: {error}"))
});
static DO_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bdo\b\s*(?:\|[^|]*\|)?\s*$")
        .unwrap_or_else(|error| panic!("invalid pattern: {error}"))
});
static LUA_OPENER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:function|if|for|while|repeat)\b|^\s*do\b")
        .unwrap_or_else(|error| panic!("invalid pattern: {error}"))
});
static LUA_CLOSER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:end|until)\b").unwrap_or_else(|error| panic!("invalid pattern: {error}"))
});
static ELIXIR_OPENER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bdo(?:\s|$)|\bfn\b").unwrap_or_else(|error| panic!("invalid pattern: {error}"))
});
static END_WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bend\b").unwrap_or_else(|error| panic!("invalid pattern: {error}"))
});

// Julia block keywords are meaningful outside grouping delimiters.
// In particular, A[end] and comprehensions cannot close a function.
static JULIA_BODY_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\p{L}\p{N}_!]+|[()\[\]{}]").expect("valid Julia body token pattern")
});

fn julia_body(lines: &[ScannedLine], start_line: usize) -> Body {
    let mut depth = 0u32;
    let mut max_depth = 0u32;
    let mut delimiters = Vec::new();
    for (line_index, line) in lines.iter().enumerate().skip(start_line) {
        let text = &line.masked;
        let mut previous = "";
        for token in JULIA_BODY_TOKEN.find_iter(text) {
            let word = token.as_str();
            match word {
                "(" => delimiters.push(")"),
                "[" => delimiters.push("]"),
                "{" => delimiters.push("}"),
                ")" | "]" | "}" => {
                    if delimiters.pop() != Some(word) {
                        return Body::Declaration;
                    }
                }
                _ if delimiters.is_empty() => {
                    // Ignore property names, quoted symbols and macro names
                    // adjacent to their punctuation, not control-flow words.
                    let qualified = token.start() > 0
                        && matches!(text.as_bytes()[token.start() - 1], b'.' | b':' | b'@');
                    if !qualified {
                        let opens = matches!(
                            word,
                            "function"
                                | "macro"
                                | "module"
                                | "baremodule"
                                | "struct"
                                | "if"
                                | "for"
                                | "while"
                                | "let"
                                | "begin"
                                | "try"
                                | "quote"
                                | "do"
                        ) || (word == "type"
                            && matches!(previous, "abstract" | "primitive"));
                        if opens {
                            depth = depth.saturating_add(1);
                            max_depth = max_depth.max(depth);
                        } else if word == "end" {
                            if depth == 0 {
                                return Body::Declaration;
                            }
                            depth -= 1;
                            if depth == 0 {
                                return Body::Braces {
                                    end: line_index,
                                    nesting: max_depth.saturating_sub(1),
                                };
                            }
                        }
                    }
                }
                _ => {}
            }
            previous = word;
        }
    }
    // Missing closing keywords do not attribute the remainder of the file
    // to an apparently complete function.
    Body::Declaration
}

fn keyword_body(language: &str, lines: &[ScannedLine], start_line: usize) -> Body {
    if language == "julia" {
        return julia_body(lines, start_line);
    }
    if language == "ruby" && RUBY_ENDLESS_DEF.is_match(&lines[start_line].masked) {
        return Body::Expression { end: start_line };
    }
    let mut depth = 0i64;
    let mut max_depth = 0i64;
    for (line_index, line) in lines.iter().enumerate().skip(start_line) {
        let text = &line.masked;
        let (opens, closes) = match language {
            "lua" => (
                LUA_OPENER.find_iter(text).count(),
                LUA_CLOSER.find_iter(text).count(),
            ),
            "elixir" => (
                ELIXIR_OPENER.find_iter(text).count(),
                END_WORD.find_iter(text).count(),
            ),
            _ => (
                usize::from(RUBY_OPENER.is_match(text)) + usize::from(DO_BLOCK.is_match(text)),
                END_WORD.find_iter(text).count(),
            ),
        };
        if line_index == start_line && opens == 0 {
            return Body::Expression { end: start_line };
        }
        depth += i64::try_from(opens).unwrap_or(0);
        max_depth = max_depth.max(depth);
        depth -= i64::try_from(closes).unwrap_or(0);
        if depth <= 0 {
            return Body::Braces {
                end: line_index,
                nesting: u32::try_from((max_depth - 1).max(0)).unwrap_or(0),
            };
        }
    }
    Body::Braces {
        end: lines.len().saturating_sub(1),
        nesting: u32::try_from((max_depth - 1).max(0)).unwrap_or(0),
    }
}

/// Counts decision points in `text`.
pub fn count_decisions(rules: &ComplexityRules, text: &str) -> u32 {
    let mut count = rules
        .keyword_regex
        .as_ref()
        .and_then(Pattern::get)
        .map_or(0, |regex| regex.find_iter(text).count());
    for operator in &rules.operators {
        count += text.matches(operator.as_str()).count();
    }
    if rules.ternary {
        count += text.matches(" ? ").count();
    }
    u32::try_from(count).unwrap_or(u32::MAX)
}

fn clean_name(name: &str) -> String {
    name.trim_matches(|c| c == '"' || c == '`' || c == '\'' || c == '[' || c == ']')
        .to_owned()
}

// An interval's priority is its position in the old stable painting order.
// Later-starting functions win; for equal starts, shorter extents win, and
// exact ties retain the last symbol's position. This also preserves behavior
// for crossing intervals produced by incomplete or malformed source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OwnerInterval {
    start: usize,
    end: usize,
    symbol: usize,
}

struct ComplexityOwners {
    intervals: Vec<OwnerInterval>,
    next: usize,
    // Highest painting priority at the root. Ended intervals below the
    // current owner are removed lazily when they reach the root.
    active: BinaryHeap<usize>,
}

impl ComplexityOwners {
    fn new(symbols: &[ParsedSymbol], line_count: usize) -> Self {
        let mut intervals = symbols
            .iter()
            .enumerate()
            .filter(|(_, symbol)| symbol.complexity.is_some())
            .filter_map(|(index, symbol)| {
                let start = (symbol.line as usize).checked_sub(1)?;
                let end = (symbol.end_line as usize).min(line_count);
                (start < end).then_some(OwnerInterval {
                    start,
                    end,
                    symbol: index,
                })
            })
            .collect::<Vec<_>>();
        intervals.sort_unstable_by(|a, b| {
            a.start
                .cmp(&b.start)
                .then_with(|| b.end.cmp(&a.end))
                .then_with(|| {
                    // Preserve the original ordering of unclamped ends.
                    symbols[b.symbol].end_line.cmp(&symbols[a.symbol].end_line)
                })
                .then_with(|| a.symbol.cmp(&b.symbol))
        });
        Self {
            intervals,
            next: 0,
            active: BinaryHeap::new(),
        }
    }

    // Calls must use monotonically increasing line indices. It is safe to
    // skip blank/comment lines and code lines with no decision points.
    fn at(&mut self, line: usize) -> Option<usize> {
        while let Some(interval) = self.intervals.get(self.next) {
            if interval.start > line {
                break;
            }
            if interval.end > line {
                self.active.push(self.next);
            }
            self.next += 1;
        }
        while let Some(&priority) = self.active.peek() {
            let interval = self.intervals[priority];
            if interval.end > line {
                return Some(interval.symbol);
            }
            self.active.pop();
        }
        None
    }
}

/// Extracts symbols, function extents, complexity, and nesting.
pub fn extract_symbols(spec: &LanguageSpec, lines: &[ScannedLine]) -> SymbolAnalysis {
    let mut symbols: Vec<ParsedSymbol> = Vec::new();
    let mut nesting_by_symbol: Vec<Option<u32>> = Vec::new();
    let method_by_indent = METHOD_BY_INDENT.contains(&spec.id.as_str());
    let require_body = REQUIRE_BODY.contains(&spec.id.as_str());

    for (index, line) in lines.iter().enumerate() {
        if symbols.len() >= MAX_SYMBOLS {
            break;
        }
        if line.kind != LineKind::Code {
            continue;
        }
        let text = &line.masked;
        let function = spec.functions.iter().find_map(|pattern| {
            let captures = pattern.regex.get()?.captures(text)?;
            let name = captures.get(pattern.group)?;
            if CONTROL_WORDS.contains(&name.as_str()) {
                return None;
            }
            Some((pattern.kind, name.as_str().to_owned(), name.start()))
        });
        if let Some((mut kind, name, column)) = function {
            if kind == SymbolKind::Function && method_by_indent && indentation(text) > 0 {
                kind = SymbolKind::Method;
            }
            let body = match spec.body {
                BodyStyle::Braces => brace_body(lines, index, column),
                BodyStyle::Indentation => indentation_body(lines, index),
                BodyStyle::EndKeyword => keyword_body(&spec.id, lines, index),
                BodyStyle::None => Body::Expression { end: index },
            };
            let (end, nesting, measurable) = match body {
                Body::Braces { end, nesting } => (end, Some(nesting), true),
                Body::Expression { end } => (end, Some(0), true),
                Body::Declaration if require_body => continue,
                Body::Declaration => (index, None, false),
            };
            symbols.push(ParsedSymbol {
                name: clean_name(&name),
                kind,
                line: line_number(index),
                end_line: line_number(end.max(index)),
                complexity: measurable.then_some(1),
                nesting,
            });
            nesting_by_symbol.push(nesting);
            continue;
        }
        let type_symbol = spec.types.iter().find_map(|pattern| {
            let captures = pattern.regex.get()?.captures(text)?;
            let name = captures.get(pattern.group)?;
            Some((pattern.kind, clean_name(name.as_str())))
        });
        if let Some((kind, name)) = type_symbol
            && !name.is_empty()
        {
            symbols.push(ParsedSymbol {
                name,
                kind,
                line: line_number(index),
                end_line: line_number(index),
                complexity: None,
                nesting: None,
            });
            nesting_by_symbol.push(None);
        }
    }

    // Stream ownership rather than painting every function's entire extent.
    // Each measurable symbol is inserted and removed at most once, using
    // O(symbols) memory instead of allocating a slot for every source line.
    let mut owners = ComplexityOwners::new(&symbols, lines.len());
    let mut module_complexity = 0u32;
    for (index, line) in lines.iter().enumerate() {
        if line.kind != LineKind::Code || spec.complexity.is_empty() {
            continue;
        }
        let decisions = count_decisions(&spec.complexity, &line.masked);
        if decisions == 0 {
            continue;
        }
        match owners.at(index) {
            Some(symbol) => {
                if let Some(complexity) = symbols[symbol].complexity.as_mut() {
                    *complexity = complexity.saturating_add(decisions);
                }
            }
            None => module_complexity = module_complexity.saturating_add(decisions),
        }
    }
    let max_nesting = nesting_by_symbol
        .iter()
        .flatten()
        .copied()
        .max()
        .unwrap_or(0);
    SymbolAnalysis {
        symbols,
        module_complexity,
        max_nesting,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::LanguageRegistry;
    use crate::scanner::scan;

    fn analyze(language: &str, source: &str) -> SymbolAnalysis {
        let spec = LanguageRegistry::builtin().get(language).unwrap();
        extract_symbols(spec, &scan(source, &spec.syntax).lines)
    }

    fn summary(analysis: &SymbolAnalysis) -> Vec<(String, SymbolKind, u32, u32, Option<u32>)> {
        analysis
            .symbols
            .iter()
            .map(|s| (s.name.clone(), s.kind, s.line, s.end_line, s.complexity))
            .collect()
    }

    fn measured_symbol(line: u32, end_line: u32) -> ParsedSymbol {
        ParsedSymbol {
            name: "fixture".into(),
            kind: SymbolKind::Function,
            line,
            end_line,
            complexity: Some(1),
            nesting: Some(0),
        }
    }

    // Independent compatibility oracle: paint every extent in the old
    // stable ordering, retaining the last symbol to overwrite each line.
    fn reference_owners(symbols: &[ParsedSymbol], lines: usize) -> Vec<Option<usize>> {
        let mut result = vec![None; lines];
        let mut order: Vec<_> = (0..symbols.len())
            .filter(|&i| symbols[i].complexity.is_some())
            .collect();
        order.sort_by(|&a, &b| {
            symbols[a]
                .line
                .cmp(&symbols[b].line)
                .then_with(|| symbols[b].end_line.cmp(&symbols[a].end_line))
        });
        for index in order {
            let symbol = &symbols[index];
            let start = symbol.line as usize - 1;
            let end = (symbol.end_line as usize).min(lines);
            for owner in result.iter_mut().take(end).skip(start) {
                *owner = Some(index);
            }
        }
        result
    }

    #[test]
    fn streaming_owners_match_painting_for_all_small_interval_triples() {
        let possible: Vec<_> = (1..=5)
            .flat_map(|start| (start..=7).map(move |end| measured_symbol(start, end)))
            .collect();
        // Include duplicate intervals, crossing extents, out-of-bounds
        // ends and every input order. The oracle defines tie precedence.
        for a in &possible {
            for b in &possible {
                for c in &possible {
                    let symbols = [a.clone(), b.clone(), c.clone()];
                    let expected = reference_owners(&symbols, 5);
                    let mut owners = ComplexityOwners::new(&symbols, 5);
                    let actual: Vec<_> = (0..5).map(|line| owners.at(line)).collect();
                    assert_eq!(actual, expected, "{symbols:?}");

                    let mut sparse = ComplexityOwners::new(&symbols, 5);
                    for line in [0, 2, 4] {
                        assert_eq!(sparse.at(line), expected[line], "{symbols:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn streaming_owners_ignore_declarations_and_empty_intervals() {
        let mut declaration = measured_symbol(2, 5);
        declaration.complexity = None;
        let symbols = [
            measured_symbol(1, 10),
            declaration,
            measured_symbol(3, 2),
            measured_symbol(20, 25),
            measured_symbol(4, 4),
        ];
        let expected = reference_owners(&symbols, 5);
        let mut owners = ComplexityOwners::new(&symbols, 5);
        assert_eq!(owners.intervals.len(), 2);
        for (line, expected) in expected.into_iter().enumerate() {
            assert_eq!(owners.at(line), expected);
        }
        assert_eq!(ComplexityOwners::new(&symbols, 0).at(0), None);
    }

    #[test]
    fn streaming_owners_restore_outer_intervals_after_crossing_extents() {
        let symbols = [
            measured_symbol(1, 12),
            measured_symbol(3, 8),
            measured_symbol(4, 10),
            measured_symbol(5, 6),
        ];
        let mut owners = ComplexityOwners::new(&symbols, 12);
        assert_eq!(owners.at(0), Some(0));
        assert_eq!(owners.at(2), Some(1));
        assert_eq!(owners.at(3), Some(2));
        assert_eq!(owners.at(4), Some(3));
        assert_eq!(owners.at(6), Some(2));
        assert_eq!(owners.at(10), Some(0));
        assert_eq!(owners.at(12), None);
    }

    #[test]
    fn sparse_queries_do_not_allocate_memory_proportional_to_file_lines() {
        let symbols = [
            measured_symbol(1, u32::MAX),
            measured_symbol(10, 20),
            measured_symbol(1_000_000, 2_000_000),
        ];
        let mut owners = ComplexityOwners::new(&symbols, 10_000_000);
        assert_eq!(owners.intervals.len(), 3);
        assert_eq!(owners.at(0), Some(0));
        assert_eq!(owners.at(19), Some(1));
        assert_eq!(owners.at(20), Some(0));
        assert_eq!(owners.at(1_999_999), Some(2));
        assert_eq!(owners.at(2_000_000), Some(0));
        assert_eq!(owners.at(9_999_999), Some(0));
        assert!(owners.active.len() <= symbols.len());
    }

    #[test]
    fn nested_python_decisions_return_to_the_enclosing_function() {
        let analysis = analyze(
            "python",
            "def outer(x):\n    if x:\n        pass\n    def inner(y):\n        if y and x:\n            return y\n        return x\n    if x:\n        return inner(x)\n    return None\nif ready:\n    outer(1)\n",
        );
        let outer = analysis.symbols.iter().find(|s| s.name == "outer").unwrap();
        let inner = analysis.symbols.iter().find(|s| s.name == "inner").unwrap();
        assert_eq!(outer.complexity, Some(3));
        assert_eq!(inner.complexity, Some(3));
        assert_eq!(analysis.module_complexity, 1);
    }

    #[test]
    fn julia_long_form_functions_measure_nested_blocks_and_indexing_end() {
        let source = concat!(
            "module Example\n",
            "mutable struct Counter\n",
            "    value::Int\n",
            "end\n",
            "function update!(items)\n",
            "    values = [x for x in items if x > 0]\n",
            "    last = items[end]\n",
            "    if last > 0\n",
            "        for value in values\n",
            "            last += value\n",
            "        end\n",
            "    elseif last < 0\n",
            "        last = 0\n",
            "    end\n",
            "    return last\n",
            "end\n",
            "function Base.show(io, counter)\n",
            "    print(io, counter.value)\n",
            "end\n",
            "end\n",
        );
        let result = analyze("julia", source);
        let symbols = summary(&result);
        assert_eq!(symbols.len(), 4);
        assert_eq!(symbols[0].0, "Example");
        assert_eq!(symbols[0].1, SymbolKind::Module);
        assert_eq!(symbols[1].0, "Counter");
        assert_eq!(symbols[1].1, SymbolKind::Struct);
        assert_eq!(
            symbols[2],
            ("update!".into(), SymbolKind::Function, 5, 16, Some(6))
        );
        assert_eq!(
            symbols[3],
            ("Base.show".into(), SymbolKind::Function, 17, 19, Some(1))
        );
        assert_eq!(result.max_nesting, 2);
    }

    #[test]
    fn julia_nested_functions_own_their_decisions_and_inline_end() {
        let source = concat!(
            "function outer(x)\n",
            "    function inner(y)\n",
            "        if y > 0; return y; end\n",
            "        return 0\n",
            "    end\n",
            "    if x > 0 && x < 10\n",
            "        return inner(x)\n",
            "    end\n",
            "end\n",
            "function single(); return 1; end\n",
        );
        let result = analyze("julia", source);
        let symbols = summary(&result);
        assert_eq!(
            symbols,
            vec![
                ("outer".into(), SymbolKind::Function, 1, 9, Some(3)),
                ("inner".into(), SymbolKind::Function, 2, 5, Some(2)),
                ("single".into(), SymbolKind::Function, 10, 10, Some(1)),
            ]
        );
    }

    #[test]
    fn julia_ignores_keywords_in_comments_strings_and_grouped_expressions() {
        let source = concat!(
            "function κόσμος(x)\n",
            "    #= function bogus(); end #= nested =# =#\n",
            "    value = \"end if for\"\n",
            "    command = \x60echo end\x60\n",
            "    last = x[end]\n",
            "    symbol = :end\n",
            "    transposed = x'\n",
            "    character = ']'\n",
            "    return last\n",
            "end\n",
        );
        let result = analyze("julia", source);
        assert_eq!(result.symbols.len(), 1);
        let symbol = &result.symbols[0];
        assert_eq!(symbol.name, "κόσμος");
        assert_eq!(symbol.end_line, 10);
        assert_eq!(symbol.complexity, Some(1));
    }

    #[test]
    fn julia_unclosed_function_does_not_claim_the_rest_of_the_file() {
        let result = analyze(
            "julia",
            "function incomplete(x)\n    if x > 0\n        return x\n",
        );
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].end_line, 1);
        assert_eq!(result.symbols[0].complexity, None);
        assert_eq!(result.module_complexity, 1);
    }

    #[test]
    fn julia_do_blocks_try_and_quoted_blocks_have_balanced_extents() {
        let result = analyze(
            "julia",
            concat!(
                "function work(items)\n",
                "    map(items) do item\n",
                "        try\n",
                "            quote\n",
                "                item\n",
                "            end\n",
                "        catch error\n",
                "            nothing\n",
                "        end\n",
                "    end\n",
                "end\n",
                "function next(); return 2; end\n",
            ),
        );
        assert_eq!(result.symbols[0].end_line, 11);
        assert_eq!(result.symbols[0].nesting, Some(3));
        assert_eq!(result.symbols[0].complexity, Some(2));
        assert_eq!(result.symbols[1].end_line, 12);
    }

    #[test]
    fn rust_functions_methods_and_types() {
        let source = r#"pub struct Point { x: i32 }

impl Point {
    pub fn classify(&self) -> &'static str {
        if self.x > 0 && self.x < 10 {
            "small"
        } else if self.x >= 10 {
            match self.x { 10 => "ten", _ => "big" }
        } else {
            "negative"
        }
    }
}

trait Shape {
    fn area(&self) -> f64;
}

fn helper() -> [u8; 4] {
    [0; 4]
}
"#;
        let analysis = analyze("rust", source);
        let symbols = summary(&analysis);
        assert_eq!(symbols[0], ("Point".into(), SymbolKind::Struct, 1, 1, None));
        assert_eq!(symbols[1].0, "classify");
        assert_eq!(symbols[1].1, SymbolKind::Method);
        assert_eq!((symbols[1].2, symbols[1].3), (4, 12));
        // 1 + if + && + if + two match arms.
        assert_eq!(symbols[1].4, Some(6));
        assert_eq!(symbols[2].0, "Shape");
        assert_eq!(
            symbols[3],
            ("area".into(), SymbolKind::Method, 16, 16, None)
        );
        assert_eq!(
            symbols[4],
            ("helper".into(), SymbolKind::Function, 19, 21, Some(1))
        );
        assert_eq!(analysis.max_nesting, 2);
    }

    #[test]
    fn nested_closures_are_not_double_counted() {
        let source = "function outer(items) {\n  if (items) {\n    items.forEach(function inner(x) {\n      if (x && x.ok) { log(x); }\n    });\n  }\n}\n";
        let analysis = analyze("javascript", source);
        let outer = analysis.symbols.iter().find(|s| s.name == "outer").unwrap();
        let inner = analysis.symbols.iter().find(|s| s.name == "inner").unwrap();
        assert_eq!(outer.complexity, Some(2), "outer: 1 + if");
        assert_eq!(inner.complexity, Some(3), "inner: 1 + if + &&");
        assert_eq!((outer.line, outer.end_line), (1, 7));
    }

    #[test]
    fn python_indentation_bodies() {
        let source = "class Repo:\n    \"\"\"Docs.\n\nMore docs at column zero.\n\"\"\"\n\n    def scan(self, path):\n        for item in path:\n            if item and item.ok:\n                yield item\n\n        return None\n\n    def short(self): return 1\n\ndef top():\n    pass\n";
        let analysis = analyze("python", source);
        let symbols = summary(&analysis);
        assert_eq!(symbols[0].0, "Repo");
        assert_eq!(
            symbols[1],
            ("scan".into(), SymbolKind::Method, 7, 12, Some(4))
        );
        assert_eq!(
            symbols[2],
            ("short".into(), SymbolKind::Method, 14, 14, Some(1))
        );
        assert_eq!(
            symbols[3],
            ("top".into(), SymbolKind::Function, 16, 17, Some(1))
        );
        let scan_symbol = &analysis.symbols[1];
        assert_eq!(scan_symbol.nesting, Some(2));
    }

    #[test]
    fn c_prototypes_and_calls_are_not_functions() {
        let source = "int add(int a, int b);\n\nint add(int a, int b)\n{\n    return a > b ? a : b;\n}\n\nstatic void log_all(void) {\n    for (int i = 0; i < 3; i++) { add(i, i); }\n}\n";
        let analysis = analyze("c", source);
        let names: Vec<_> = analysis
            .symbols
            .iter()
            .map(|s| (s.name.as_str(), s.line, s.complexity))
            .collect();
        assert_eq!(names, vec![("add", 3, Some(2)), ("log_all", 8, Some(2))]);
    }

    #[test]
    fn java_methods_skip_control_flow() {
        let source = "public class Service {\n    public int handle(int x) throws Exception {\n        if (x > 0) {\n            return x;\n        }\n        while (x < 0) { x++; }\n        return 0;\n    }\n}\n";
        let analysis = analyze("java", source);
        let names: Vec<_> = analysis
            .symbols
            .iter()
            .map(|s| (s.name.as_str(), s.kind, s.complexity))
            .collect();
        assert_eq!(
            names,
            vec![
                ("Service", SymbolKind::Class, None),
                ("handle", SymbolKind::Method, Some(3))
            ]
        );
    }

    #[test]
    fn ruby_end_keyword_bodies() {
        let source = "class Cart\n  def total(items)\n    items.each do |item|\n      next unless item\n    end\n    42\n  end\n\n  def name = \"cart\"\nend\n";
        let analysis = analyze("ruby", source);
        let total = analysis.symbols.iter().find(|s| s.name == "total").unwrap();
        assert_eq!((total.line, total.end_line), (2, 7));
        assert_eq!(total.complexity, Some(2));
        let name = analysis.symbols.iter().find(|s| s.name == "name").unwrap();
        assert_eq!((name.line, name.end_line), (9, 9));
    }

    #[test]
    fn go_receivers_are_methods() {
        let source = "package main\n\ntype Server struct{}\n\nfunc (s *Server) Start() error {\n\tif s == nil {\n\t\treturn nil\n\t}\n\treturn nil\n}\n\nfunc main() {}\n";
        let analysis = analyze("go", source);
        let names: Vec<_> = analysis
            .symbols
            .iter()
            .map(|s| (s.name.as_str(), s.kind))
            .collect();
        assert_eq!(
            names,
            vec![
                ("Server", SymbolKind::Struct),
                ("Start", SymbolKind::Method),
                ("main", SymbolKind::Function)
            ]
        );
        assert_eq!(analysis.symbols[1].complexity, Some(2));
    }

    #[test]
    fn typescript_arrow_functions_and_methods() {
        let source = "export const double = (x: number): number => x * 2;\nexport class Store {\n  load(id: string): Item {\n    return id ? find(id) : empty;\n  }\n}\n";
        let analysis = analyze("typescript", source);
        let names: Vec<_> = analysis
            .symbols
            .iter()
            .map(|s| (s.name.as_str(), s.kind, s.complexity))
            .collect();
        assert_eq!(
            names,
            vec![
                ("double", SymbolKind::Function, Some(1)),
                ("Store", SymbolKind::Class, None),
                ("load", SymbolKind::Method, Some(2))
            ]
        );
    }

    #[test]
    fn sql_schema_objects() {
        let analysis = analyze(
            "sql",
            "CREATE TABLE IF NOT EXISTS \"users\" (id int);\ncreate or replace view active_users as select 1;\n",
        );
        let names: Vec<_> = analysis.symbols.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["users", "active_users"]);
    }

    #[test]
    fn top_level_script_complexity() {
        let analysis = analyze(
            "shell",
            "if [ -f x ]; then\n  echo ok\nfi\ncheck() {\n  [ -n \"$1\" ] && echo yes\n}\n",
        );
        assert_eq!(analysis.module_complexity, 1);
        assert_eq!(analysis.symbols[0].complexity, Some(2));
    }
}
