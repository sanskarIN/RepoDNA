//! Dead-code candidates: files and directories that nothing in the repository appears to use.
//!
//! These are candidates, never verdicts. Code can be used through configuration, runtime
//! loading, reflection, code generation, or from outside the repository, so every candidate
//! carries its confidence and the reason it was listed.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use repodna_core::confidence::Confidence;
use repodna_core::evidence::Evidence;
use repodna_core::model::quality::{DeadCodeCandidate, DeadCodeKind};
use repodna_core::paths;
use repodna_core::time::Timestamp;

use crate::QualityFile;

/// Maximum candidates listed.
pub const MAX_CANDIDATES: usize = 50;

/// Days without changes after which a legacy-named directory counts as stale.
pub const STALE_DAYS: i64 = 365;

/// Languages whose imports are resolved to files, so "nothing imports it" is meaningful.
const IMPORT_RESOLVED: &[&str] = &[
    "javascript",
    "lua",
    "php",
    "python",
    "ruby",
    "svelte",
    "typescript",
    "vue",
];

/// Directory names whose files are normally run or loaded directly rather than imported.
const DIRECT_USE_DIRS: &[&str] = &[
    "__tests__",
    "bench",
    "benches",
    "benchmarks",
    "bin",
    "demo",
    "demos",
    "docs",
    "e2e",
    "example",
    "examples",
    "fixtures",
    "migrations",
    "public",
    "samples",
    "scripts",
    "static",
    "stories",
    "test",
    "tests",
    "tools",
];

/// File stems that are entry points or loaded by tools by convention.
const CONVENTIONAL_STEMS: &[&str] = &[
    "__init__",
    "__main__",
    "app",
    "asgi",
    "cli",
    "config",
    "conftest",
    "index",
    "main",
    "manage",
    "server",
    "settings",
    "setup",
    "setupTests",
    "vite-env.d",
    "wsgi",
];

/// Directory names that suggest code kept for reference rather than use.
const LEGACY_NAMES: &[&str] = &[
    "archive",
    "archived",
    "attic",
    "backup",
    "deprecated",
    "legacy",
    "obsolete",
    "old",
    "unused",
];

/// What quality analysis knows about how files are used.
#[derive(Debug, Clone, Copy)]
pub struct UsageContext<'a> {
    /// Per file: number of first-party files that import or reference it.
    pub dependents: &'a [u32],
    /// Per file: `true` when a module declaration (Rust `mod`) includes it.
    pub module_declared: &'a [bool],
    /// Paths of detected entrypoints.
    pub entrypoints: &'a BTreeSet<String>,
    /// `false` when too many imports were unresolved for "nothing imports it" to be
    /// meaningful.
    pub imports_reliable: bool,
    /// Per file: time of the last change, when history is available.
    pub last_changed: &'a [Option<Timestamp>],
    /// Time that staleness is measured against (normally the latest commit).
    pub reference_time: Option<Timestamp>,
}

fn is_rust_crate_root(path: &str) -> bool {
    matches!(paths::file_name(path), "lib.rs" | "main.rs" | "build.rs")
        || paths::ancestors(path).iter().any(|dir| {
            matches!(
                paths::file_name(dir),
                "bin" | "benches" | "examples" | "tests"
            )
        })
}

fn is_conventional(path: &str) -> bool {
    let name = paths::file_name(path);
    let stem = paths::file_stem(path);
    name.starts_with('.')
        || CONVENTIONAL_STEMS.contains(&stem)
        || name.contains(".config.")
        || name.ends_with(".d.ts")
        || [".stories.", ".test.", ".spec."]
            .iter()
            .any(|part| name.contains(part))
}

fn in_direct_use_dir(path: &str) -> bool {
    paths::ancestors(path)
        .iter()
        .any(|dir| DIRECT_USE_DIRS.contains(&paths::file_name(dir)))
}

/// Lists dead-code candidates for `files`, whose usage is described by `context`.
pub fn dead_code_candidates(
    files: &[QualityFile<'_>],
    context: &UsageContext<'_>,
) -> Vec<DeadCodeCandidate> {
    let mut candidates = Vec::new();
    let rust_uses_modules = files.iter().enumerate().any(|(index, file)| {
        file.language == Some("rust") && context.module_declared.get(index) == Some(&true)
    });
    for (index, file) in files.iter().enumerate().filter(|(_, file)| !file.test) {
        let Some(language) = file.language else {
            continue;
        };
        if context.entrypoints.contains(file.path) {
            continue;
        }
        if language == "rust" {
            if rust_uses_modules
                && context.module_declared.get(index) == Some(&false)
                && !is_rust_crate_root(file.path)
            {
                candidates.push(DeadCodeCandidate {
                    path: file.path.to_owned(),
                    kind: DeadCodeKind::UnreferencedFile,
                    label: "Not included by any module".to_owned(),
                    reason: "No `mod` declaration includes this file, so Cargo does not compile it unless a #[path] attribute or include! macro refers to it.".to_owned(),
                    confidence: Confidence::Medium,
                    evidence: vec![
                        Evidence::file(file.path).with_note("no `mod` declaration found"),
                        Evidence::observation(
                            "Rust compiles only files reachable through `mod` declarations from a crate root.",
                        ),
                    ],
                });
            }
            continue;
        }
        if context.imports_reliable
            && IMPORT_RESOLVED.contains(&language)
            && context.dependents.get(index) == Some(&0)
            && !is_conventional(file.path)
            && !in_direct_use_dir(file.path)
        {
            candidates.push(DeadCodeCandidate {
                path: file.path.to_owned(),
                kind: DeadCodeKind::UnreferencedFile,
                label: "Not imported".to_owned(),
                reason:
                    "No resolved import or file reference in the repository points to this file."
                        .to_owned(),
                confidence: Confidence::Low,
                evidence: vec![Evidence::file(file.path).with_note("no incoming imports found")],
            });
        }
    }

    // Legacy-named directories that contain code.
    let mut legacy: BTreeMap<String, (u64, Option<Timestamp>)> = BTreeMap::new();
    for (index, file) in files.iter().enumerate() {
        let Some(dir) = paths::ancestors(file.path).into_iter().find(|dir| {
            LEGACY_NAMES.contains(&paths::file_name(dir).to_ascii_lowercase().as_str())
        }) else {
            continue;
        };
        let entry = legacy.entry(dir.to_owned()).or_insert((0, None));
        entry.0 += 1;
        let changed = context.last_changed.get(index).copied().flatten();
        entry.1 = match (entry.1, changed) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
    }
    for (dir, (count, last_change)) in legacy {
        let stale_days = match (last_change, context.reference_time) {
            (Some(changed), Some(now)) => Some(changed.days_until(now)),
            _ => None,
        };
        let stale = stale_days.is_some_and(|days| days >= STALE_DAYS);
        let plural = if count == 1 { "" } else { "s" };
        let mut reason = format!(
            "The directory name suggests code kept for reference; it holds {count} code file{plural}."
        );
        let mut evidence =
            vec![Evidence::directory(&dir).with_note(format!("{count} code file{plural}"))];
        if let (true, Some(days)) = (stale, stale_days) {
            reason.push_str(&format!(" Nothing in it changed in the last {days} days."));
            evidence.push(Evidence::metric_with_threshold(
                "git.directory.days_since_change",
                days as f64,
                STALE_DAYS as f64,
                "days",
            ));
        }
        candidates.push(DeadCodeCandidate {
            path: dir,
            kind: DeadCodeKind::LegacyDirectory,
            label: "Legacy directory".to_owned(),
            reason,
            confidence: if stale {
                Confidence::Medium
            } else {
                Confidence::Low
            },
            evidence,
        });
    }

    candidates.sort_by_key(|candidate| (Reverse(candidate.confidence), candidate.path.clone()));
    candidates.truncate(MAX_CANDIDATES);
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::structure::LineCounts;

    fn file(path: &str) -> QualityFile<'_> {
        let language = match paths::extension(path).as_deref() {
            Some("rs") => "rust",
            Some("py") => "python",
            _ => "typescript",
        };
        QualityFile {
            path,
            language: Some(language),
            test: path.starts_with("tests/"),
            lines: LineCounts::default(),
            analysis: None,
            tokens: None,
        }
    }

    #[test]
    fn lists_rust_orphans_unimported_files_and_legacy_directories() {
        let paths = [
            "src/lib.rs",
            "src/used.rs",
            "src/orphan.rs",
            "src/bin/tool.rs",
            "web/src/util.ts",
            "web/src/index.ts",
            "web/src/used.ts",
            "scripts/gen.py",
            "tests/test_x.py",
            "old/legacy_api.py",
        ];
        let files: Vec<QualityFile<'_>> = paths.iter().map(|path| file(path)).collect();
        let dependents = [0, 0, 0, 0, 0, 0, 3, 0, 0, 0];
        let declared = [
            false, true, false, false, false, false, false, false, false, false,
        ];
        let last_changed: Vec<Option<Timestamp>> = paths
            .iter()
            .map(|path| {
                if path.starts_with("old/") {
                    Timestamp::from_ymd(2020, 1, 1)
                } else {
                    Timestamp::from_ymd(2026, 1, 1)
                }
            })
            .collect();
        let entrypoints = BTreeSet::new();
        let context = UsageContext {
            dependents: &dependents,
            module_declared: &declared,
            entrypoints: &entrypoints,
            imports_reliable: true,
            last_changed: &last_changed,
            reference_time: Timestamp::from_ymd(2026, 6, 1),
        };
        let candidates = dead_code_candidates(&files, &context);
        let summary: Vec<(&str, DeadCodeKind, Confidence)> = candidates
            .iter()
            .map(|c| (c.path.as_str(), c.kind, c.confidence))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("old", DeadCodeKind::LegacyDirectory, Confidence::Medium),
                (
                    "src/orphan.rs",
                    DeadCodeKind::UnreferencedFile,
                    Confidence::Medium
                ),
                (
                    "old/legacy_api.py",
                    DeadCodeKind::UnreferencedFile,
                    Confidence::Low
                ),
                (
                    "web/src/util.ts",
                    DeadCodeKind::UnreferencedFile,
                    Confidence::Low
                ),
            ]
        );
        assert!(candidates[0].reason.contains("days"));

        let unreliable = UsageContext {
            imports_reliable: false,
            reference_time: None,
            ..context
        };
        let candidates = dead_code_candidates(&files, &unreliable);
        let paths: Vec<&str> = candidates.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(paths, vec!["src/orphan.rs", "old"]);
    }
}
