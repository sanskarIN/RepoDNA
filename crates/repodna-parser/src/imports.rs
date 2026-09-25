//! Import extraction.
//!
//! Most languages are handled by per-line patterns. Rust `use` trees, Go import blocks,
//! and Python `from … import (…)` statements span lines or need expansion, so they have
//! dedicated extractors. JavaScript and TypeScript use their patterns, but statements that
//! only import or re-export types are left out: the compiler erases them.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::scanner::ScannedLine;
use crate::spec::{ImportExtractor, ImportKind, LanguageSpec};

/// An import statement as written in the source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawImport {
    /// The imported module, package, or path, e.g. `./util`, `crate::model`, or `fmt`.
    pub specifier: String,
    /// Line of the statement (1-based).
    pub line: u32,
    /// Kind of import.
    pub kind: ImportKind,
    /// Names imported from the module (Python `from x import a, b`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub names: Vec<String>,
}

impl RawImport {
    fn new(specifier: impl Into<String>, line: usize, kind: ImportKind) -> Self {
        Self {
            specifier: specifier.into(),
            line: u32::try_from(line + 1).unwrap_or(u32::MAX),
            kind,
            names: Vec::new(),
        }
    }
}

/// Maximum number of imports recorded per file, a guard against pathological inputs.
const MAX_IMPORTS: usize = 2_000;

/// Extracts imports from scanned lines.
pub fn extract_imports(spec: &LanguageSpec, lines: &[ScannedLine]) -> Vec<RawImport> {
    let mut imports = match spec.import_extractor {
        ImportExtractor::Patterns | ImportExtractor::JavaScript => Vec::new(),
        ImportExtractor::Rust => rust_imports(lines),
        ImportExtractor::Go => go_imports(lines),
        ImportExtractor::Python => python_imports(lines),
    };
    let type_only = if spec.import_extractor == ImportExtractor::JavaScript {
        type_only_lines(lines)
    } else {
        HashSet::new()
    };
    if !spec.imports.is_empty() {
        for (index, line) in lines.iter().enumerate() {
            if line.code.trim().is_empty() || type_only.contains(&index) {
                continue;
            }
            for pattern in &spec.imports {
                for captures in pattern.regex.captures_iter(&line.code) {
                    let Some(whole) = captures.get(0) else {
                        continue;
                    };
                    if !starts_in_code(line, whole.start(), whole.end()) {
                        continue;
                    }
                    if let Some(specifier) = captures.get(pattern.group) {
                        let specifier = specifier.as_str().trim();
                        if !specifier.is_empty() {
                            imports.push(RawImport::new(specifier, index, pattern.kind));
                        }
                    }
                }
            }
            if imports.len() >= MAX_IMPORTS {
                break;
            }
        }
    }
    imports.sort_by(|a, b| {
        a.line
            .cmp(&b.line)
            .then_with(|| a.specifier.cmp(&b.specifier))
    });
    imports.dedup();
    imports.truncate(MAX_IMPORTS);
    imports
}

static FROM_CLAUSE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\bfrom\s*["'][^"']+["']"#).expect("the from-clause pattern is valid")
});

/// Lines holding the `from "…"` clause of a TypeScript statement that imports or re-exports
/// only types: `import type …`, `export type { … } from`, or braces whose every name is
/// marked `type`. Statements may span lines, so they are followed from their first line.
fn type_only_lines(lines: &[ScannedLine]) -> HashSet<usize> {
    let mut found = HashSet::new();
    let mut statement: Option<String> = None;
    for (index, line) in lines.iter().enumerate() {
        let code = line.code.trim();
        if starts_module_statement(code) {
            statement = Some(String::new());
        }
        let Some(text) = statement.as_mut() else {
            continue;
        };
        text.push_str(code);
        text.push(' ');
        if FROM_CLAUSE.is_match(code) {
            if is_type_only(text) {
                found.insert(index);
            }
            statement = None;
        } else if code.ends_with(';') || text.len() > 4_000 {
            statement = None;
        }
    }
    found
}

/// `import …` (not a dynamic `import(…)`) or a re-export `export {`, `export *`, `export type`.
fn starts_module_statement(code: &str) -> bool {
    if let Some(rest) = code.strip_prefix("import") {
        return rest.starts_with([' ', '{', '*']) && !rest.trim_start().starts_with('(');
    }
    code.strip_prefix("export")
        .map(str::trim_start)
        .is_some_and(|rest| rest.starts_with(['{', '*']) || rest.starts_with("type "))
}

/// Whether an import or export statement brings in types only.
fn is_type_only(statement: &str) -> bool {
    let rest = statement
        .strip_prefix("import")
        .or_else(|| statement.strip_prefix("export"))
        .unwrap_or(statement)
        .trim_start();
    if let Some(after) = rest.strip_prefix("type") {
        // `import type from "./type"` imports a value named `type`.
        let after = after.trim_start();
        return rest[4..].starts_with([' ', '{', '*']) && !after.starts_with("from");
    }
    let Some(open) = rest.find('{') else {
        return false;
    };
    if !rest[..open].trim().is_empty() {
        // A default or namespace import beside the braces is a value import.
        return false;
    }
    let Some(close) = rest[open..].find('}') else {
        return false;
    };
    let names: Vec<&str> = rest[open + 1..open + close]
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect();
    !names.is_empty() && names.iter().all(|name| name.starts_with("type "))
}

/// Returns `true` when the first non-whitespace byte of `code[start..end]` is real code
/// rather than the contents of a string literal. `code` and `masked` are byte-aligned, and
/// string contents are blanked in `masked`.
fn starts_in_code(line: &ScannedLine, start: usize, end: usize) -> bool {
    let code = line.code.as_bytes();
    let masked = line.masked.as_bytes();
    (start..end.min(code.len()))
        .find(|&index| !code[index].is_ascii_whitespace())
        .is_some_and(|index| masked.get(index) == Some(&code[index]))
}

/// Extracts the package or namespace declaration, if the language has one.
pub fn extract_package(spec: &LanguageSpec, lines: &[ScannedLine]) -> Option<String> {
    let pattern = spec.package.as_ref()?;
    lines.iter().find_map(|line| {
        pattern
            .captures(&line.code)
            .and_then(|captures| captures.get(1))
            .map(|m| m.as_str().to_owned())
    })
}

static RUST_MOD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+([A-Za-z_]\w*)\s*;")
        .unwrap_or_else(|error| panic!("invalid Rust mod pattern: {error}"))
});
static RUST_EXTERN_CRATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*extern\s+crate\s+([A-Za-z_]\w*)")
        .unwrap_or_else(|error| panic!("invalid Rust extern crate pattern: {error}"))
});
static RUST_USE_START: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?use\s+")
        .unwrap_or_else(|error| panic!("invalid Rust use pattern: {error}"))
});
static RUST_INLINE_MOD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+[A-Za-z_]\w*\s*\{")
        .unwrap_or_else(|error| panic!("invalid Rust inline mod pattern: {error}"))
});

fn rust_imports(lines: &[ScannedLine]) -> Vec<RawImport> {
    let mut imports = Vec::new();
    // Brace depth, and the depths at which enclosing inline `mod name { … }` blocks opened.
    let mut depth = 0usize;
    let mut inline_modules: Vec<usize> = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let text = &lines[index].masked;
        let first_line = index;
        if let Some(captures) = RUST_MOD.captures(text) {
            imports.push(RawImport::new(&captures[1], index, ImportKind::Module));
        } else if let Some(captures) = RUST_EXTERN_CRATE.captures(text) {
            imports.push(RawImport::new(&captures[1], index, ImportKind::Import));
        } else if RUST_INLINE_MOD.is_match(text) {
            inline_modules.push(depth);
        } else if let Some(start) = RUST_USE_START.find(text) {
            // Accumulate the statement until its terminating semicolon (at most 50 lines).
            let mut statement = text[start.end()..].to_owned();
            while !statement.contains(';') && index + 1 < lines.len() && index - first_line < 50 {
                index += 1;
                statement.push(' ');
                statement.push_str(&lines[index].masked);
            }
            let tree = normalize_use_tree(statement.split(';').next().unwrap_or_default());
            let tree = rebase_super(&tree, inline_modules.len());
            let mut paths = Vec::new();
            expand_use_tree("", &tree, &mut paths);
            for path in paths {
                imports.push(RawImport::new(path, first_line, ImportKind::Import));
            }
        }
        for line in &lines[first_line..=index] {
            for byte in line.masked.bytes() {
                match byte {
                    b'{' => depth += 1,
                    b'}' => {
                        depth = depth.saturating_sub(1);
                        while inline_modules.last().is_some_and(|&open| open >= depth) {
                            inline_modules.pop();
                        }
                    }
                    _ => {}
                }
            }
        }
        index += 1;
    }
    imports
}

/// Rewrites a use tree written inside `inline_depth` inline `mod name { … }` blocks so that
/// it is relative to the file's own module: inside `mod tests { use super::*; }` the
/// `super` is the file itself, not its parent module.
fn rebase_super(tree: &str, inline_depth: usize) -> String {
    if inline_depth == 0 {
        return tree.to_owned();
    }
    let mut rest = tree;
    let mut stripped = 0;
    while stripped < inline_depth {
        if let Some(after) = rest.strip_prefix("super::") {
            rest = after;
            stripped += 1;
        } else if rest == "super" {
            return "self".to_owned();
        } else {
            break;
        }
    }
    if stripped == 0 {
        tree.to_owned()
    } else if stripped == inline_depth && (rest == "super" || rest.starts_with("super::")) {
        rest.to_owned()
    } else {
        format!("self::{rest}")
    }
}

/// Expands a Rust use tree such as `a::{b, c::{d, e as f}}` into full paths.
fn expand_use_tree(prefix: &str, tree: &str, out: &mut Vec<String>) {
    let join = |head: &str| -> String {
        let head = head.trim_start_matches("::");
        if prefix.is_empty() {
            head.to_owned()
        } else if head.is_empty() {
            prefix.to_owned()
        } else {
            format!("{prefix}::{head}")
        }
    };
    match tree.find('{') {
        None => {
            let path = tree.split(" as ").next().unwrap_or(tree).trim();
            let path = path.trim_end_matches("::*").trim_end_matches("::self");
            if path == "self" || path == "*" {
                if !prefix.is_empty() {
                    out.push(prefix.to_owned());
                }
            } else if !path.is_empty() {
                out.push(join(path));
            }
        }
        Some(open) => {
            let head = tree[..open].trim_end_matches("::");
            let new_prefix = join(head);
            let Some(inner) = matching_brace_contents(&tree[open..]) else {
                return;
            };
            for part in split_top_level(inner) {
                if !part.is_empty() {
                    expand_use_tree(&new_prefix, part, out);
                }
            }
        }
    }
}

/// Collapses whitespace in a use tree and removes it around `::`, braces, and commas, so
/// `a :: { b , c as d }` becomes `a::{b,c as d}` while aliases keep their spaces.
fn normalize_use_tree(tree: &str) -> String {
    let collapsed = tree.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars: Vec<char> = collapsed.chars().collect();
    let mut out = String::with_capacity(collapsed.len());
    for (index, &c) in chars.iter().enumerate() {
        if c == ' ' {
            let previous = index.checked_sub(1).and_then(|i| chars.get(i)).copied();
            let next = chars.get(index + 1).copied();
            let punctuation = |c: Option<char>| matches!(c, Some('{' | '}' | ',' | ':'));
            if punctuation(previous) || punctuation(next) {
                continue;
            }
        }
        out.push(c);
    }
    out
}

fn matching_brace_contents(text: &str) -> Option<&str> {
    let mut depth = 0usize;
    for (index, c) in text.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(&text[1..index]);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (index, c) in text.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&text[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

static QUOTED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"["`]([^"`]+)["`]"#)
        .unwrap_or_else(|error| panic!("invalid quoted pattern: {error}"))
});

fn go_imports(lines: &[ScannedLine]) -> Vec<RawImport> {
    let mut imports = Vec::new();
    let mut in_block = false;
    for (index, line) in lines.iter().enumerate() {
        let text = line.code.trim();
        if in_block {
            if text.starts_with(')') {
                in_block = false;
                continue;
            }
            if let Some(captures) = QUOTED.captures(text) {
                imports.push(RawImport::new(&captures[1], index, ImportKind::Import));
            }
        } else if let Some(rest) = text.strip_prefix("import") {
            let rest = rest.trim_start();
            if let Some(block) = rest.strip_prefix('(') {
                in_block = !block.contains(')');
                for captures in QUOTED.captures_iter(block) {
                    imports.push(RawImport::new(&captures[1], index, ImportKind::Import));
                }
            } else if let Some(captures) = QUOTED.captures(rest) {
                imports.push(RawImport::new(&captures[1], index, ImportKind::Import));
            }
        }
    }
    imports
}

static PY_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*import\s+(.+)$")
        .unwrap_or_else(|error| panic!("invalid Python pattern: {error}"))
});
static PY_FROM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*from\s+(\.*[\w.]*)\s+import\s+(.+)$")
        .unwrap_or_else(|error| panic!("invalid Python pattern: {error}"))
});

fn python_imports(lines: &[ScannedLine]) -> Vec<RawImport> {
    let mut imports = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let text = lines[index].code.trim_end();
        if let Some(captures) = PY_FROM.captures(text) {
            let module = captures[1].to_owned();
            let mut names_text = captures[2].to_owned();
            let first_line = index;
            if names_text.trim_start().starts_with('(') {
                while !names_text.contains(')')
                    && index + 1 < lines.len()
                    && index - first_line < 200
                {
                    index += 1;
                    names_text.push(' ');
                    names_text.push_str(&lines[index].code);
                }
            } else {
                while names_text.trim_end().ends_with('\\') && index + 1 < lines.len() {
                    index += 1;
                    names_text = names_text.trim_end().trim_end_matches('\\').to_owned();
                    names_text.push(' ');
                    names_text.push_str(&lines[index].code);
                }
            }
            let names = names_text
                .replace(['(', ')', '\\'], " ")
                .split(',')
                .filter_map(|name| name.split_whitespace().next())
                .filter(|name| *name != "*" && !name.is_empty())
                .map(str::to_owned)
                .collect();
            let mut import = RawImport::new(module, first_line, ImportKind::Import);
            import.names = names;
            imports.push(import);
        } else if let Some(captures) = PY_IMPORT.captures(text) {
            for module in captures[1].split(',') {
                if let Some(name) = module.split_whitespace().next() {
                    let name = name.trim_end_matches(';');
                    if !name.is_empty()
                        && name
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
                    {
                        imports.push(RawImport::new(name, index, ImportKind::Import));
                    }
                }
            }
        }
        index += 1;
    }
    imports
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::LanguageRegistry;
    use crate::scanner::scan;

    fn imports(language: &str, source: &str) -> Vec<(String, u32)> {
        let spec = LanguageRegistry::builtin().get(language).unwrap();
        let scanned = scan(source, &spec.syntax);
        extract_imports(spec, &scanned.lines)
            .into_iter()
            .map(|import| (import.specifier, import.line))
            .collect()
    }

    fn specifiers(language: &str, source: &str) -> Vec<String> {
        imports(language, source)
            .into_iter()
            .map(|(s, _)| s)
            .collect()
    }

    #[test]
    fn rust_use_trees_expand() {
        let source = "use std::collections::{HashMap, HashSet};\nuse crate::model::{\n    structure::FileRecord,\n    git::{GitReport, CommitRef},\n};\nmod scanner;\npub(crate) mod util;\nextern crate serde;\nuse super::*;\n";
        let found = specifiers("rust", source);
        for expected in [
            "std::collections::HashMap",
            "std::collections::HashSet",
            "crate::model::structure::FileRecord",
            "crate::model::git::GitReport",
            "crate::model::git::CommitRef",
            "scanner",
            "util",
            "serde",
            "super",
        ] {
            assert!(
                found.contains(&expected.to_owned()),
                "{expected} missing from {found:?}"
            );
        }
        let lines: Vec<u32> = imports("rust", source)
            .into_iter()
            .map(|(_, l)| l)
            .collect();
        assert!(
            lines.contains(&2),
            "multi-line use is attributed to its first line"
        );
    }

    #[test]
    fn rust_super_inside_inline_modules_refers_to_the_file() {
        let source = "use super::parent_item;\nfn f() {}\n#[cfg(test)]\nmod tests {\n    use super::*;\n    use super::helper;\n    use super::super::sibling;\n    mod nested {\n        use super::super::Thing;\n    }\n    #[test]\n    fn t() { let _ = 1; }\n}\nuse super::after;\n";
        let found = specifiers("rust", source);
        assert_eq!(
            found,
            vec![
                "super::parent_item",
                "self::helper",
                "super::sibling",
                "self::Thing",
                "super::after",
            ]
        );
    }

    #[test]
    fn rust_aliases_are_stripped() {
        let found = specifiers(
            "rust",
            "use std::io::Result as IoResult;\nuse a::{b as c, d};\n",
        );
        assert_eq!(found, vec!["std::io::Result", "a::b", "a::d"]);
    }

    #[test]
    fn rust_self_and_glob_imports() {
        let found = specifiers("rust", "use crate::config::{self, load::*};\n");
        assert!(found.contains(&"crate::config".to_owned()), "{found:?}");
        assert!(
            found.contains(&"crate::config::load".to_owned()),
            "{found:?}"
        );
    }

    #[test]
    fn javascript_and_typescript_imports() {
        let source = "import React from 'react';\nimport {\n  a,\n  b,\n} from \"./utils/helpers\";\nimport './styles.css';\nconst fs = require('fs');\nconst lazy = () => import('./lazy');\nexport * from '../shared';\n// import fake from 'commented';\nconst s = \"from 'not-an-import'\";\n";
        let found = specifiers("typescript", source);
        assert_eq!(
            found,
            vec![
                "react",
                "./utils/helpers",
                "./styles.css",
                "fs",
                "./lazy",
                "../shared"
            ]
        );
    }

    #[test]
    fn python_imports_with_names() {
        let spec = LanguageRegistry::builtin().get("python").unwrap();
        let source = "import os, sys as system\nfrom .models import (\n    User,\n    Group as G,\n)\nfrom . import views\nfrom a.b import c \\\n    , d\n";
        let scanned = scan(source, &spec.syntax);
        let found = extract_imports(spec, &scanned.lines);
        let summary: Vec<(String, Vec<String>)> = found
            .iter()
            .map(|i| (i.specifier.clone(), i.names.clone()))
            .collect();
        assert!(summary.contains(&("os".into(), vec![])));
        assert!(summary.contains(&("sys".into(), vec![])));
        assert!(summary.contains(&(".models".into(), vec!["User".into(), "Group".into()])));
        assert!(summary.contains(&(".".into(), vec!["views".into()])));
        assert!(summary.contains(&("a.b".into(), vec!["c".into(), "d".into()])));
    }

    #[test]
    fn go_import_blocks() {
        let source = "package main\n\nimport \"fmt\"\nimport (\n    \"os\"\n    str \"strings\"\n    _ \"github.com/lib/pq\"\n)\n";
        assert_eq!(
            specifiers("go", source),
            vec!["fmt", "os", "strings", "github.com/lib/pq"]
        );
    }

    #[test]
    fn c_family_and_other_languages() {
        assert_eq!(
            specifiers("c", "#include \"util.h\"\n#include <stdio.h>\n"),
            vec!["util.h", "stdio.h"]
        );
        assert_eq!(
            specifiers(
                "java",
                "package com.example;\nimport java.util.List;\nimport static org.junit.Assert.*;\n"
            ),
            vec!["java.util.List", "org.junit.Assert.*"]
        );
        assert_eq!(
            specifiers(
                "csharp",
                "using System;\nusing static System.Math;\nusing (var x = y) {}\n"
            ),
            vec!["System", "System.Math"]
        );
        assert_eq!(
            specifiers("ruby", "require 'json'\nrequire_relative '../lib/app'\n"),
            vec!["json", "../lib/app"]
        );
        assert_eq!(
            specifiers("shell", "source ./env.sh\n. lib/common.sh\n"),
            vec!["./env.sh", "lib/common.sh"]
        );
        assert_eq!(
            specifiers("dart", "import 'package:flutter/material.dart';\n"),
            vec!["package:flutter/material.dart"]
        );
        assert_eq!(
            specifiers("swift", "import Foundation\n@testable import App\n"),
            vec!["Foundation", "App"]
        );
        assert_eq!(
            specifiers(
                "html",
                "<script src=\"app.js\"></script>\n<link rel=\"stylesheet\" href=\"style.css\">\n"
            ),
            vec!["app.js", "style.css"]
        );
        assert_eq!(
            specifiers("css", "@import url(\"base.css\");\n@import 'theme.css';\n"),
            vec!["base.css", "theme.css"]
        );
        assert_eq!(
            specifiers(
                "php",
                "use App\\Models\\User;\nrequire_once 'config.php';\n"
            ),
            vec!["App\\Models\\User", "config.php"]
        );
    }

    #[test]
    fn extracts_package_declarations() {
        let spec = LanguageRegistry::builtin().get("java").unwrap();
        let scanned = scan("// header\npackage com.example.app;\n", &spec.syntax);
        assert_eq!(
            extract_package(spec, &scanned.lines).as_deref(),
            Some("com.example.app")
        );
        let rust = LanguageRegistry::builtin().get("rust").unwrap();
        assert_eq!(
            extract_package(rust, &scan("fn main() {}", &rust.syntax).lines),
            None
        );
    }

    #[test]
    fn typescript_type_only_imports_are_not_runtime_dependencies() {
        let source = "import type { Backend } from './backend';\nimport type {\n  Session,\n  Job,\n} from './types';\nimport { type A, type B } from './letters';\nimport { type C, D } from './mixed';\nimport E, { type F } from './default';\nexport type { G } from './reexported';\nexport * from './everything';\nimport type from './named-type';\nimport types from './types-value';\nimport './styles.css';\nconst lazy = await import('./lazy');\n";
        let found = specifiers("typescript", source);
        assert_eq!(
            found,
            vec![
                "./mixed",
                "./default",
                "./everything",
                "./named-type",
                "./types-value",
                "./styles.css",
                "./lazy",
            ]
        );
    }
}
