//! Project conventions for RepoDNA: tests, build systems, commands, CI, environment
//! requirements, and documentation.
//!
//! Everything here is read from repository metadata. Commands are *detected*, never run,
//! unless the user explicitly enables execution; only a supervised successful run marks a
//! command as verified.

pub mod ci;
pub mod commands;
pub mod docs;
pub mod environment;
pub mod testing;
pub mod tools;

use repodna_core::model::structure::FileCategory;
use repodna_core::paths;

/// A repository file as seen by project analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectFile<'a> {
    /// Repository-relative path.
    pub path: &'a str,
    /// Primary classification.
    pub category: FileCategory,
    /// Language identifier, when detected.
    pub language: Option<&'a str>,
    /// Code lines.
    pub code_lines: u64,
    /// All lines.
    pub total_lines: u64,
    /// The file contains inline tests, such as a Rust `#[cfg(test)]` module.
    pub inline_tests: bool,
}

/// File names whose content project analysis reads.
const CONTENT_FILES: &[&str] = &[
    ".gitlab-ci.yml",
    ".java-version",
    ".node-version",
    ".nvmrc",
    ".python-version",
    ".ruby-version",
    ".go-version",
    ".terraform-version",
    ".tool-versions",
    ".travis.yml",
    ".drone.yml",
    "appveyor.yml",
    "azure-pipelines.yml",
    "bitbucket-pipelines.yml",
    "build.gradle",
    "build.gradle.kts",
    "Cargo.toml",
    "CMakeLists.txt",
    "compose.yaml",
    "compose.yml",
    "composer.json",
    "deno.json",
    "deno.jsonc",
    "Gemfile",
    "global.json",
    "GNUmakefile",
    "go.mod",
    "Jenkinsfile",
    "justfile",
    "Justfile",
    "Makefile",
    "makefile",
    "meson.build",
    "package.json",
    "Package.swift",
    "pom.xml",
    "Procfile",
    "pubspec.yaml",
    "pyproject.toml",
    "pytest.ini",
    "rust-toolchain",
    "rust-toolchain.toml",
    "runtime.txt",
    "setup.cfg",
    "Taskfile.yaml",
    "Taskfile.yml",
    "tox.ini",
];

/// Returns `true` for files whose content project analysis needs: manifests, build and
/// CI configuration, tool-version files, container definitions, READMEs, and licenses.
pub fn wants_content(path: &str) -> bool {
    let name = paths::file_name(path);
    let lower = name.to_ascii_lowercase();
    CONTENT_FILES.contains(&name)
        || lower.starts_with("dockerfile")
        || lower.starts_with("docker-compose")
        || lower.ends_with(".csproj")
        || lower.ends_with(".fsproj")
        || lower.ends_with(".vbproj")
        || (paths::depth(path) <= 2 && is_readme(path))
        || (paths::depth(path) <= 2 && is_license(path))
        || is_ci_file(path)
}

/// Returns `true` for README files.
pub fn is_readme(path: &str) -> bool {
    let lower = paths::file_name(path).to_ascii_lowercase();
    lower == "readme" || lower.starts_with("readme.")
}

/// Returns `true` for license files.
pub fn is_license(path: &str) -> bool {
    let lower = paths::file_name(path).to_ascii_lowercase();
    ["license", "licence", "copying", "unlicense"]
        .iter()
        .any(|name| {
            lower == *name
                || lower.starts_with(&format!("{name}."))
                || lower.starts_with(&format!("{name}-"))
        })
}

/// Returns `true` for CI/CD configuration files.
pub fn is_ci_file(path: &str) -> bool {
    let yaml = matches!(paths::extension(path).as_deref(), Some("yml" | "yaml"));
    (yaml && (path.starts_with(".github/workflows/") || path.starts_with(".buildkite/")))
        || path == ".circleci/config.yml"
        || path == "cloudbuild.yaml"
        || path == "cloudbuild.yml"
        || matches!(
            path,
            ".gitlab-ci.yml"
                | ".travis.yml"
                | ".drone.yml"
                | "appveyor.yml"
                | "azure-pipelines.yml"
                | "bitbucket-pipelines.yml"
                | "Jenkinsfile"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_metadata_files() {
        for path in [
            "package.json",
            "web/package.json",
            "Cargo.toml",
            ".github/workflows/ci.yml",
            "README.md",
            "docs/README.md",
            "LICENSE",
            "LICENSE-APACHE",
            "Dockerfile.dev",
            "src/App/App.csproj",
            ".nvmrc",
        ] {
            assert!(wants_content(path), "{path}");
        }
        for path in [
            "src/main.rs",
            "a/b/c/README.md",
            "licenses.json",
            "notes.txt",
        ] {
            assert!(!wants_content(path), "{path}");
        }
        assert!(is_ci_file(".circleci/config.yml"));
        assert!(!is_ci_file("config.yml"));
    }
}
