//! .NET / NuGet: SDK-style project files, `packages.config`, central package management
//! (`Directory.Packages.props`), and `packages.lock.json`.

use std::sync::LazyLock;

use regex::Regex;
use repodna_core::model::dependencies::{DependencyScope, ManifestKind};
use serde_json::Value;

use crate::model::{
    DeclaredDependency, EcosystemProvider, FileMatch, ParsedLockfile, ParsedManifest, file_name,
};

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|error| panic!("invalid NuGet pattern {pattern}: {error}"))
}

static XML_COMMENT: LazyLock<Regex> = LazyLock::new(|| re(r"(?s)<!--.*?-->"));
static PACKAGE_REFERENCE: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r#"(?s)<Package(?:Reference|Version)\s+[^>]*?(?:Include|Update)\s*=\s*"([^"]+)"([^>]*?)(?:/>|>(.*?)</Package(?:Reference|Version)>)"#,
    )
});
static VERSION_ATTRIBUTE: LazyLock<Regex> = LazyLock::new(|| re(r#"\bVersion\s*=\s*"([^"]+)""#));
static VERSION_ELEMENT: LazyLock<Regex> =
    LazyLock::new(|| re(r"<Version>\s*([^<]+?)\s*</Version>"));
static PACKAGES_CONFIG: LazyLock<Regex> = LazyLock::new(|| re(r#"<package\s+([^>]*)/?>"#));
static ATTRIBUTE: LazyLock<Regex> = LazyLock::new(|| re(r#"(\w+)\s*=\s*"([^"]*)""#));
static TARGET_FRAMEWORK: LazyLock<Regex> =
    LazyLock::new(|| re(r"<TargetFrameworks?>\s*([^<]+?)\s*</TargetFrameworks?>"));

/// NuGet provider.
pub struct NuGet;

impl EcosystemProvider for NuGet {
    fn ecosystem(&self) -> &'static str {
        "nuget"
    }

    fn matches(&self, path: &str) -> Option<FileMatch> {
        let name = file_name(path);
        let kind = if name.ends_with(".csproj")
            || name.ends_with(".fsproj")
            || name.ends_with(".vbproj")
            || matches!(name, "packages.config" | "Directory.Packages.props")
        {
            ManifestKind::Manifest
        } else if name == "packages.lock.json" {
            ManifestKind::Lockfile
        } else {
            return None;
        };
        Some(FileMatch { kind })
    }

    fn parse_manifest(&self, path: &str, content: &str) -> Result<ParsedManifest, String> {
        let text = XML_COMMENT.replace_all(content, "");
        let mut manifest = ParsedManifest::default();
        if file_name(path) == "packages.config" {
            for captures in PACKAGES_CONFIG.captures_iter(&text) {
                let attributes: Vec<(String, String)> = ATTRIBUTE
                    .captures_iter(&captures[1])
                    .map(|a| (a[1].to_owned(), a[2].to_owned()))
                    .collect();
                let get = |key: &str| {
                    attributes
                        .iter()
                        .find(|(k, _)| k == key)
                        .map(|(_, v)| v.clone())
                };
                let Some(id) = get("id") else {
                    manifest.partial = true;
                    continue;
                };
                let scope = if get("developmentDependency").as_deref() == Some("true") {
                    DependencyScope::Development
                } else {
                    DependencyScope::Runtime
                };
                manifest
                    .dependencies
                    .push(DeclaredDependency::registry(id, get("version"), scope));
            }
            return Ok(manifest);
        }
        if !text.contains("<Project") {
            return Err("project file has no <Project> element".to_owned());
        }
        for captures in PACKAGE_REFERENCE.captures_iter(&text) {
            let attributes = captures.get(2).map_or("", |m| m.as_str());
            let body = captures.get(3).map_or("", |m| m.as_str());
            let version = VERSION_ATTRIBUTE
                .captures(attributes)
                .or_else(|| VERSION_ELEMENT.captures(body))
                .map(|c| c[1].to_owned());
            manifest.dependencies.push(DeclaredDependency::registry(
                captures[1].to_owned(),
                version,
                DependencyScope::Runtime,
            ));
        }
        if let Some(framework) = TARGET_FRAMEWORK.captures(&text) {
            manifest
                .requirements
                .push((".NET".to_owned(), framework[1].to_owned()));
        }
        manifest.package_name = (file_name(path) != "Directory.Packages.props")
            .then(|| repodna_core::paths::file_stem(path).to_owned());
        Ok(manifest)
    }

    fn parse_lockfile(&self, _path: &str, content: &str) -> Result<ParsedLockfile, String> {
        let json: Value = serde_json::from_str(content).map_err(|error| error.to_string())?;
        let mut packages = Vec::new();
        if let Some(frameworks) = json.get("dependencies").and_then(Value::as_object) {
            for entries in frameworks.values().filter_map(Value::as_object) {
                for (name, entry) in entries {
                    if let Some(version) = entry.get("resolved").and_then(Value::as_str) {
                        packages.push((name.clone(), version.to_owned()));
                    }
                }
            }
        }
        packages.sort();
        packages.dedup();
        Ok(ParsedLockfile { packages })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sdk_style_projects() {
        let manifest = NuGet
            .parse_manifest(
                "src/Api/Api.csproj",
                r#"<Project Sdk="Microsoft.NET.Sdk.Web">
  <PropertyGroup><TargetFramework>net8.0</TargetFramework></PropertyGroup>
  <ItemGroup>
    <PackageReference Include="Serilog" Version="3.1.1" />
    <PackageReference Include="Dapper">
      <Version>2.1.28</Version>
    </PackageReference>
    <!-- <PackageReference Include="Commented" Version="1" /> -->
    <PackageReference Include="Central" />
  </ItemGroup>
</Project>"#,
            )
            .unwrap();
        let deps: Vec<_> = manifest
            .dependencies
            .iter()
            .map(|d| (d.name.as_str(), d.requirement.as_deref()))
            .collect();
        assert_eq!(
            deps,
            vec![
                ("Serilog", Some("3.1.1")),
                ("Dapper", Some("2.1.28")),
                ("Central", None)
            ]
        );
        assert_eq!(manifest.package_name.as_deref(), Some("Api"));
        assert_eq!(
            manifest.requirements,
            vec![(".NET".into(), "net8.0".into())]
        );
    }

    #[test]
    fn parses_packages_config_and_lockfiles() {
        let manifest = NuGet
            .parse_manifest(
                "packages.config",
                r#"<packages><package id="Newtonsoft.Json" version="13.0.3" /><package id="StyleCop" version="1.0" developmentDependency="true" /></packages>"#,
            )
            .unwrap();
        assert_eq!(manifest.dependencies.len(), 2);
        assert_eq!(manifest.dependencies[1].scope, DependencyScope::Development);
        let lock = NuGet
            .parse_lockfile(
                "packages.lock.json",
                r#"{"version":1,"dependencies":{"net8.0":{"Serilog":{"type":"Direct","resolved":"3.1.1"},"Transitive.Lib":{"type":"Transitive","resolved":"1.0.0"}}}}"#,
            )
            .unwrap();
        assert_eq!(lock.packages.len(), 2);
    }
}
