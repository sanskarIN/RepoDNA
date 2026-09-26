//! PHP / Composer: `composer.json` and `composer.lock`.

use repodna_core::model::dependencies::{DependencyScope, ManifestKind};
use serde_json::Value;

use crate::model::{
    DeclaredDependency, EcosystemProvider, FileMatch, ParsedLockfile, ParsedManifest, file_name,
};

/// Composer provider.
pub struct Composer;

impl EcosystemProvider for Composer {
    fn ecosystem(&self) -> &'static str {
        "composer"
    }

    fn matches(&self, path: &str) -> Option<FileMatch> {
        let kind = match file_name(path) {
            "composer.json" => ManifestKind::Manifest,
            "composer.lock" => ManifestKind::Lockfile,
            _ => return None,
        };
        Some(FileMatch { kind })
    }

    fn parse_manifest(&self, _path: &str, content: &str) -> Result<ParsedManifest, String> {
        let json: Value = serde_json::from_str(content).map_err(|error| error.to_string())?;
        let mut manifest = ParsedManifest {
            package_name: json.get("name").and_then(Value::as_str).map(str::to_owned),
            package_version: json
                .get("version")
                .and_then(Value::as_str)
                .map(str::to_owned),
            description: json
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_owned),
            ..ParsedManifest::default()
        };
        for (section, scope) in [
            ("require", DependencyScope::Runtime),
            ("require-dev", DependencyScope::Development),
        ] {
            if let Some(entries) = json.get(section).and_then(Value::as_object) {
                for (name, requirement) in entries {
                    let requirement = requirement.as_str().map(str::to_owned);
                    if name == "php" {
                        if let Some(version) = requirement {
                            manifest.requirements.push(("PHP".to_owned(), version));
                        }
                        continue;
                    }
                    if name.starts_with("ext-") || name.starts_with("lib-") {
                        continue;
                    }
                    manifest.dependencies.push(DeclaredDependency::registry(
                        name,
                        requirement,
                        scope,
                    ));
                }
            }
        }
        Ok(manifest)
    }

    fn parse_lockfile(&self, _path: &str, content: &str) -> Result<ParsedLockfile, String> {
        let json: Value = serde_json::from_str(content).map_err(|error| error.to_string())?;
        let mut packages = Vec::new();
        for section in ["packages", "packages-dev"] {
            if let Some(entries) = json.get(section).and_then(Value::as_array) {
                for entry in entries {
                    if let (Some(name), Some(version)) = (
                        entry.get("name").and_then(Value::as_str),
                        entry.get("version").and_then(Value::as_str),
                    ) {
                        packages
                            .push((name.to_owned(), version.trim_start_matches('v').to_owned()));
                    }
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
    fn parses_composer_files() {
        let manifest = Composer
            .parse_manifest(
                "composer.json",
                r#"{"name":"acme/shop","require":{"php":"^8.2","ext-json":"*","laravel/framework":"^11.0"},"require-dev":{"phpunit/phpunit":"^11"}}"#,
            )
            .unwrap();
        assert_eq!(manifest.package_name.as_deref(), Some("acme/shop"));
        assert_eq!(manifest.dependencies.len(), 2);
        assert_eq!(manifest.requirements, vec![("PHP".into(), "^8.2".into())]);
        let lock = Composer
            .parse_lockfile(
                "composer.lock",
                r#"{"packages":[{"name":"laravel/framework","version":"v11.5.0"}],"packages-dev":[{"name":"phpunit/phpunit","version":"11.1.0"}]}"#,
            )
            .unwrap();
        assert_eq!(
            lock.packages[0],
            ("laravel/framework".into(), "11.5.0".into())
        );
    }
}
