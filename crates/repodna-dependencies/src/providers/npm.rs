//! JavaScript / npm: `package.json`, `pnpm-workspace.yaml`, and npm, Yarn, and pnpm lockfiles.

use repodna_core::model::dependencies::{DependencyScope, ManifestKind};
use repodna_core::model::structure::EntrypointKind;
use serde_json::Value;

use crate::model::{
    DeclaredDependency, EcosystemProvider, FileMatch, ParsedLockfile, ParsedManifest, file_name,
};
use crate::yaml;

/// npm provider (covers npm, Yarn, pnpm, and Bun manifests).
pub struct Npm;

fn source_of(requirement: &str) -> &'static str {
    if requirement.starts_with("workspace:") {
        "workspace"
    } else if requirement.starts_with("file:")
        || requirement.starts_with("link:")
        || requirement.starts_with("portal:")
    {
        "path"
    } else if requirement.starts_with("git")
        || requirement.starts_with("github:")
        || requirement.contains("://")
        || (requirement.contains('/')
            && !requirement.starts_with("npm:")
            && !requirement.starts_with('@'))
    {
        "git"
    } else {
        "registry"
    }
}

fn text(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

/// Splits `name@version` where the name may start with `@scope/`.
fn split_name_version(spec: &str) -> Option<(&str, &str)> {
    let position = spec[1..].rfind('@')? + 1;
    Some((&spec[..position], &spec[position + 1..]))
}

impl Npm {
    fn parse_package_json(content: &str) -> Result<ParsedManifest, String> {
        let json: Value = serde_json::from_str(content).map_err(|error| error.to_string())?;
        let mut manifest = ParsedManifest {
            package_name: text(json.get("name")),
            package_version: text(json.get("version")),
            description: text(json.get("description")),
            ..ParsedManifest::default()
        };
        let sections = [
            ("dependencies", DependencyScope::Runtime),
            ("devDependencies", DependencyScope::Development),
            ("peerDependencies", DependencyScope::Peer),
            ("optionalDependencies", DependencyScope::Optional),
        ];
        for (section, scope) in sections {
            if let Some(entries) = json.get(section).and_then(Value::as_object) {
                for (name, value) in entries {
                    let requirement = value.as_str().unwrap_or_default().to_owned();
                    let source = source_of(&requirement);
                    manifest.dependencies.push(
                        DeclaredDependency::registry(name, Some(requirement), scope)
                            .with_source(source),
                    );
                }
            }
        }
        let workspaces = match json.get("workspaces") {
            Some(Value::Array(items)) => items.clone(),
            Some(Value::Object(object)) => object
                .get("packages")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        if !workspaces.is_empty() {
            manifest.workspace_tool = Some("npm-workspaces".to_owned());
            manifest.workspace_members = workspaces
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
        }
        for field in ["main", "module", "browser"] {
            if let Some(entry) = json.get(field).and_then(Value::as_str) {
                manifest.entrypoints.push((
                    entry.trim_start_matches("./").to_owned(),
                    EntrypointKind::Library,
                ));
            }
        }
        match json.get("bin") {
            Some(Value::String(entry)) => {
                manifest.entrypoints.push((
                    entry.trim_start_matches("./").to_owned(),
                    EntrypointKind::Cli,
                ));
            }
            Some(Value::Object(entries)) => {
                for entry in entries.values().filter_map(Value::as_str) {
                    manifest.entrypoints.push((
                        entry.trim_start_matches("./").to_owned(),
                        EntrypointKind::Cli,
                    ));
                }
            }
            _ => {}
        }
        if let Some(node) = json
            .get("engines")
            .and_then(|e| e.get("node"))
            .and_then(Value::as_str)
        {
            manifest
                .requirements
                .push(("Node.js".to_owned(), node.to_owned()));
        }
        if let Some(manager) = json.get("packageManager").and_then(Value::as_str)
            && let Some((name, version)) = split_name_version(manager)
        {
            manifest.requirements.push((
                name.to_owned(),
                version.split('+').next().unwrap_or(version).to_owned(),
            ));
        }
        Ok(manifest)
    }

    fn parse_pnpm_workspace(content: &str) -> Result<ParsedManifest, String> {
        let document = yaml::load(content)?;
        let members = document["packages"]
            .as_vec()
            .map(|items| items.iter().filter_map(yaml::scalar).collect())
            .unwrap_or_default();
        Ok(ParsedManifest {
            workspace_tool: Some("pnpm-workspaces".to_owned()),
            workspace_members: members,
            ..ParsedManifest::default()
        })
    }

    fn parse_package_lock(content: &str) -> Result<ParsedLockfile, String> {
        let json: Value = serde_json::from_str(content).map_err(|error| error.to_string())?;
        let mut packages = Vec::new();
        if let Some(entries) = json.get("packages").and_then(Value::as_object) {
            for (key, value) in entries {
                let Some(name) = key
                    .rsplit("node_modules/")
                    .next()
                    .filter(|_| key.contains("node_modules/"))
                else {
                    continue;
                };
                if value.get("link").and_then(Value::as_bool) == Some(true) {
                    continue;
                }
                if let Some(version) = text(value.get("version")) {
                    packages.push((name.to_owned(), version));
                }
            }
        } else if let Some(dependencies) = json.get("dependencies").and_then(Value::as_object) {
            fn walk(
                dependencies: &serde_json::Map<String, Value>,
                packages: &mut Vec<(String, String)>,
                depth: usize,
            ) {
                if depth > 64 {
                    return;
                }
                for (name, value) in dependencies {
                    if let Some(version) = value.get("version").and_then(Value::as_str) {
                        packages.push((name.clone(), version.to_owned()));
                    }
                    if let Some(nested) = value.get("dependencies").and_then(Value::as_object) {
                        walk(nested, packages, depth + 1);
                    }
                }
            }
            walk(dependencies, &mut packages, 0);
        }
        Ok(ParsedLockfile { packages })
    }

    fn parse_yarn_lock(content: &str) -> ParsedLockfile {
        let mut packages = Vec::new();
        let mut current: Option<String> = None;
        for line in content.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if !line.starts_with(' ') && line.ends_with(':') {
                let first = line
                    .trim_end_matches(':')
                    .split(", ")
                    .next()
                    .unwrap_or_default();
                let spec = first.trim_matches('"');
                current = split_name_version(spec)
                    .map(|(name, _)| name.to_owned())
                    .filter(|name| name != "__metadata");
                continue;
            }
            let trimmed = line.trim_start();
            if let (Some(name), Some(rest)) = (current.as_ref(), trimmed.strip_prefix("version")) {
                let version = rest.trim_start_matches(':').trim().trim_matches('"');
                if !version.is_empty() {
                    packages.push((name.clone(), version.to_owned()));
                }
                current = None;
            }
        }
        ParsedLockfile { packages }
    }

    fn parse_pnpm_lock(content: &str) -> Result<ParsedLockfile, String> {
        let document = yaml::load(content)?;
        let mut packages = Vec::new();
        if let Some(entries) = document["packages"].as_hash() {
            for key in entries.keys().filter_map(yaml::scalar) {
                let key = key.trim_start_matches('/');
                let key = key.split('(').next().unwrap_or(key);
                if let Some((name, version)) = split_name_version(key) {
                    packages.push((name.to_owned(), version.to_owned()));
                } else if let Some((name, version)) = key.rsplit_once('/') {
                    packages.push((name.to_owned(), version.to_owned()));
                }
            }
        }
        Ok(ParsedLockfile { packages })
    }
}

impl EcosystemProvider for Npm {
    fn ecosystem(&self) -> &'static str {
        "npm"
    }

    fn matches(&self, path: &str) -> Option<FileMatch> {
        let kind = match file_name(path) {
            "package.json" | "pnpm-workspace.yaml" => ManifestKind::Manifest,
            "package-lock.json" | "npm-shrinkwrap.json" | "yarn.lock" | "pnpm-lock.yaml" => {
                ManifestKind::Lockfile
            }
            _ => return None,
        };
        Some(FileMatch { kind })
    }

    fn parse_manifest(&self, path: &str, content: &str) -> Result<ParsedManifest, String> {
        if file_name(path) == "pnpm-workspace.yaml" {
            Self::parse_pnpm_workspace(content)
        } else {
            Self::parse_package_json(content)
        }
    }

    fn parse_lockfile(&self, path: &str, content: &str) -> Result<ParsedLockfile, String> {
        match file_name(path) {
            "yarn.lock" => Ok(Self::parse_yarn_lock(content)),
            "pnpm-lock.yaml" => Self::parse_pnpm_lock(content),
            _ => Self::parse_package_lock(content),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_json() {
        let manifest = Npm
            .parse_manifest(
                "package.json",
                r#"{
  "name": "@acme/web",
  "version": "1.0.0",
  "description": "Web app",
  "workspaces": ["packages/*"],
  "engines": { "node": ">=20" },
  "packageManager": "pnpm@9.1.0+sha256.abc",
  "main": "./dist/index.js",
  "bin": { "acme": "bin/acme.js" },
  "dependencies": { "react": "^19.0.0", "shared": "workspace:*", "local": "file:../local", "fork": "github:acme/fork" },
  "devDependencies": { "vitest": "^5.0.0" },
  "peerDependencies": { "react-dom": "*" },
  "optionalDependencies": { "fsevents": "^2" }
}"#,
            )
            .unwrap();
        assert_eq!(manifest.package_name.as_deref(), Some("@acme/web"));
        assert_eq!(manifest.workspace_members, vec!["packages/*"]);
        assert_eq!(
            manifest.entrypoints,
            vec![
                ("dist/index.js".to_owned(), EntrypointKind::Library),
                ("bin/acme.js".to_owned(), EntrypointKind::Cli)
            ]
        );
        assert!(
            manifest
                .requirements
                .contains(&("Node.js".into(), ">=20".into()))
        );
        assert!(
            manifest
                .requirements
                .contains(&("pnpm".into(), "9.1.0".into()))
        );
        let find = |name: &str| {
            manifest
                .dependencies
                .iter()
                .find(|d| d.name == name)
                .unwrap()
        };
        assert_eq!(find("react").source, "registry");
        assert_eq!(find("shared").source, "workspace");
        assert_eq!(find("local").source, "path");
        assert_eq!(find("fork").source, "git");
        assert_eq!(find("vitest").scope, DependencyScope::Development);
        assert_eq!(find("react-dom").scope, DependencyScope::Peer);
        assert_eq!(find("fsevents").scope, DependencyScope::Optional);
    }

    #[test]
    fn parses_npm_lockfiles_v1_and_v3() {
        let v3 = Npm
            .parse_lockfile(
                "package-lock.json",
                r#"{"lockfileVersion":3,"packages":{"":{"name":"root"},"node_modules/a":{"version":"1.0.0"},"node_modules/a/node_modules/@s/b":{"version":"2.0.0"},"node_modules/linked":{"link":true}}}"#,
            )
            .unwrap();
        assert_eq!(
            v3.packages,
            vec![
                ("a".into(), "1.0.0".into()),
                ("@s/b".into(), "2.0.0".into())
            ]
        );
        let v1 = Npm
            .parse_lockfile(
                "package-lock.json",
                r#"{"lockfileVersion":1,"dependencies":{"a":{"version":"1.0.0","dependencies":{"b":{"version":"2.0.0"}}}}}"#,
            )
            .unwrap();
        assert_eq!(v1.packages.len(), 2);
    }

    #[test]
    fn parses_yarn_classic_and_berry() {
        let classic = "# yarn lockfile v1\n\n\"@babel/core@^7.0.0\", \"@babel/core@^7.1.0\":\n  version \"7.24.0\"\n  resolved \"https://example\"\n\nlodash@^4.17.21:\n  version \"4.17.21\"\n";
        assert_eq!(
            Npm.parse_lockfile("yarn.lock", classic).unwrap().packages,
            vec![
                ("@babel/core".into(), "7.24.0".into()),
                ("lodash".into(), "4.17.21".into())
            ]
        );
        let berry = "__metadata:\n  version: 8\n\n\"react@npm:^19.0.0\":\n  version: 19.0.0\n  resolution: \"react@npm:19.0.0\"\n";
        assert_eq!(
            Npm.parse_lockfile("yarn.lock", berry).unwrap().packages,
            vec![("react".into(), "19.0.0".into())]
        );
    }

    #[test]
    fn parses_pnpm_files() {
        let lock = "lockfileVersion: '9.0'\npackages:\n  react@19.0.0:\n    resolution: {integrity: x}\n  '@types/node@22.0.0':\n    resolution: {integrity: y}\n  /old/1.0.0:\n    resolution: {integrity: z}\n  /peer@2.0.0(react@19.0.0):\n    resolution: {integrity: w}\n";
        let packages = Npm.parse_lockfile("pnpm-lock.yaml", lock).unwrap().packages;
        assert!(packages.contains(&("react".into(), "19.0.0".into())));
        assert!(packages.contains(&("@types/node".into(), "22.0.0".into())));
        assert!(packages.contains(&("old".into(), "1.0.0".into())));
        assert!(packages.contains(&("peer".into(), "2.0.0".into())));
        let workspace = Npm
            .parse_manifest(
                "pnpm-workspace.yaml",
                "packages:\n  - 'apps/*'\n  - packages/*\n",
            )
            .unwrap();
        assert_eq!(workspace.workspace_members, vec!["apps/*", "packages/*"]);
    }
}
