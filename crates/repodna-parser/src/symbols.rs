//! Symbol detection, function extents, approximate cyclomatic complexity, and nesting.
//!
//! Complexity follows McCabe's definition — one plus the number of decision points — with
//! decision points recognized lexically (keywords such as `if` or `case`, short-circuit
//! operators, and ternaries). Each line's decision points are attributed to the innermost
//! function that contains it, so nested functions and closures are not double-counted.
//! Results are approximations and are labeled as such everywhere they are shown.

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

fn keyword_body(language: &str, lines: &[ScannedLine], start_line: usize) -> Body {
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

    // Attribute each line's decision points to the innermost measurable function.
    let mut owner: Vec<Option<usize>> = vec![None; lines.len()];
    let mut order: Vec<usize> = (0..symbols.len())
        .filter(|&i| symbols[i].complexity.is_some())
        .collect();
    order.sort_by(|&a, &b| {
        symbols[a]
            .line
            .cmp(&symbols[b].line)
            .then_with(|| symbols[b].end_line.cmp(&symbols[a].end_line))
    });
    for &symbol in &order {
        let start = symbols[symbol].line as usize - 1;
        let end = (symbols[symbol].end_line as usize).min(lines.len());
        for slot in owner.iter_mut().take(end).skip(start) {
            *slot = Some(symbol);
        }
    }
    let mut module_complexity = 0u32;
    for (index, line) in lines.iter().enumerate() {
        if line.kind != LineKind::Code || spec.complexity.is_empty() {
            continue;
        }
        let decisions = count_decisions(&spec.complexity, &line.masked);
        if decisions == 0 {
            continue;
        }
        match owner[index] {
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
