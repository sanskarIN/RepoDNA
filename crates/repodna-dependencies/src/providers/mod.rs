//! Built-in ecosystem providers.

pub mod cargo;
pub mod dotnet;
pub mod go;
pub mod jvm;
pub mod npm;
pub mod php;
pub mod python;
pub mod ruby;

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
