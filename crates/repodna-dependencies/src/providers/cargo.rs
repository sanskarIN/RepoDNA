//! Rust / Cargo: `Cargo.toml` and `Cargo.lock`.

use repodna_core::model::dependencies::{DependencyScope, ManifestKind};

use crate::model::{
    DeclaredDependency, EcosystemProvider, FileMatch, ParsedLockfile, ParsedManifest, file_name,
};

/// Cargo provider.
pub struct Cargo;

fn as_str(value: Option<&toml::Value>) -> Option<String> {
    value.and_then(toml::Value::as_str).map(str::to_owned)
}

fn dependency(name: &str, value: &toml::Value, scope: DependencyScope) -> DeclaredDependency {
    match value {
        toml::Value::String(requirement) => {
            DeclaredDependency::registry(name, Some(requirement.clone()), scope)
        }
        toml::Value::Table(table) => {
            let package = as_str(table.get("package")).unwrap_or_else(|| name.to_owned());
            let optional = table
                .get("optional")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false);
            let scope = if optional && scope == DependencyScope::Runtime {
                DependencyScope::Optional
            } else {
                scope
            };
            let requirement = as_str(table.get("version"));
            let source = if table.contains_key("path") {
                "path"
            } else if table.contains_key("git") {
                "git"
            } else if table.get("workspace").and_then(toml::Value::as_bool) == Some(true) {
                "workspace"
            } else {
                "registry"
            };
            DeclaredDependency::registry(package, requirement, scope).with_source(source)
        }
        _ => DeclaredDependency::registry(name, None, scope),
    }
}

fn collect(table: &toml::Table, manifest: &mut ParsedManifest) {
    let sections = [
        ("dependencies", DependencyScope::Runtime),
        ("dev-dependencies", DependencyScope::Development),
        ("build-dependencies", DependencyScope::Build),
    ];
    for (section, scope) in sections {
        if let Some(entries) = table.get(section).and_then(toml::Value::as_table) {
            for (name, value) in entries {
                manifest.dependencies.push(dependency(name, value, scope));
            }
        }
    }
}

impl EcosystemProvider for Cargo {
    fn ecosystem(&self) -> &'static str {
        "cargo"
    }

    fn matches(&self, path: &str) -> Option<FileMatch> {
        match file_name(path) {
            "Cargo.toml" => Some(FileMatch {
                kind: ManifestKind::Manifest,
            }),
            "Cargo.lock" => Some(FileMatch {
                kind: ManifestKind::Lockfile,
            }),
            _ => None,
        }
    }

    fn parse_manifest(&self, _path: &str, content: &str) -> Result<ParsedManifest, String> {
        let table: toml::Table = content
            .parse()
            .map_err(|error: toml::de::Error| error.to_string())?;
        let mut manifest = ParsedManifest::default();
        if let Some(package) = table.get("package").and_then(toml::Value::as_table) {
            manifest.package_name = as_str(package.get("name"));
            manifest.package_version = as_str(package.get("version"));
            manifest.description = as_str(package.get("description"));
            if let Some(rust) = as_str(package.get("rust-version")) {
                manifest.requirements.push(("Rust".to_owned(), rust));
            }
        }
        collect(&table, &mut manifest);
        if let Some(targets) = table.get("target").and_then(toml::Value::as_table) {
            for target in targets.values().filter_map(toml::Value::as_table) {
                collect(target, &mut manifest);
            }
        }
        if let Some(workspace) = table.get("workspace").and_then(toml::Value::as_table) {
            manifest.workspace_tool = Some("cargo-workspace".to_owned());
            if let Some(members) = workspace.get("members").and_then(toml::Value::as_array) {
                manifest.workspace_members = members
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .map(str::to_owned)
                    .collect();
            }
            if let Some(package) = workspace.get("package").and_then(toml::Value::as_table)
                && let Some(rust) = as_str(package.get("rust-version"))
            {
                manifest.requirements.push(("Rust".to_owned(), rust));
            }
        }
        Ok(manifest)
    }

    fn parse_lockfile(&self, _path: &str, content: &str) -> Result<ParsedLockfile, String> {
        let table: toml::Table = content
            .parse()
            .map_err(|error: toml::de::Error| error.to_string())?;
        let packages = table
            .get("package")
            .and_then(toml::Value::as_array)
            .map(|packages| {
                packages
                    .iter()
                    .filter_map(toml::Value::as_table)
                    .filter_map(|package| {
                        Some((
                            as_str(package.get("name"))?,
                            as_str(package.get("version"))?,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(ParsedLockfile { packages })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cargo_manifests() {
        let manifest = Cargo
            .parse_manifest(
                "Cargo.toml",
                r#"
[package]
name = "demo"
version = "0.3.0"
description = "A demo"
rust-version = "1.80"

[dependencies]
serde = { version = "1", features = ["derive"] }
regex = "1.10"
local = { path = "../local" }
fork = { git = "https://example.invalid/fork" }
shared = { workspace = true }
extra = { version = "2", optional = true }
renamed = { package = "real-name", version = "3" }

[dev-dependencies]
tempfile = "3"

[build-dependencies]
cc = "1"

[target.'cfg(windows)'.dependencies]
winapi = "0.3"
"#,
            )
            .unwrap();
        assert_eq!(manifest.package_name.as_deref(), Some("demo"));
        assert_eq!(manifest.requirements, vec![("Rust".into(), "1.80".into())]);
        let find = |name: &str| {
            manifest
                .dependencies
                .iter()
                .find(|d| d.name == name)
                .unwrap()
        };
        assert_eq!(find("serde").requirement.as_deref(), Some("1"));
        assert_eq!(find("local").source, "path");
        assert_eq!(find("fork").source, "git");
        assert_eq!(find("shared").source, "workspace");
        assert_eq!(find("extra").scope, DependencyScope::Optional);
        assert_eq!(find("real-name").requirement.as_deref(), Some("3"));
        assert_eq!(find("tempfile").scope, DependencyScope::Development);
        assert_eq!(find("cc").scope, DependencyScope::Build);
        assert_eq!(find("winapi").scope, DependencyScope::Runtime);
    }

    #[test]
    fn parses_workspaces_and_lockfiles() {
        let manifest = Cargo
            .parse_manifest("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n")
            .unwrap();
        assert_eq!(manifest.workspace_tool.as_deref(), Some("cargo-workspace"));
        assert_eq!(manifest.workspace_members, vec!["crates/*"]);
        let lock = Cargo
            .parse_lockfile(
                "Cargo.lock",
                "version = 3\n[[package]]\nname = \"a\"\nversion = \"1.0.0\"\n[[package]]\nname = \"a\"\nversion = \"2.0.0\"\n",
            )
            .unwrap();
        assert_eq!(
            lock.packages,
            vec![("a".into(), "1.0.0".into()), ("a".into(), "2.0.0".into())]
        );
        assert!(Cargo.parse_manifest("Cargo.toml", "[package\n").is_err());
    }
}
