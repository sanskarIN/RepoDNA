//! Dart and Flutter / pub: `pubspec.yaml` and `pubspec.lock`.

use repodna_core::model::dependencies::{DependencyScope, ManifestKind};
use yaml_rust2::Yaml;

use crate::model::{
    DeclaredDependency, EcosystemProvider, FileMatch, ParsedLockfile, ParsedManifest, file_name,
};
use crate::yaml;

/// pub provider.
pub struct Pub;

fn dependencies(section: &Yaml, scope: DependencyScope, manifest: &mut ParsedManifest) {
    let Some(entries) = section.as_hash() else {
        return;
    };
    for (key, value) in entries {
        let Some(name) = yaml::scalar(key) else {
            continue;
        };
        let (requirement, source) = match value {
            Yaml::Hash(_) => {
                let source = if !value["sdk"].is_badvalue() {
                    "sdk"
                } else if !value["path"].is_badvalue() {
                    "path"
                } else if !value["git"].is_badvalue() {
                    "git"
                } else {
                    "registry"
                };
                (yaml::scalar(&value["version"]), source)
            }
            Yaml::Null => (None, "registry"),
            other => (yaml::scalar(other), "registry"),
        };
        manifest
            .dependencies
            .push(DeclaredDependency::registry(name, requirement, scope).with_source(source));
    }
}

impl EcosystemProvider for Pub {
    fn ecosystem(&self) -> &'static str {
        "pub"
    }

    fn matches(&self, path: &str) -> Option<FileMatch> {
        let kind = match file_name(path) {
            "pubspec.yaml" => ManifestKind::Manifest,
            "pubspec.lock" => ManifestKind::Lockfile,
            _ => return None,
        };
        Some(FileMatch { kind })
    }

    fn parse_manifest(&self, _path: &str, content: &str) -> Result<ParsedManifest, String> {
        let document = yaml::load(content)?;
        if document.as_hash().is_none() {
            return Err("pubspec.yaml is not a mapping".to_owned());
        }
        let mut manifest = ParsedManifest {
            package_name: yaml::scalar(&document["name"]),
            package_version: yaml::scalar(&document["version"]),
            description: yaml::scalar(&document["description"]),
            ..ParsedManifest::default()
        };
        dependencies(
            &document["dependencies"],
            DependencyScope::Runtime,
            &mut manifest,
        );
        dependencies(
            &document["dev_dependencies"],
            DependencyScope::Development,
            &mut manifest,
        );
        if let Some(sdk) = yaml::scalar(&document["environment"]["sdk"]) {
            manifest.requirements.push(("Dart".to_owned(), sdk));
        }
        if let Some(flutter) = yaml::scalar(&document["environment"]["flutter"]) {
            manifest.requirements.push(("Flutter".to_owned(), flutter));
        } else if manifest.dependencies.iter().any(|d| d.name == "flutter") {
            manifest
                .requirements
                .push(("Flutter".to_owned(), "any".to_owned()));
        }
        if let Some(members) = document["workspace"].as_vec() {
            manifest.workspace_tool = Some("pub-workspace".to_owned());
            manifest.workspace_members = members.iter().filter_map(yaml::scalar).collect();
        }
        Ok(manifest)
    }

    fn parse_lockfile(&self, _path: &str, content: &str) -> Result<ParsedLockfile, String> {
        let document = yaml::load(content)?;
        let packages = document["packages"]
            .as_hash()
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|(name, entry)| {
                        Some((yaml::scalar(name)?, yaml::scalar(&entry["version"])?))
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
    fn parses_pubspecs() {
        let manifest = Pub
            .parse_manifest(
                "pubspec.yaml",
                "name: app\nversion: 1.0.0+1\nenvironment:\n  sdk: '>=3.3.0 <4.0.0'\ndependencies:\n  flutter:\n    sdk: flutter\n  http: ^1.2.0\n  local:\n    path: ../local\ndev_dependencies:\n  flutter_test:\n    sdk: flutter\n  lints:\n",
            )
            .unwrap();
        assert_eq!(manifest.package_name.as_deref(), Some("app"));
        let deps: Vec<_> = manifest
            .dependencies
            .iter()
            .map(|d| (d.name.as_str(), d.source.as_str()))
            .collect();
        assert_eq!(
            deps,
            vec![
                ("flutter", "sdk"),
                ("http", "registry"),
                ("local", "path"),
                ("flutter_test", "sdk"),
                ("lints", "registry")
            ]
        );
        assert!(
            manifest
                .requirements
                .iter()
                .any(|(name, _)| name == "Flutter")
        );
        assert!(
            Pub.parse_manifest("pubspec.yaml", "- just a list\n")
                .is_err()
        );
    }

    #[test]
    fn parses_pubspec_lock() {
        let lock = Pub
            .parse_lockfile(
                "pubspec.lock",
                "packages:\n  http:\n    dependency: direct main\n    version: \"1.2.1\"\n  meta:\n    version: \"1.12.0\"\nsdks:\n  dart: \">=3.3.0\"\n",
            )
            .unwrap();
        assert_eq!(
            lock.packages,
            vec![
                ("http".into(), "1.2.1".into()),
                ("meta".into(), "1.12.0".into())
            ]
        );
    }
}
