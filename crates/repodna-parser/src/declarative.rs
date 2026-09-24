//! Declarative language definitions (language plugins).
//!
//! A language plugin is a TOML file describing a language's syntax and patterns. It is pure
//! data: loading it never executes code, and every expression is compiled by the `regex`
//! crate, which guarantees linear-time matching, so a plugin cannot cause catastrophic
//! backtracking.
//!
//! ```toml
//! id = "zig"
//! name = "Zig"
//! kind = "programming"
//! extensions = ["zig"]
//! line_comments = ["//"]
//! strings = [{ open = "\"", escape = "\\" }]
//! imports = ['@import\("([^"]+)"\)']
//! functions = ['^\s*(?:pub\s+)?fn\s+([A-Za-z_]\w*)']
//! body = "braces"
//! complexity_keywords = ["if", "for", "while", "switch", "catch", "orelse"]
//! complexity_operators = ["and", "or"]
//! ```

use std::path::Path;

use regex::Regex;
use repodna_core::model::languages::LanguageKind;
use repodna_core::model::structure::SymbolKind;
use serde::Deserialize;

use crate::spec::{
    BlockComment, BodyStyle, ComplexityRules, ImportKind, ImportPattern, LanguageSpec, LineComment,
    StringRule, SymbolPattern,
};

/// Maximum size of a language definition file.
const MAX_DEFINITION_BYTES: u64 = 256 * 1024;
/// Maximum length of one expression, to keep compiled automata small.
const MAX_PATTERN_LENGTH: usize = 1_000;

/// Error in a language definition.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("language definition {origin}: {message}")]
pub struct DefinitionError {
    origin: String,
    message: String,
}

impl DefinitionError {
    fn new(origin: &str, message: impl Into<String>) -> Self {
        Self {
            origin: origin.to_owned(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StringDefinition {
    open: String,
    #[serde(default)]
    close: Option<String>,
    #[serde(default)]
    escape: Option<String>,
    #[serde(default)]
    multiline: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TypeDefinition {
    pattern: String,
    #[serde(default = "default_type_kind")]
    kind: String,
}

fn default_type_kind() -> String {
    "type".to_owned()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LanguageDefinition {
    id: String,
    name: String,
    #[serde(default = "default_kind")]
    kind: LanguageKind,
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default)]
    filenames: Vec<String>,
    #[serde(default)]
    shebangs: Vec<String>,
    #[serde(default)]
    line_comments: Vec<String>,
    #[serde(default)]
    block_comments: Vec<[String; 2]>,
    #[serde(default)]
    nested_block_comments: bool,
    #[serde(default)]
    strings: Vec<StringDefinition>,
    #[serde(default)]
    imports: Vec<String>,
    #[serde(default)]
    package: Option<String>,
    #[serde(default)]
    functions: Vec<String>,
    #[serde(default)]
    types: Vec<TypeDefinition>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    complexity_keywords: Vec<String>,
    #[serde(default)]
    complexity_operators: Vec<String>,
}

fn default_kind() -> LanguageKind {
    LanguageKind::Programming
}

fn compile(origin: &str, pattern: &str) -> Result<Regex, DefinitionError> {
    if pattern.len() > MAX_PATTERN_LENGTH {
        return Err(DefinitionError::new(
            origin,
            format!("pattern longer than {MAX_PATTERN_LENGTH} characters"),
        ));
    }
    let regex = regex::RegexBuilder::new(pattern)
        .size_limit(1 << 20)
        .build()
        .map_err(|error| {
            DefinitionError::new(origin, format!("invalid pattern {pattern:?}: {error}"))
        })?;
    if regex.captures_len() < 2 {
        return Err(DefinitionError::new(
            origin,
            format!("pattern {pattern:?} needs a capture group for the name or path"),
        ));
    }
    Ok(regex)
}

fn symbol_kind(origin: &str, kind: &str) -> Result<SymbolKind, DefinitionError> {
    Ok(match kind {
        "function" => SymbolKind::Function,
        "method" => SymbolKind::Method,
        "class" => SymbolKind::Class,
        "struct" => SymbolKind::Struct,
        "enum" => SymbolKind::Enum,
        "interface" => SymbolKind::Interface,
        "trait" => SymbolKind::Trait,
        "module" => SymbolKind::Module,
        "type" => SymbolKind::Type,
        "constant" => SymbolKind::Constant,
        "macro" => SymbolKind::Macro,
        other => {
            return Err(DefinitionError::new(
                origin,
                format!("unknown symbol kind {other:?}"),
            ));
        }
    })
}

/// Parses a language definition from TOML text.
pub fn parse_definition(text: &str, origin: &str) -> Result<LanguageSpec, DefinitionError> {
    let definition: LanguageDefinition =
        toml::from_str(text).map_err(|error| DefinitionError::new(origin, error.to_string()))?;
    if definition.id.is_empty()
        || !definition
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(DefinitionError::new(
            origin,
            "id must be non-empty and contain only lowercase letters, digits, and hyphens",
        ));
    }
    if definition.extensions.is_empty() && definition.filenames.is_empty() {
        return Err(DefinitionError::new(
            origin,
            "at least one extension or file name is required",
        ));
    }

    let mut spec = LanguageSpec::new(&definition.id, &definition.name, definition.kind);
    spec.extensions = definition
        .extensions
        .iter()
        .map(|e| e.trim_start_matches('.').to_ascii_lowercase())
        .collect();
    spec.filenames = definition.filenames;
    spec.shebangs = definition.shebangs;
    spec.syntax.line_comments = definition
        .line_comments
        .into_iter()
        .map(|token| LineComment {
            token,
            requires_boundary: false,
        })
        .collect();
    spec.syntax.block_comments = definition
        .block_comments
        .into_iter()
        .map(|[open, close]| BlockComment {
            open,
            close,
            nested: definition.nested_block_comments,
        })
        .collect();
    for string in definition.strings {
        let escape = match string.escape.as_deref() {
            None | Some("") => None,
            Some(escape) if escape.chars().count() == 1 => escape.chars().next(),
            Some(_) => {
                return Err(DefinitionError::new(
                    origin,
                    "string escape must be a single character",
                ));
            }
        };
        if string.open.is_empty() {
            return Err(DefinitionError::new(
                origin,
                "string delimiters must not be empty",
            ));
        }
        spec.syntax.strings.push(StringRule {
            close: string.close.unwrap_or_else(|| string.open.clone()),
            open: string.open,
            escape,
            multiline: string.multiline,
        });
    }
    for pattern in &definition.imports {
        spec.imports.push(ImportPattern {
            regex: compile(origin, pattern)?,
            group: 1,
            kind: ImportKind::Import,
        });
    }
    if let Some(pattern) = &definition.package {
        spec.package = Some(compile(origin, pattern)?);
    }
    for pattern in &definition.functions {
        spec.functions.push(SymbolPattern {
            regex: compile(origin, pattern)?,
            group: 1,
            kind: SymbolKind::Function,
        });
    }
    for definition in &definition.types {
        spec.types.push(SymbolPattern {
            regex: compile(origin, &definition.pattern)?,
            group: 1,
            kind: symbol_kind(origin, &definition.kind)?,
        });
    }
    spec.body = match definition.body.as_deref() {
        None | Some("none") => BodyStyle::None,
        Some("braces") => BodyStyle::Braces,
        Some("indentation") => BodyStyle::Indentation,
        Some("end") => BodyStyle::EndKeyword,
        Some(other) => {
            return Err(DefinitionError::new(
                origin,
                format!("unknown body style {other:?}; expected braces, indentation, end, or none"),
            ));
        }
    };
    let keywords: Vec<&str> = definition
        .complexity_keywords
        .iter()
        .map(String::as_str)
        .collect();
    let operators: Vec<&str> = definition
        .complexity_operators
        .iter()
        .map(String::as_str)
        .collect();
    spec.complexity = ComplexityRules::new(&keywords, &operators, false);
    spec.duplication = spec.kind == LanguageKind::Programming;
    Ok(spec)
}

/// Loads every `*.toml` language definition in `directory`.
///
/// Returns the specifications that loaded and a message for each file that did not.
pub fn load_directory(directory: &Path) -> (Vec<LanguageSpec>, Vec<String>) {
    let mut specs = Vec::new();
    let mut errors = Vec::new();
    let Ok(entries) = std::fs::read_dir(directory) else {
        return (specs, errors);
    };
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    paths.sort();
    for path in paths {
        let origin = path.display().to_string();
        let too_large =
            std::fs::metadata(&path).map_or(true, |meta| meta.len() > MAX_DEFINITION_BYTES);
        if too_large {
            errors.push(format!(
                "{origin}: unreadable or larger than {MAX_DEFINITION_BYTES} bytes"
            ));
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(text) => match parse_definition(&text, &origin) {
                Ok(spec) => specs.push(spec),
                Err(error) => errors.push(error.to_string()),
            },
            Err(error) => errors.push(format!("{origin}: {error}")),
        }
    }
    (specs, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::analyze_source;
    use repodna_core::model::languages::ParserCapability;

    const ZIG: &str = r#"
id = "zig"
name = "Zig"
extensions = [".zig"]
line_comments = ["//"]
strings = [{ open = "\"", escape = "\\" }]
imports = ['@import\("([^"]+)"\)']
functions = ['^\s*(?:pub\s+)?fn\s+([A-Za-z_]\w*)']
types = [{ pattern = '^\s*(?:pub\s+)?const\s+([A-Z]\w*)\s*=\s*struct', kind = "struct" }]
body = "braces"
complexity_keywords = ["if", "for", "while", "switch", "catch", "orelse"]
complexity_operators = ["and", "or"]
"#;

    #[test]
    fn parses_and_uses_a_definition() {
        let spec = parse_definition(ZIG, "zig.toml").unwrap();
        assert_eq!(spec.extensions, vec!["zig"]);
        assert_eq!(spec.capability(), ParserCapability::Lexical);
        let source = "const std = @import(\"std\");\n// entry\npub fn main() void {\n    if (true) {}\n}\nconst Point = struct { x: i32 };\n";
        let analysis = analyze_source(&spec, source);
        assert_eq!(analysis.imports[0].specifier, "std");
        let random = analyze_source(&spec, "fn f() void {\n    const random = operand;\n}\n");
        assert_eq!(
            random.symbols[0].complexity,
            Some(1),
            "word operators need word boundaries"
        );
        let names: Vec<_> = analysis.symbols.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["main", "Point"]);
        assert_eq!(analysis.symbols[0].complexity, Some(2));
        assert_eq!(analysis.lines.comment, 1);
    }

    #[test]
    fn rejects_invalid_definitions() {
        assert!(
            parse_definition("id = \"Bad Id\"\nname = \"x\"\nextensions = [\"x\"]", "a").is_err()
        );
        assert!(parse_definition("id = \"x\"\nname = \"x\"", "a").is_err());
        assert!(
            parse_definition(
                "id = \"x\"\nname = \"x\"\nextensions = [\"x\"]\nfunctions = ['fn \\w+']",
                "a"
            )
            .unwrap_err()
            .to_string()
            .contains("capture group")
        );
        assert!(
            parse_definition(
                "id = \"x\"\nname = \"x\"\nextensions = [\"x\"]\nbody = \"curly\"",
                "a"
            )
            .is_err()
        );
        assert!(
            parse_definition(
                "id = \"x\"\nname = \"x\"\nextensions = [\"x\"]\nunknown = 1",
                "a"
            )
            .is_err()
        );
    }

    #[test]
    fn loads_a_directory_and_reports_errors() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("zig.toml"), ZIG).unwrap();
        std::fs::write(dir.path().join("broken.toml"), "id = ").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "ignored").unwrap();
        let (specs, errors) = load_directory(dir.path());
        assert_eq!(specs.len(), 1);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("broken.toml"));
    }
}
