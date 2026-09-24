//! Language lookup by file name, extension, and shebang.

use std::collections::HashMap;
use std::sync::LazyLock;

use repodna_core::paths;

use crate::builtin::builtin_languages;
use crate::spec::LanguageSpec;

/// A set of language specifications with fast lookup tables.
#[derive(Debug, Clone)]
pub struct LanguageRegistry {
    languages: Vec<LanguageSpec>,
    by_id: HashMap<String, usize>,
    by_extension: HashMap<String, usize>,
    by_filename: HashMap<String, usize>,
    by_prefix: Vec<(String, usize)>,
    by_shebang: HashMap<String, usize>,
}

static BUILTIN: LazyLock<LanguageRegistry> =
    LazyLock::new(|| LanguageRegistry::new(builtin_languages()));

impl LanguageRegistry {
    /// Builds a registry. Later specifications override earlier ones that claim the same
    /// identifier, extension, file name, or interpreter.
    pub fn new(languages: Vec<LanguageSpec>) -> Self {
        let mut registry = Self {
            languages: Vec::new(),
            by_id: HashMap::new(),
            by_extension: HashMap::new(),
            by_filename: HashMap::new(),
            by_prefix: Vec::new(),
            by_shebang: HashMap::new(),
        };
        for language in languages {
            registry.add(language);
        }
        registry
    }

    /// The shared registry of built-in languages.
    pub fn builtin() -> &'static LanguageRegistry {
        &BUILTIN
    }

    /// Returns a copy of the built-in registry extended with additional specifications
    /// (for example declarative language plugins).
    pub fn builtin_with(extra: Vec<LanguageSpec>) -> Self {
        let mut registry = BUILTIN.clone();
        for language in extra {
            registry.add(language);
        }
        registry
    }

    /// Adds or replaces a language.
    pub fn add(&mut self, language: LanguageSpec) {
        let index = match self.by_id.get(&language.id) {
            Some(&existing) => {
                self.languages[existing] = language;
                existing
            }
            None => {
                self.languages.push(language);
                self.languages.len() - 1
            }
        };
        let language = &self.languages[index];
        self.by_id.insert(language.id.clone(), index);
        for extension in &language.extensions {
            self.by_extension
                .insert(extension.to_ascii_lowercase(), index);
        }
        for filename in &language.filenames {
            self.by_filename.insert(filename.clone(), index);
        }
        for prefix in &language.filename_prefixes {
            self.by_prefix.retain(|(existing, _)| existing != prefix);
            self.by_prefix.push((prefix.clone(), index));
        }
        for interpreter in &language.shebangs {
            self.by_shebang.insert(interpreter.clone(), index);
        }
    }

    /// Every language in the registry.
    pub fn languages(&self) -> &[LanguageSpec] {
        &self.languages
    }

    /// Looks up a language by identifier.
    pub fn get(&self, id: &str) -> Option<&LanguageSpec> {
        self.by_id.get(id).map(|&index| &self.languages[index])
    }

    /// Detects a language from a repository path: exact file name first, then file-name
    /// prefix, then extension.
    pub fn detect_path(&self, path: &str) -> Option<&LanguageSpec> {
        let name = paths::file_name(path);
        if let Some(&index) = self.by_filename.get(name) {
            return Some(&self.languages[index]);
        }
        if let Some((_, index)) = self
            .by_prefix
            .iter()
            .find(|(prefix, _)| name.starts_with(prefix.as_str()))
        {
            return Some(&self.languages[*index]);
        }
        let extension = paths::extension(path)?;
        self.by_extension
            .get(&extension)
            .map(|&index| &self.languages[index])
    }

    /// Detects a language from a `#!` line such as `#!/usr/bin/env python3`.
    pub fn detect_shebang(&self, first_line: &str) -> Option<&LanguageSpec> {
        let interpreter = shebang_interpreter(first_line)?;
        if let Some(&index) = self.by_shebang.get(interpreter) {
            return Some(&self.languages[index]);
        }
        // `python3.12` → `python3` → `python`.
        let trimmed = interpreter.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
        let base = interpreter
            .split_once('.')
            .map_or(interpreter, |(base, _)| base);
        [base, trimmed]
            .into_iter()
            .find_map(|candidate| self.by_shebang.get(candidate))
            .map(|&index| &self.languages[index])
    }

    /// Detects a language from the path, falling back to the shebang line.
    pub fn detect(&self, path: &str, first_line: Option<&str>) -> Option<&LanguageSpec> {
        self.detect_path(path)
            .or_else(|| first_line.and_then(|line| self.detect_shebang(line)))
    }
}

/// Extracts the interpreter name from a shebang line.
///
/// Handles `#!/bin/sh`, `#!/usr/bin/env python3`, and `#!/usr/bin/env -S deno run`.
pub fn shebang_interpreter(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("#!")?.trim();
    let mut parts = rest.split_whitespace();
    let program = parts.next()?;
    let program_name = program.rsplit('/').next()?;
    if program_name == "env" {
        parts.find(|part| !part.starts_with('-'))
    } else {
        Some(program_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::languages::LanguageKind;

    fn id(path: &str) -> Option<&'static str> {
        LanguageRegistry::builtin()
            .detect_path(path)
            .map(|language| language.id.as_str())
    }

    #[test]
    fn detects_common_extensions() {
        assert_eq!(id("src/main.rs"), Some("rust"));
        assert_eq!(id("web/App.TSX"), Some("typescript"));
        assert_eq!(id("lib/index.mjs"), Some("javascript"));
        assert_eq!(id("pkg/server.go"), Some("go"));
        assert_eq!(id("include/api.hpp"), Some("cpp"));
        assert_eq!(id("db/schema.sql"), Some("sql"));
        assert_eq!(id("docs/guide.md"), Some("markdown"));
        assert_eq!(id("unknown.xyz"), None);
        assert_eq!(id("LICENSE"), None);
    }

    #[test]
    fn file_names_win_over_extensions() {
        assert_eq!(id("CMakeLists.txt"), Some("cmake"));
        assert_eq!(id("notes.txt"), Some("text"));
        assert_eq!(id("Dockerfile"), Some("dockerfile"));
        assert_eq!(id("deploy/Dockerfile.prod"), Some("dockerfile"));
        assert_eq!(id("Makefile"), Some("makefile"));
        assert_eq!(id("Cargo.lock"), Some("toml"));
        assert_eq!(id("Gemfile"), Some("ruby"));
    }

    #[test]
    fn parses_shebangs() {
        assert_eq!(shebang_interpreter("#!/bin/bash"), Some("bash"));
        assert_eq!(
            shebang_interpreter("#!/usr/bin/env python3"),
            Some("python3")
        );
        assert_eq!(
            shebang_interpreter("#!/usr/bin/env -S deno run"),
            Some("deno")
        );
        assert_eq!(shebang_interpreter("no shebang"), None);
        let registry = LanguageRegistry::builtin();
        assert_eq!(
            registry
                .detect_shebang("#!/usr/bin/python3.12")
                .map(|l| l.id.as_str()),
            Some("python")
        );
        assert_eq!(
            registry
                .detect("scripts/deploy", Some("#!/bin/sh"))
                .map(|l| l.id.as_str()),
            Some("shell")
        );
    }

    #[test]
    fn custom_languages_extend_and_override() {
        let mut zig = LanguageSpec::new("zig", "Zig", LanguageKind::Programming);
        zig.extensions = vec!["zig".into()];
        let registry = LanguageRegistry::builtin_with(vec![zig]);
        assert_eq!(
            registry.detect_path("build.zig").map(|l| l.id.as_str()),
            Some("zig")
        );
        assert!(registry.get("rust").is_some());

        let mut custom_rust = LanguageSpec::new("rust", "Rust (custom)", LanguageKind::Programming);
        custom_rust.extensions = vec!["rs".into()];
        let registry = LanguageRegistry::builtin_with(vec![custom_rust]);
        assert_eq!(
            registry.get("rust").map(|l| l.name.as_str()),
            Some("Rust (custom)")
        );
        assert_eq!(
            registry
                .languages()
                .iter()
                .filter(|l| l.id == "rust")
                .count(),
            1
        );
    }
}
