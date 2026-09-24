//! Ruby / Bundler: `Gemfile`, `*.gemspec`, and `Gemfile.lock`.

use std::sync::LazyLock;

use regex::Regex;
use repodna_core::model::dependencies::{DependencyScope, ManifestKind};

use crate::model::{
    DeclaredDependency, EcosystemProvider, FileMatch, ParsedLockfile, ParsedManifest, file_name,
};

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|error| panic!("invalid Ruby pattern {pattern}: {error}"))
}

static GEM: LazyLock<Regex> =
    LazyLock::new(|| re(r#"^\s*gem\s+["']([^"']+)["']((?:\s*,\s*["'][^"']*["'])*)"#));
static STRING: LazyLock<Regex> = LazyLock::new(|| re(r#"["']([^"']*)["']"#));
static GROUP: LazyLock<Regex> = LazyLock::new(|| re(r"^\s*group\s+(.+?)\s+do\s*$"));
static GEMSPEC_DEPENDENCY: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r#"\.add_(runtime_dependency|development_dependency|dependency)\s*\(?\s*["']([^"']+)["']((?:\s*,\s*["'][^"']*["'])*)"#,
    )
});
static RUBY_VERSION: LazyLock<Regex> = LazyLock::new(|| re(r#"^\s*ruby\s+["']([^"']+)["']"#));

/// Bundler provider.
pub struct Bundler;

fn requirement(rest: &str) -> Option<String> {
    let parts: Vec<String> = STRING
        .captures_iter(rest)
        .map(|c| c[1].to_owned())
        .collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

impl EcosystemProvider for Bundler {
    fn ecosystem(&self) -> &'static str {
        "rubygems"
    }

    fn matches(&self, path: &str) -> Option<FileMatch> {
        let name = file_name(path);
        let kind = if name == "Gemfile" || name.ends_with(".gemspec") {
            ManifestKind::Manifest
        } else if name == "Gemfile.lock" {
            ManifestKind::Lockfile
        } else {
            return None;
        };
        Some(FileMatch { kind })
    }

    fn parse_manifest(&self, path: &str, content: &str) -> Result<ParsedManifest, String> {
        let mut manifest = ParsedManifest::default();
        if file_name(path).ends_with(".gemspec") {
            manifest.package_name = Some(repodna_core::paths::file_stem(path).to_owned());
            for captures in GEMSPEC_DEPENDENCY.captures_iter(content) {
                let scope = if &captures[1] == "development_dependency" {
                    DependencyScope::Development
                } else {
                    DependencyScope::Runtime
                };
                manifest.dependencies.push(DeclaredDependency::registry(
                    &captures[2],
                    requirement(&captures[3]),
                    scope,
                ));
            }
            return Ok(manifest);
        }
        let mut development_depth = 0usize;
        let mut depth = 0usize;
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(captures) = GROUP.captures(line) {
                depth += 1;
                let groups = &captures[1];
                if groups.contains(":development") || groups.contains(":test") {
                    development_depth = depth;
                }
                continue;
            }
            if trimmed == "end" {
                if depth == development_depth {
                    development_depth = 0;
                }
                depth = depth.saturating_sub(1);
                continue;
            }
            if let Some(captures) = RUBY_VERSION.captures(line) {
                manifest
                    .requirements
                    .push(("Ruby".to_owned(), captures[1].to_owned()));
            }
            if let Some(captures) = GEM.captures(line) {
                let inline_development =
                    trimmed.contains("group: :development") || trimmed.contains("group: :test");
                let scope = if development_depth > 0 || inline_development {
                    DependencyScope::Development
                } else {
                    DependencyScope::Runtime
                };
                let source = if trimmed.contains("path:") {
                    "path"
                } else if trimmed.contains("git:") || trimmed.contains("github:") {
                    "git"
                } else {
                    "registry"
                };
                manifest.dependencies.push(
                    DeclaredDependency::registry(&captures[1], requirement(&captures[2]), scope)
                        .with_source(source),
                );
            }
        }
        Ok(manifest)
    }

    fn parse_lockfile(&self, _path: &str, content: &str) -> Result<ParsedLockfile, String> {
        let mut packages = Vec::new();
        let mut in_specs = false;
        for line in content.lines() {
            if line.trim() == "specs:" {
                in_specs = true;
                continue;
            }
            if line.trim().is_empty() || !line.starts_with(' ') {
                in_specs = false;
                continue;
            }
            // Top-level specs are indented by exactly four spaces: `    name (version)`.
            if in_specs && line.starts_with("    ") && !line.starts_with("     ") {
                let spec = line.trim();
                if let Some((name, version)) = spec.split_once(" (") {
                    packages.push((name.to_owned(), version.trim_end_matches(')').to_owned()));
                }
            }
        }
        Ok(ParsedLockfile { packages })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gemfiles_with_groups() {
        let manifest = Bundler
            .parse_manifest(
                "Gemfile",
                "source 'https://rubygems.org'\nruby '3.3.0'\ngem 'rails', '~> 7.1', '>= 7.1.2'\ngem 'local', path: '../local'\ngroup :development, :test do\n  gem 'rspec-rails'\nend\ngem 'puma'\ngem 'debug', group: :development\n",
            )
            .unwrap();
        let deps: Vec<_> = manifest
            .dependencies
            .iter()
            .map(|d| (d.name.as_str(), d.scope))
            .collect();
        assert_eq!(
            deps,
            vec![
                ("rails", DependencyScope::Runtime),
                ("local", DependencyScope::Runtime),
                ("rspec-rails", DependencyScope::Development),
                ("puma", DependencyScope::Runtime),
                ("debug", DependencyScope::Development),
            ]
        );
        assert_eq!(
            manifest.dependencies[0].requirement.as_deref(),
            Some("~> 7.1, >= 7.1.2")
        );
        assert_eq!(manifest.dependencies[1].source, "path");
        assert_eq!(manifest.requirements, vec![("Ruby".into(), "3.3.0".into())]);
    }

    #[test]
    fn parses_gemspecs_and_lockfiles() {
        let spec = Bundler
            .parse_manifest(
                "widget.gemspec",
                "Gem::Specification.new do |s|\n  s.add_dependency 'thor', '~> 1.3'\n  s.add_development_dependency('rake')\nend\n",
            )
            .unwrap();
        assert_eq!(spec.package_name.as_deref(), Some("widget"));
        assert_eq!(spec.dependencies.len(), 2);
        let lock = Bundler
            .parse_lockfile(
                "Gemfile.lock",
                "GEM\n  remote: https://rubygems.org/\n  specs:\n    rails (7.1.3)\n      actionpack (= 7.1.3)\n    puma (6.4.2)\n\nPLATFORMS\n  ruby\n",
            )
            .unwrap();
        assert_eq!(
            lock.packages,
            vec![
                ("rails".into(), "7.1.3".into()),
                ("puma".into(), "6.4.2".into())
            ]
        );
    }
}
