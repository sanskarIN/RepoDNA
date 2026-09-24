//! Built-in ecosystem providers.

pub mod cargo;
pub mod dart;
pub mod dotnet;
pub mod go;
pub mod jvm;
pub mod npm;
pub mod php;
pub mod python;
pub mod ruby;
pub mod swift;

use crate::model::{EcosystemProvider, FileMatch};

/// Every built-in provider.
pub fn builtin_providers() -> Vec<Box<dyn EcosystemProvider>> {
    vec![
        Box::new(cargo::Cargo),
        Box::new(npm::Npm),
        Box::new(python::Python),
        Box::new(go::Go),
        Box::new(jvm::Maven),
        Box::new(jvm::Gradle),
        Box::new(dotnet::NuGet),
        Box::new(php::Composer),
        Box::new(ruby::Bundler),
        Box::new(dart::Pub),
        Box::new(swift::SwiftPm),
    ]
}

/// Finds the provider that handles `path`.
pub fn find_provider<'a>(
    providers: &'a [Box<dyn EcosystemProvider>],
    path: &str,
) -> Option<(&'a dyn EcosystemProvider, FileMatch)> {
    providers.iter().find_map(|provider| {
        provider
            .matches(path)
            .map(|found| (provider.as_ref(), found))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::dependencies::ManifestKind;

    #[test]
    fn every_known_file_has_exactly_one_provider() {
        let providers = builtin_providers();
        for (path, ecosystem, kind) in [
            ("Cargo.toml", "cargo", ManifestKind::Manifest),
            ("web/package-lock.json", "npm", ManifestKind::Lockfile),
            ("requirements-dev.txt", "pypi", ManifestKind::Manifest),
            ("go.sum", "go", ManifestKind::Lockfile),
            ("pom.xml", "maven", ManifestKind::Manifest),
            ("app/build.gradle.kts", "gradle", ManifestKind::Manifest),
            ("src/App/App.csproj", "nuget", ManifestKind::Manifest),
            ("composer.lock", "composer", ManifestKind::Lockfile),
            ("Gemfile", "rubygems", ManifestKind::Manifest),
            ("pubspec.yaml", "pub", ManifestKind::Manifest),
            ("Package.resolved", "swiftpm", ManifestKind::Lockfile),
        ] {
            let matching: Vec<_> = providers
                .iter()
                .filter_map(|p| p.matches(path).map(|m| (p.ecosystem(), m.kind)))
                .collect();
            assert_eq!(matching, vec![(ecosystem, kind)], "{path}");
        }
        assert!(find_provider(&providers, "README.md").is_none());
    }
}
