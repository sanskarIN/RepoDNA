//! Entrypoint detection: where execution or consumption of the code starts.
//!
//! Entrypoints declared in manifests (`[[bin]]` targets, `bin` and `main` in
//! `package.json`) have high confidence. Conventions (`src/main.rs`, `package main` with
//! `func main`, `__main__.py`, a `main` method, `index.html`) have medium confidence, and
//! weaker conventions (an `index.ts` that may be a library barrel) have low confidence.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

use repodna_core::confidence::Confidence;
use repodna_core::evidence::Evidence;
use repodna_core::model::structure::{Entrypoint, EntrypointKind};
use repodna_core::paths;
use repodna_dependencies::DeclaredEntrypoint;
use repodna_parser::ParsedSymbol;

use crate::SourceFile;

/// Maximum number of entrypoints reported.
pub const MAX_ENTRYPOINTS: usize = 200;

fn kind_rank(kind: EntrypointKind) -> u8 {
    match kind {
        EntrypointKind::Binary => 0,
        EntrypointKind::Server => 1,
        EntrypointKind::Web => 2,
        EntrypointKind::Cli => 3,
        EntrypointKind::Script => 4,
        EntrypointKind::Library => 5,
    }
}

/// A conventional entrypoint match.
struct Convention {
    kind: EntrypointKind,
    confidence: Confidence,
    reason: String,
    evidence: Evidence,
}

impl Convention {
    fn new(
        kind: EntrypointKind,
        confidence: Confidence,
        reason: impl Into<String>,
        evidence: Evidence,
    ) -> Option<Self> {
        Some(Self {
            kind,
            confidence,
            reason: reason.into(),
            evidence,
        })
    }
}

fn main_function<'a>(file: &SourceFile<'a>, names: &[&str]) -> Option<&'a ParsedSymbol> {
    file.analysis?
        .symbols
        .iter()
        .find(|symbol| symbol.is_function() && names.contains(&symbol.name.as_str()))
}

fn symbol_evidence(path: &str, symbol: &ParsedSymbol) -> Evidence {
    Evidence::symbol(path, &symbol.name, Some(symbol.line))
}

fn conventional(file: &SourceFile<'_>) -> Option<Convention> {
    let path = file.path;
    let name = paths::file_name(path);
    let stem = paths::file_stem(path);
    let parent_name = paths::file_name(paths::parent(path));
    let main_guard = file.analysis.is_some_and(|analysis| analysis.main_guard);
    let whole_file = || Evidence::file(path);
    match file.language.unwrap_or_default() {
        "rust" => {
            if name == "main.rs" {
                Convention::new(
                    EntrypointKind::Binary,
                    Confidence::High,
                    "Cargo builds main.rs as a binary crate root",
                    whole_file(),
                )
            } else if path.starts_with("src/bin/") || path.contains("/src/bin/") {
                Convention::new(
                    EntrypointKind::Binary,
                    Confidence::High,
                    "Cargo builds files in src/bin as binaries",
                    whole_file(),
                )
            } else if name == "lib.rs" && parent_name == "src" {
                Convention::new(
                    EntrypointKind::Library,
                    Confidence::High,
                    "src/lib.rs is the library crate root",
                    whole_file(),
                )
            } else {
                None
            }
        }
        "go" => {
            let package_main = file
                .analysis
                .is_some_and(|analysis| analysis.package.as_deref() == Some("main"));
            let main = main_function(file, &["main"])?;
            package_main.then(|| Convention {
                kind: EntrypointKind::Binary,
                confidence: Confidence::High,
                reason: "Declares package main with func main".to_owned(),
                evidence: symbol_evidence(path, main),
            })
        }
        "python" => {
            if name == "__main__.py" {
                Convention::new(
                    EntrypointKind::Binary,
                    Confidence::High,
                    "__main__.py runs when the package is executed with python -m",
                    whole_file(),
                )
            } else if matches!(name, "wsgi.py" | "asgi.py") {
                Convention::new(
                    EntrypointKind::Server,
                    Confidence::Medium,
                    "Exposes a WSGI or ASGI application",
                    whole_file(),
                )
            } else if main_guard {
                Convention::new(
                    EntrypointKind::Script,
                    Confidence::Medium,
                    "Runs as a program through an `if __name__ == \"__main__\"` guard",
                    whole_file(),
                )
            } else {
                None
            }
        }
        "javascript" | "typescript" => {
            let extension = paths::extension(path).unwrap_or_default();
            if main_guard {
                Convention::new(
                    EntrypointKind::Script,
                    Confidence::Medium,
                    "Runs as a program when executed directly",
                    whole_file(),
                )
            } else if matches!(stem, "main" | "index")
                && matches!(extension.as_str(), "tsx" | "jsx")
            {
                Convention::new(
                    EntrypointKind::Web,
                    Confidence::Medium,
                    format!("Conventional application entry module ({name})"),
                    whole_file(),
                )
            } else if matches!(stem, "main" | "index") && parent_name == "src" {
                Convention::new(
                    EntrypointKind::Library,
                    Confidence::Low,
                    format!("Conventional package entry module ({name})"),
                    whole_file(),
                )
            } else if stem == "server" {
                Convention::new(
                    EntrypointKind::Server,
                    Confidence::Low,
                    "Named like a server entry module",
                    whole_file(),
                )
            } else {
                None
            }
        }
        "html" if name == "index.html" => Convention::new(
            EntrypointKind::Web,
            Confidence::Medium,
            "HTML page that loads the web application",
            whole_file(),
        ),
        "java" | "kotlin" | "scala" | "groovy" => {
            let main = main_function(file, &["main"])?;
            Convention::new(
                EntrypointKind::Binary,
                Confidence::Medium,
                "Defines a main method",
                symbol_evidence(path, main),
            )
        }
        "csharp" => {
            if let Some(main) = main_function(file, &["Main"]) {
                Convention::new(
                    EntrypointKind::Binary,
                    Confidence::Medium,
                    "Defines a Main method",
                    symbol_evidence(path, main),
                )
            } else if name == "Program.cs" {
                Convention::new(
                    EntrypointKind::Binary,
                    Confidence::Medium,
                    "Program.cs holds the program entry (possibly with top-level statements)",
                    whole_file(),
                )
            } else {
                None
            }
        }
        "c" | "cpp" | "objective-c" => {
            let main = main_function(file, &["main", "wmain", "WinMain"])?;
            Convention::new(
                EntrypointKind::Binary,
                Confidence::Medium,
                "Defines the main function",
                symbol_evidence(path, main),
            )
        }
        "swift" if name == "main.swift" => Convention::new(
            EntrypointKind::Binary,
            Confidence::High,
            "main.swift holds top-level program code",
            whole_file(),
        ),
        "dart" => {
            let main = main_function(file, &["main"])?;
            (parent_name == "bin" || path == "lib/main.dart" || path.ends_with("/lib/main.dart"))
                .then(|| Convention {
                    kind: EntrypointKind::Binary,
                    confidence: Confidence::Medium,
                    reason: "Defines main in a conventional entry file".to_owned(),
                    evidence: symbol_evidence(path, main),
                })
        }
        "php" if name == "index.php" => Convention::new(
            EntrypointKind::Web,
            Confidence::Medium,
            "index.php is the web server's front controller",
            whole_file(),
        ),
        "ruby" => {
            if name == "config.ru" {
                Convention::new(
                    EntrypointKind::Server,
                    Confidence::Medium,
                    "config.ru starts a Rack application",
                    whole_file(),
                )
            } else if main_guard {
                Convention::new(
                    EntrypointKind::Script,
                    Confidence::Medium,
                    "Runs as a program through an `if __FILE__ == $0` guard",
                    whole_file(),
                )
            } else if parent_name == "bin" {
                Convention::new(
                    EntrypointKind::Script,
                    Confidence::Low,
                    "Executable in a bin directory",
                    whole_file(),
                )
            } else {
                None
            }
        }
        "shell" if parent_name == "bin" => Convention::new(
            EntrypointKind::Script,
            Confidence::Low,
            "Executable in a bin directory",
            whole_file(),
        ),
        _ => None,
    }
}

/// Detects entrypoints among `files` and those declared in manifests.
///
/// Declared entrypoints that do not exist in the repository (for example build outputs such
/// as `dist/cli.js`) are skipped. Test files never count as entrypoints.
pub fn detect_entrypoints(
    files: &[SourceFile<'_>],
    declared: &[DeclaredEntrypoint],
) -> Vec<Entrypoint> {
    let by_path: HashMap<&str, &SourceFile<'_>> =
        files.iter().map(|file| (file.path, file)).collect();
    let mut found: BTreeMap<String, Entrypoint> = BTreeMap::new();
    for entry in declared {
        let Some(file) = by_path.get(entry.path.as_str()) else {
            continue;
        };
        found
            .entry(entry.path.clone())
            .or_insert_with(|| Entrypoint {
                path: entry.path.clone(),
                kind: entry.kind,
                language: file.language.map(str::to_owned),
                reason: format!("Declared in {}", entry.manifest),
                confidence: Confidence::High,
                evidence: vec![
                    Evidence::file(&entry.manifest).with_note(format!("declares {}", entry.path)),
                    Evidence::file(&entry.path),
                ],
            });
    }
    for file in files.iter().filter(|file| file.first_party && !file.test) {
        if found.contains_key(file.path) {
            continue;
        }
        if let Some(convention) = conventional(file) {
            found.insert(
                file.path.to_owned(),
                Entrypoint {
                    path: file.path.to_owned(),
                    kind: convention.kind,
                    language: file.language.map(str::to_owned),
                    reason: convention.reason,
                    confidence: convention.confidence,
                    evidence: vec![convention.evidence],
                },
            );
        }
    }
    let mut entrypoints: Vec<Entrypoint> = found.into_values().collect();
    entrypoints.sort_by(|a, b| {
        b.confidence
            .cmp(&a.confidence)
            .then_with(|| kind_rank(a.kind).cmp(&kind_rank(b.kind)))
            .then_with(|| a.path.cmp(&b.path))
    });
    entrypoints.truncate(MAX_ENTRYPOINTS);
    entrypoints
}

/// Orders entrypoints the way they are displayed: strongest evidence first.
pub fn display_order(a: &Entrypoint, b: &Entrypoint) -> Ordering {
    b.confidence
        .cmp(&a.confidence)
        .then_with(|| kind_rank(a.kind).cmp(&kind_rank(b.kind)))
        .then_with(|| a.path.cmp(&b.path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_parser::{FileAnalysis, LanguageRegistry, analyze_source};

    fn analyze(files: &[(&'static str, &str)]) -> Vec<(&'static str, Option<FileAnalysis>)> {
        let registry = LanguageRegistry::builtin();
        files
            .iter()
            .map(|(path, text)| {
                (
                    *path,
                    registry
                        .detect_path(path)
                        .map(|spec| analyze_source(spec, text)),
                )
            })
            .collect()
    }

    fn sources<'a>(analyzed: &'a [(&'static str, Option<FileAnalysis>)]) -> Vec<SourceFile<'a>> {
        analyzed
            .iter()
            .map(|(path, analysis)| SourceFile {
                path,
                language: analysis.as_ref().map(|a| a.language.as_str()),
                code_lines: 1,
                first_party: true,
                test: path.starts_with("tests/"),
                analysis: analysis.as_ref(),
            })
            .collect()
    }

    #[test]
    fn detects_conventional_and_declared_entrypoints() {
        let analyzed = analyze(&[
            ("src/main.rs", "fn main() {}\n"),
            ("src/lib.rs", "pub fn f() {}\n"),
            ("src/bin/tool.rs", "fn main() {}\n"),
            ("cmd/api/main.go", "package main\n\nfunc main() {\n}\n"),
            ("internal/x/x.go", "package x\n\nfunc main() {\n}\n"),
            ("pkg/__main__.py", ""),
            (
                "scripts/gen.py",
                "def run():\n    pass\n\nif __name__ == \"__main__\":\n    run()\n",
            ),
            (
                "tests/test_gen.py",
                "if __name__ == \"__main__\":\n    pass\n",
            ),
            ("web/src/main.tsx", "import App from './App';\n"),
            ("web/index.html", "<html></html>\n"),
            (
                "App.java",
                "class App {\n  public static void main(String[] args) {\n  }\n}\n",
            ),
            ("cli/index.js", "#!/usr/bin/env node\n"),
            ("util/helpers.ts", "export const x = 1;\n"),
        ]);
        let files = sources(&analyzed);
        let declared = [
            DeclaredEntrypoint {
                manifest: "cli/package.json".into(),
                path: "cli/index.js".into(),
                kind: EntrypointKind::Cli,
            },
            DeclaredEntrypoint {
                manifest: "package.json".into(),
                path: "dist/missing.js".into(),
                kind: EntrypointKind::Cli,
            },
        ];
        let found = detect_entrypoints(&files, &declared);
        let summary: Vec<(&str, EntrypointKind, Confidence)> = found
            .iter()
            .map(|entry| (entry.path.as_str(), entry.kind, entry.confidence))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("cmd/api/main.go", EntrypointKind::Binary, Confidence::High),
                ("pkg/__main__.py", EntrypointKind::Binary, Confidence::High),
                ("src/bin/tool.rs", EntrypointKind::Binary, Confidence::High),
                ("src/main.rs", EntrypointKind::Binary, Confidence::High),
                ("cli/index.js", EntrypointKind::Cli, Confidence::High),
                ("src/lib.rs", EntrypointKind::Library, Confidence::High),
                ("App.java", EntrypointKind::Binary, Confidence::Medium),
                ("web/index.html", EntrypointKind::Web, Confidence::Medium),
                ("web/src/main.tsx", EntrypointKind::Web, Confidence::Medium),
                ("scripts/gen.py", EntrypointKind::Script, Confidence::Medium),
            ]
        );
        let declared_entry = found.iter().find(|e| e.path == "cli/index.js").unwrap();
        assert_eq!(declared_entry.reason, "Declared in cli/package.json");
        assert_eq!(declared_entry.evidence.len(), 2);
        let go = found.iter().find(|e| e.path == "cmd/api/main.go").unwrap();
        assert!(matches!(
            go.evidence[0],
            Evidence::Symbol { line: Some(3), .. }
        ));
        let mut sorted = found.clone();
        sorted.sort_by(display_order);
        assert_eq!(sorted, found);
    }
}
