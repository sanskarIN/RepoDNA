//! Cross-language interactions: where code in one language uses code in another.
//!
//! Two kinds of evidence are used. Resolved edges between files of different languages
//! (an HTML page loading a script, a Rust file embedding an HTML asset) are direct
//! observations. Bridge frameworks (Tauri, wasm-bindgen, PyO3, pybind11, Node addons, cgo,
//! JNI, ctypes) are recognized from the imports that set them up.

use std::collections::{BTreeMap, BTreeSet};

use repodna_core::confidence::Confidence;
use repodna_core::evidence::Evidence;
use repodna_core::model::languages::LanguageInteraction;
use repodna_core::paths;

use crate::SourceFile;

/// Maximum evidence items per interaction.
const MAX_EVIDENCE: usize = 5;

/// A resolved edge between two files, as seen by interaction detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrossEdge {
    /// Dependent file index.
    pub from: usize,
    /// Dependency file index.
    pub to: usize,
    /// Line of the declaration in `from`.
    pub line: u32,
    /// Mechanism label, e.g. `import` or `file-reference`.
    pub mechanism: &'static str,
    /// Confidence of the resolution.
    pub confidence: Confidence,
}

/// Which language is on one side of a bridge.
#[derive(Debug, Clone, Copy)]
enum Side {
    /// The language of the file containing the marker import.
    Host,
    /// A fixed language.
    Language(&'static str),
    /// The first of these languages present in the repository (or the first one if none).
    FirstPresent(&'static [&'static str]),
}

/// A framework that connects two languages, recognized by an import.
struct Bridge {
    mechanism: &'static str,
    description: &'static str,
    hosts: &'static [&'static str],
    specifiers: &'static [&'static str],
    from: Side,
    to: Side,
    confidence: Confidence,
}

const JS: &[&str] = &["typescript", "javascript"];
const NATIVE: &[&str] = &["c", "cpp", "rust"];
const JVM: &[&str] = &["java", "kotlin"];

const BRIDGES: &[Bridge] = &[
    Bridge {
        mechanism: "tauri-ipc",
        description: "Frontend code calls Rust commands through Tauri's IPC bridge.",
        hosts: JS,
        specifiers: &["@tauri-apps/api"],
        from: Side::Host,
        to: Side::Language("rust"),
        confidence: Confidence::Medium,
    },
    Bridge {
        mechanism: "wasm-bindgen",
        description: "Rust code is compiled to WebAssembly and called from JavaScript through wasm-bindgen.",
        hosts: &["rust"],
        specifiers: &["wasm_bindgen"],
        from: Side::FirstPresent(JS),
        to: Side::Host,
        confidence: Confidence::Medium,
    },
    Bridge {
        mechanism: "pyo3",
        description: "Rust code is exposed to Python as an extension module through PyO3.",
        hosts: &["rust"],
        specifiers: &["pyo3"],
        from: Side::Language("python"),
        to: Side::Host,
        confidence: Confidence::Medium,
    },
    Bridge {
        mechanism: "pybind11",
        description: "C++ code is exposed to Python through pybind11.",
        hosts: &["cpp", "c"],
        specifiers: &["pybind11"],
        from: Side::Language("python"),
        to: Side::Host,
        confidence: Confidence::Medium,
    },
    Bridge {
        mechanism: "node-addon",
        description: "Native code is loaded by Node.js as an addon (N-API).",
        hosts: &["rust"],
        specifiers: &["napi", "napi_derive", "neon"],
        from: Side::FirstPresent(JS),
        to: Side::Host,
        confidence: Confidence::Medium,
    },
    Bridge {
        mechanism: "node-addon",
        description: "Native code is loaded by Node.js as an addon (N-API).",
        hosts: &["cpp", "c"],
        specifiers: &["napi.h", "node_api.h", "node.h"],
        from: Side::FirstPresent(JS),
        to: Side::Host,
        confidence: Confidence::Medium,
    },
    Bridge {
        mechanism: "cgo",
        description: "Go code calls C through cgo.",
        hosts: &["go"],
        specifiers: &["C"],
        from: Side::Host,
        to: Side::Language("c"),
        confidence: Confidence::High,
    },
    Bridge {
        mechanism: "jni",
        description: "Native code implements JVM methods through the Java Native Interface.",
        hosts: &["c", "cpp"],
        specifiers: &["jni.h"],
        from: Side::FirstPresent(JVM),
        to: Side::Host,
        confidence: Confidence::Medium,
    },
    Bridge {
        mechanism: "jni",
        description: "Native code implements JVM methods through the Java Native Interface.",
        hosts: &["rust"],
        specifiers: &["jni"],
        from: Side::FirstPresent(JVM),
        to: Side::Host,
        confidence: Confidence::Medium,
    },
    Bridge {
        mechanism: "ffi",
        description: "Python code loads native libraries through ctypes or cffi.",
        hosts: &["python"],
        specifiers: &["ctypes", "cffi"],
        from: Side::Host,
        to: Side::FirstPresent(NATIVE),
        confidence: Confidence::Low,
    },
];

/// `true` when `specifier` is `marker` or starts with it followed by a separator.
fn matches_marker(specifier: &str, marker: &str) -> bool {
    specifier == marker
        || specifier
            .strip_prefix(marker)
            .is_some_and(|rest| rest.starts_with(['/', ':', '.']))
}

fn side_language<'a>(side: Side, host: &'a str, present: &BTreeSet<&str>) -> &'a str {
    match side {
        Side::Host => host,
        Side::Language(fixed) => fixed,
        Side::FirstPresent(options) => options
            .iter()
            .copied()
            .find(|option| present.contains(option))
            .or_else(|| options.first().copied())
            .unwrap_or("unknown"),
    }
}

#[derive(Default)]
struct Accumulator {
    description: String,
    count: u32,
    confidence: Confidence,
    evidence: Vec<Evidence>,
    files: BTreeSet<usize>,
}

/// Detects interactions from cross-language `edges` and bridge imports in `files`.
pub fn detect_interactions(
    files: &[SourceFile<'_>],
    edges: &[CrossEdge],
) -> Vec<LanguageInteraction> {
    let present: BTreeSet<&str> = files
        .iter()
        .filter(|file| file.first_party)
        .filter_map(|file| file.language)
        .collect();
    let mut found: BTreeMap<(String, String, &'static str), Accumulator> = BTreeMap::new();

    for edge in edges {
        let (Some(from), Some(to)) = (files[edge.from].language, files[edge.to].language) else {
            continue;
        };
        if from == to {
            continue;
        }
        let entry = found
            .entry((from.to_owned(), to.to_owned(), edge.mechanism))
            .or_default();
        if entry.description.is_empty() {
            entry.description = match edge.mechanism {
                "file-reference" => format!("{from} code refers to {to} files by path."),
                "markup-reference" => format!("{from} files load {to} files."),
                "include" => format!("{from} files include {to} files."),
                _ => format!("{from} code imports {to} files."),
            };
        }
        entry.count += 1;
        entry.confidence = entry.confidence.max(edge.confidence);
        if entry.evidence.len() < MAX_EVIDENCE {
            entry.evidence.push(Evidence::edge_at(
                files[edge.from].path,
                files[edge.to].path,
                files[edge.from].path,
                Some(edge.line),
            ));
        }
    }

    for (index, file) in files.iter().enumerate() {
        let (Some(language), Some(analysis)) = (file.language, file.analysis) else {
            continue;
        };
        if !file.first_party {
            continue;
        }
        for bridge in BRIDGES
            .iter()
            .filter(|bridge| bridge.hosts.contains(&language))
        {
            let Some(import) = analysis.imports.iter().find(|import| {
                bridge
                    .specifiers
                    .iter()
                    .any(|marker| matches_marker(&import.specifier, marker))
            }) else {
                continue;
            };
            let from = side_language(bridge.from, language, &present);
            let to = side_language(bridge.to, language, &present);
            if from == to {
                continue;
            }
            let entry = found
                .entry((from.to_owned(), to.to_owned(), bridge.mechanism))
                .or_default();
            if entry.files.insert(index) {
                entry.description = bridge.description.to_owned();
                entry.count += 1;
                entry.confidence = entry.confidence.max(bridge.confidence);
                if entry.evidence.len() < MAX_EVIDENCE {
                    entry.evidence.push(
                        Evidence::line(file.path, import.line)
                            .with_note(format!("imports {}", import.specifier)),
                    );
                }
            }
        }
        if paths::file_name(file.path).ends_with("-Bridging-Header.h") {
            let entry = found
                .entry((
                    "swift".to_owned(),
                    "objective-c".to_owned(),
                    "bridging-header",
                ))
                .or_default();
            entry.description =
                "Swift code calls Objective-C through a bridging header.".to_owned();
            entry.count += 1;
            entry.confidence = Confidence::High;
            if entry.evidence.len() < MAX_EVIDENCE {
                entry.evidence.push(Evidence::file(file.path));
            }
        }
    }

    let mut interactions: Vec<LanguageInteraction> = found
        .into_iter()
        .map(|((from, to, mechanism), entry)| LanguageInteraction {
            from,
            to,
            mechanism: mechanism.to_owned(),
            description: entry.description,
            count: entry.count,
            confidence: entry.confidence,
            evidence: entry.evidence,
        })
        .collect();
    interactions.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.from.cmp(&b.from))
            .then_with(|| a.to.cmp(&b.to))
            .then_with(|| a.mechanism.cmp(&b.mechanism))
    });
    interactions
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_parser::{FileAnalysis, LanguageRegistry, analyze_source};

    #[test]
    fn detects_edges_and_bridges() {
        let registry = LanguageRegistry::builtin();
        let inputs: [(&str, &str); 6] = [
            (
                "ui/src/api.ts",
                "import { invoke } from '@tauri-apps/api/core';\n",
            ),
            ("ui/index.html", "<script src=\"./src/api.ts\"></script>\n"),
            ("src-tauri/src/main.rs", "use tauri::Manager;\n"),
            ("native/lib.rs", "use pyo3::prelude::*;\n"),
            ("go/cgo.go", "package x\n\nimport \"C\"\n"),
            ("ios/App-Bridging-Header.h", "#import \"Legacy.h\"\n"),
        ];
        let analyses: Vec<Option<FileAnalysis>> = inputs
            .iter()
            .map(|(path, text)| {
                registry
                    .detect_path(path)
                    .map(|spec| analyze_source(spec, text))
            })
            .collect();
        let files: Vec<SourceFile<'_>> = inputs
            .iter()
            .zip(&analyses)
            .map(|((path, _), analysis)| SourceFile {
                path,
                language: analysis.as_ref().map(|a| a.language.as_str()),
                code_lines: 1,
                first_party: true,
                test: false,
                analysis: analysis.as_ref(),
            })
            .collect();
        let edges = [CrossEdge {
            from: 1,
            to: 0,
            line: 1,
            mechanism: "markup-reference",
            confidence: Confidence::High,
        }];
        let found = detect_interactions(&files, &edges);
        let summary: Vec<(&str, &str, &str, u32)> = found
            .iter()
            .map(|i| {
                (
                    i.from.as_str(),
                    i.to.as_str(),
                    i.mechanism.as_str(),
                    i.count,
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                ("go", "c", "cgo", 1),
                ("html", "typescript", "markup-reference", 1),
                ("python", "rust", "pyo3", 1),
                ("swift", "objective-c", "bridging-header", 1),
                ("typescript", "rust", "tauri-ipc", 1),
            ]
        );
        let tauri = found.iter().find(|i| i.mechanism == "tauri-ipc").unwrap();
        assert!(matches!(
            tauri.evidence[0],
            Evidence::File { line: Some(1), .. }
        ));
    }

    #[test]
    fn markers_match_whole_segments() {
        assert!(matches_marker("@tauri-apps/api/core", "@tauri-apps/api"));
        assert!(matches_marker("wasm_bindgen::prelude", "wasm_bindgen"));
        assert!(matches_marker("pybind11/pybind11.h", "pybind11"));
        assert!(!matches_marker("pyo3_macros_backend", "pyo3"));
        assert!(!matches_marker("Cats", "C"));
    }
}
