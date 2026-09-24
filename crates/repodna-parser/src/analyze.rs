//! Per-file analysis entry point.

use std::sync::LazyLock;

use regex::Regex;
use repodna_core::model::languages::{LanguageKind, ParserCapability};
use repodna_core::model::structure::{FileAnalysisSummary, LineCounts};
use serde::{Deserialize, Serialize};

use crate::imports::{RawImport, extract_imports, extract_package};
use crate::markers::{RawMarker, extract_markers};
use crate::scanner::scan;
use crate::spec::{ImportExtractor, LanguageSpec};
use crate::symbols::{ParsedSymbol, extract_symbols};

/// Version of the per-file analysis output. Bump whenever results for the same input can
/// change, so cached results are invalidated.
pub const ANALYZER_VERSION: u32 = 1;

/// Maximum string-literal file references recorded per file.
const MAX_REFERENCES: usize = 500;

/// A string literal that looks like a path to another repository file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawReference {
    /// The path as written.
    pub path: String,
    /// Line (1-based).
    pub line: u32,
}

/// Everything lexical analysis learned about one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileAnalysis {
    /// Language identifier.
    pub language: String,
    /// Depth of analysis applied.
    pub capability: ParserCapability,
    /// Line counts.
    pub lines: LineCounts,
    /// Import statements.
    #[serde(default)]
    pub imports: Vec<RawImport>,
    /// Symbols.
    #[serde(default)]
    pub symbols: Vec<ParsedSymbol>,
    /// Marker comments.
    #[serde(default)]
    pub markers: Vec<RawMarker>,
    /// String-literal references to other files.
    #[serde(default)]
    pub references: Vec<RawReference>,
    /// Declared package or namespace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    /// Decision points outside functions.
    pub module_complexity: u32,
    /// Deepest nesting inside any function.
    pub max_nesting: u32,
}

impl FileAnalysis {
    /// Functions and methods with measured bodies.
    pub fn functions(&self) -> impl Iterator<Item = &ParsedSymbol> {
        self.symbols
            .iter()
            .filter(|symbol| symbol.is_function() && symbol.complexity.is_some())
    }

    /// Compact summary stored on the file record.
    pub fn summary(&self) -> FileAnalysisSummary {
        let mut summary = FileAnalysisSummary {
            imports: u32::try_from(self.imports.len()).unwrap_or(u32::MAX),
            symbols: u32::try_from(self.symbols.len()).unwrap_or(u32::MAX),
            markers: u32::try_from(self.markers.len()).unwrap_or(u32::MAX),
            max_nesting: self.max_nesting,
            ..FileAnalysisSummary::default()
        };
        for function in self.functions() {
            let complexity = function.complexity.unwrap_or(1);
            summary.functions += 1;
            summary.cyclomatic_total = summary.cyclomatic_total.saturating_add(complexity);
            summary.cyclomatic_max = summary.cyclomatic_max.max(complexity);
            summary.longest_function = summary.longest_function.max(function.length());
        }
        summary
    }
}

static REFERENCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"["'`]((?:\.{1,2}/)*[\w@.\-]+(?:/[\w@.\-]+)*\.(?:rs|py|js|mjs|cjs|jsx|ts|tsx|go|java|kt|c|h|cc|cpp|hpp|cs|php|rb|swift|dart|sh|sql|html|htm|css|scss|json|yaml|yml|toml|xml|md|proto|wasm|graphql|lua|vue|svelte))["'`]"#,
    )
    .unwrap_or_else(|error| panic!("invalid reference pattern: {error}"))
});

/// Analyzes source text with the given language specification.
pub fn analyze_source(spec: &LanguageSpec, text: &str) -> FileAnalysis {
    let scanned = scan(text, &spec.syntax);
    let lines = scanned.line_counts();
    let imports = if spec.imports.is_empty() && spec.import_extractor == ImportExtractor::Patterns {
        Vec::new()
    } else {
        extract_imports(spec, &scanned.lines)
    };
    let symbol_analysis = if spec.functions.is_empty() && spec.types.is_empty() {
        None
    } else {
        Some(extract_symbols(spec, &scanned.lines))
    };
    let markers = extract_markers(&scanned.lines);
    let references = if spec.kind == LanguageKind::Programming {
        extract_references(&scanned.lines, &imports)
    } else {
        Vec::new()
    };
    let package = extract_package(spec, &scanned.lines);
    let (symbols, module_complexity, max_nesting) = match symbol_analysis {
        Some(analysis) => (
            analysis.symbols,
            analysis.module_complexity,
            analysis.max_nesting,
        ),
        None => (Vec::new(), 0, 0),
    };
    FileAnalysis {
        language: spec.id.clone(),
        capability: spec.capability(),
        lines,
        imports,
        symbols,
        markers,
        references,
        package,
        module_complexity,
        max_nesting,
    }
}

fn extract_references(
    lines: &[crate::scanner::ScannedLine],
    imports: &[RawImport],
) -> Vec<RawReference> {
    let mut references = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !line.code.contains(['"', '\'', '`']) {
            continue;
        }
        let line_number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let code = line.code.as_bytes();
        let masked = line.masked.as_bytes();
        for captures in REFERENCE.captures_iter(&line.code) {
            let (Some(whole), Some(path)) = (captures.get(0), captures.get(1)) else {
                continue;
            };
            // The opening quote must be a real delimiter, not text inside another string.
            if masked.get(whole.start()) != code.get(whole.start()) {
                continue;
            }
            let path = path.as_str();
            let is_import = imports
                .iter()
                .any(|import| import.line == line_number && import.specifier == path);
            if !is_import {
                references.push(RawReference {
                    path: path.to_owned(),
                    line: line_number,
                });
            }
            if references.len() >= MAX_REFERENCES {
                return references;
            }
        }
    }
    references
}

/// Counts lines only, for files whose language has no lexical rules.
pub fn count_lines(spec: &LanguageSpec, text: &str) -> LineCounts {
    scan(text, &spec.syntax).line_counts()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::LanguageRegistry;

    fn analyze(language: &str, source: &str) -> FileAnalysis {
        analyze_source(LanguageRegistry::builtin().get(language).unwrap(), source)
    }

    #[test]
    fn summarizes_a_typescript_module() {
        let source = "import { api } from './api';\n\n// TODO: cache results\nexport function load(id: string) {\n  if (!id) { return null; }\n  return fetch(`/items/${id}`);\n}\nconst template = './views/item.html';\n";
        let analysis = analyze("typescript", source);
        assert_eq!(analysis.capability, ParserCapability::Lexical);
        assert_eq!(analysis.lines.total, 8);
        assert_eq!(analysis.lines.comment, 1);
        assert_eq!(analysis.lines.blank, 1);
        assert_eq!(analysis.imports.len(), 1);
        assert_eq!(analysis.markers.len(), 1);
        assert_eq!(
            analysis.references,
            vec![RawReference {
                path: "./views/item.html".into(),
                line: 8
            }]
        );
        let summary = analysis.summary();
        assert_eq!(summary.functions, 1);
        assert_eq!(summary.cyclomatic_max, 2);
        assert_eq!(summary.longest_function, 4);
        assert_eq!(summary.imports, 1);
    }

    #[test]
    fn data_files_only_count_lines() {
        let analysis = analyze("yaml", "# config\nname: demo\n\nitems:\n  - a\n");
        assert_eq!(analysis.capability, ParserCapability::LineCount);
        assert!(analysis.symbols.is_empty());
        assert!(analysis.references.is_empty());
        assert_eq!(
            (
                analysis.lines.code,
                analysis.lines.comment,
                analysis.lines.blank
            ),
            (3, 1, 1)
        );
    }

    #[test]
    fn analysis_round_trips_through_json_for_caching() {
        let analysis = analyze(
            "python",
            "import os\n\ndef f(x):\n    return x if x else os.sep\n",
        );
        let json = serde_json::to_string(&analysis).unwrap();
        let back: FileAnalysis = serde_json::from_str(&json).unwrap();
        assert_eq!(back, analysis);
    }

    #[test]
    fn imports_are_not_duplicated_as_references() {
        let analysis = analyze("javascript", "const cfg = require('./config.json');\n");
        assert_eq!(analysis.imports.len(), 1);
        assert!(analysis.references.is_empty());
    }
}
