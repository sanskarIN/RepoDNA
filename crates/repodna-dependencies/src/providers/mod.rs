//! Built-in ecosystem providers.

pub mod cargo;
pub mod npm;

use crate::model::{EcosystemProvider, FileMatch};

/// Every built-in provider.
pub fn builtin_providers() -> Vec<Box<dyn EcosystemProvider>> {
    vec![Box::new(cargo::Cargo), Box::new(npm::Npm)]
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
