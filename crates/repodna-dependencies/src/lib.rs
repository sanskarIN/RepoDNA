//! # repodna-dependencies
//!
//! Reads dependency manifests and lockfiles across ecosystems through pluggable
//! [`EcosystemProvider`]s. Providers only read file contents: they never run package
//! managers or contact registries, so results work offline and are reproducible.
//! Vulnerability data is out of scope unless an advisory provider is added, and the report
//! says so explicitly.

pub mod model;
pub mod yaml;

pub use model::{DeclaredDependency, EcosystemProvider, FileMatch, ParsedLockfile, ParsedManifest};
