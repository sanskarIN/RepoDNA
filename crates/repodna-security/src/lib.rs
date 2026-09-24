//! Defensive security signals for RepoDNA.
//!
//! The scanner looks for credentials committed to the repository, risky constructs in code,
//! configuration, CI workflows, and container definitions, and unusual file permissions. It
//! is a static pattern matcher: a clean result does not mean the code is secure, and a
//! match does not prove a vulnerability.
//!
//! Secret values never leave this crate. A candidate records its rule, location, and a
//! fingerprint computed with HMAC-SHA256 under a key derived from the scanned content, so
//! identical values can be recognized within one scan without being revealed or guessed
//! from a shared report.

pub mod secrets;

pub use secrets::{SECRET_RULES, SecretRule, SecretScanner};

use repodna_core::paths;

/// Directory and file-name fragments that mark tests, fixtures, examples, and documentation,
/// where sample credentials are common.
const SAMPLE_MARKERS: &[&str] = &[
    "__tests__",
    "demo",
    "doc",
    "docs",
    "example",
    "examples",
    "fixture",
    "fixtures",
    "mock",
    "mocks",
    "sample",
    "samples",
    "spec",
    "test",
    "testdata",
    "tests",
];

/// Returns `true` when `path` looks like tests, fixtures, examples, or documentation.
pub fn is_sample_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let name = paths::file_name(&lower);
    lower.split('/').any(|part| SAMPLE_MARKERS.contains(&part))
        || name.starts_with("test_")
        || name.contains(".test.")
        || name.contains(".spec.")
        || name.contains("_test.")
        || name.contains(".example")
        || name.contains(".sample")
        || name.ends_with(".md")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_sample_paths() {
        for path in [
            "tests/fixtures/key.pem",
            "src/auth/token.test.ts",
            "docs/setup.md",
            "config/.env.example",
            "pkg/auth_test.go",
        ] {
            assert!(is_sample_path(path), "{path}");
        }
        for path in ["src/config.py", "deploy/prod.env", "latest/notes.txt"] {
            assert!(!is_sample_path(path), "{path}");
        }
    }
}
