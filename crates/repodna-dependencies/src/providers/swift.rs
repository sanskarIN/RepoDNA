//! Swift Package Manager: `Package.swift` and `Package.resolved`.

use std::sync::LazyLock;

use regex::Regex;
use repodna_core::model::dependencies::{DependencyScope, ManifestKind};
use serde_json::Value;

use crate::model::{
    DeclaredDependency, EcosystemProvider, FileMatch, ParsedLockfile, ParsedManifest, file_name,
};

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|error| panic!("invalid Swift pattern {pattern}: {error}"))
}

static TOOLS_VERSION: LazyLock<Regex> =
    LazyLock::new(|| re(r"swift-tools-version\s*:\s*([0-9.]+)"));
static PACKAGE_NAME: LazyLock<Regex> =
    LazyLock::new(|| re(r#"Package\s*\(\s*name\s*:\s*"([^"]+)""#));
static URL: LazyLock<Regex> = LazyLock::new(|| re(r#"url\s*:\s*"([^"]+)""#));
static PATH: LazyLock<Regex> = LazyLock::new(|| re(r#"path\s*:\s*"([^"]+)""#));
static VERSION: LazyLock<Regex> = LazyLock::new(|| re(r#""(\d+\.\d+(?:\.\d+)?[^"]*)""#));

/// SwiftPM provider.
pub struct SwiftPm;

fn repository_name(url: &str) -> String {
    url.trim_end_matches('/')
        .trim_end_matches(".git")
        .rsplit(['/', ':'])
        .next()
        .unwrap_or(url)
        .to_owned()
}

/// Returns the argument text of each `.package(…)` call, honoring nested parentheses.
fn package_calls(content: &str) -> Vec<&str> {
    let mut calls = Vec::new();
    let mut search = 0;
    while let Some(offset) = content[search..].find(".package(") {
        let start = search + offset + ".package(".len();
        let mut depth = 1usize;
        let mut end = None;
        for (index, c) in content[start..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(start + index);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else {
            break;
        };
        calls.push(&content[start..end]);
        search = end;
    }
    calls
}

impl EcosystemProvider for SwiftPm {
    fn ecosystem(&self) -> &'static str {
        "swiftpm"
    }

    fn matches(&self, path: &str) -> Option<FileMatch> {
        let kind = match file_name(path) {
            "Package.swift" => ManifestKind::Manifest,
            "Package.resolved" => ManifestKind::Lockfile,
            _ => return None,
        };
        Some(FileMatch { kind })
    }

    fn parse_manifest(&self, _path: &str, content: &str) -> Result<ParsedManifest, String> {
        let mut manifest = ParsedManifest {
            package_name: PACKAGE_NAME.captures(content).map(|c| c[1].to_owned()),
            ..ParsedManifest::default()
        };
        if let Some(version) = TOOLS_VERSION.captures(content) {
            manifest
                .requirements
                .push(("Swift".to_owned(), version[1].to_owned()));
        }
        for call in package_calls(content) {
            if let Some(url) = URL.captures(call) {
                let version = VERSION
                    .captures(&call[url.get(0).map_or(0, |m| m.end())..])
                    .map(|c| c[1].to_owned());
                manifest.dependencies.push(
                    DeclaredDependency::registry(
                        repository_name(&url[1]),
                        version,
                        DependencyScope::Runtime,
                    )
                    .with_source("git"),
                );
            } else if let Some(path) = PATH.captures(call) {
                manifest.dependencies.push(
                    DeclaredDependency::registry(
                        repository_name(&path[1]),
                        None,
                        DependencyScope::Runtime,
                    )
                    .with_source("path"),
                );
            } else {
                manifest.partial = true;
            }
        }
        Ok(manifest)
    }

    fn parse_lockfile(&self, _path: &str, content: &str) -> Result<ParsedLockfile, String> {
        let json: Value = serde_json::from_str(content).map_err(|error| error.to_string())?;
        let pins = json
            .get("pins")
            .or_else(|| json.get("object").and_then(|object| object.get("pins")))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let packages = pins
            .iter()
            .filter_map(|pin| {
                let name = pin
                    .get("identity")
                    .or_else(|| pin.get("package"))
                    .and_then(Value::as_str)?
                    .to_owned();
                let state = pin.get("state")?;
                let version = state
                    .get("version")
                    .and_then(Value::as_str)
                    .or_else(|| state.get("revision").and_then(Value::as_str))?;
                Some((name, version.to_owned()))
            })
            .collect();
        Ok(ParsedLockfile { packages })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_swift() {
        let manifest = SwiftPm
            .parse_manifest(
                "Package.swift",
                "// swift-tools-version:5.9\nimport PackageDescription\nlet package = Package(\n  name: \"Kit\",\n  dependencies: [\n    .package(url: \"https://github.com/apple/swift-argument-parser.git\", from: \"1.3.0\"),\n    .package(url: \"https://github.com/pointfreeco/swift-snapshot-testing\", .upToNextMajor(from: \"1.15.0\")),\n    .package(path: \"../LocalKit\"),\n  ]\n)\n",
            )
            .unwrap();
        assert_eq!(manifest.package_name.as_deref(), Some("Kit"));
        assert_eq!(manifest.requirements, vec![("Swift".into(), "5.9".into())]);
        let deps: Vec<_> = manifest
            .dependencies
            .iter()
            .map(|d| (d.name.as_str(), d.requirement.as_deref(), d.source.as_str()))
            .collect();
        assert_eq!(
            deps,
            vec![
                ("swift-argument-parser", Some("1.3.0"), "git"),
                ("swift-snapshot-testing", Some("1.15.0"), "git"),
                ("LocalKit", None, "path"),
            ]
        );
    }

    #[test]
    fn parses_package_resolved_v1_and_v2() {
        let v2 = SwiftPm
            .parse_lockfile(
                "Package.resolved",
                r#"{"pins":[{"identity":"swift-argument-parser","state":{"version":"1.3.1"}}],"version":2}"#,
            )
            .unwrap();
        assert_eq!(
            v2.packages,
            vec![("swift-argument-parser".into(), "1.3.1".into())]
        );
        let v1 = SwiftPm
            .parse_lockfile(
                "Package.resolved",
                r#"{"object":{"pins":[{"package":"Alamofire","state":{"version":"5.9.0"}}]},"version":1}"#,
            )
            .unwrap();
        assert_eq!(v1.packages.len(), 1);
    }
}
