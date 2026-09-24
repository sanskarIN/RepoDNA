//! Intermediate results produced by ecosystem providers.

use repodna_core::model::dependencies::{DependencyScope, ManifestKind};
use repodna_core::model::structure::EntrypointKind;

/// A dependency declared in a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredDependency {
    /// Package name as declared (normalized where the ecosystem defines normalization).
    pub name: String,
    /// Version requirement as written.
    pub requirement: Option<String>,
    /// Usage scope.
    pub scope: DependencyScope,
    /// `registry`, `git`, `path`, `workspace` (a package of the same workspace), or
    /// `inherited` (declared in a shared workspace table and resolved during aggregation).
    pub source: String,
}

impl DeclaredDependency {
    /// A registry dependency.
    pub fn registry(
        name: impl Into<String>,
        requirement: Option<String>,
        scope: DependencyScope,
    ) -> Self {
        Self {
            name: name.into(),
            requirement: requirement.filter(|r| !r.trim().is_empty()),
            scope,
            source: "registry".to_owned(),
        }
    }

    /// Sets the source kind.
    #[must_use]
    pub fn with_source(mut self, source: &str) -> Self {
        self.source = source.to_owned();
        self
    }
}

/// A parsed manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedManifest {
    /// Declared package name.
    pub package_name: Option<String>,
    /// Declared package version.
    pub package_version: Option<String>,
    /// Declared description.
    pub description: Option<String>,
    /// Declared dependencies.
    pub dependencies: Vec<DeclaredDependency>,
    /// Declarations that member packages can inherit instead of repeating them, such as
    /// Cargo's `[workspace.dependencies]`. They are not dependencies of this manifest.
    pub shared_dependencies: Vec<DeclaredDependency>,
    /// Workspace member patterns declared by this manifest.
    pub workspace_members: Vec<String>,
    /// Workspace tool, e.g. `cargo-workspace`, when this manifest declares a workspace.
    pub workspace_tool: Option<String>,
    /// Toolchain or runtime requirements, e.g. `("Go", "1.22")`.
    pub requirements: Vec<(String, String)>,
    /// Entrypoints declared by the manifest, as paths relative to the manifest's directory.
    pub entrypoints: Vec<(String, EntrypointKind)>,
    /// `true` when some entries could not be interpreted.
    pub partial: bool,
}

/// A parsed lockfile: `(name, version)` pairs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedLockfile {
    /// Pinned packages.
    pub packages: Vec<(String, String)>,
}

/// What a provider can do with a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileMatch {
    /// Manifest or lockfile.
    pub kind: ManifestKind,
}

/// An ecosystem-specific parser for manifests and lockfiles.
///
/// Providers are pure functions of file content: they never run package managers, never
/// access the network, and report unparseable input as an error instead of guessing.
pub trait EcosystemProvider: Send + Sync {
    /// Ecosystem identifier, e.g. `cargo`.
    fn ecosystem(&self) -> &'static str;
    /// Returns whether and how the provider handles `path` (repository-relative).
    fn matches(&self, path: &str) -> Option<FileMatch>;
    /// Parses a manifest.
    fn parse_manifest(&self, path: &str, content: &str) -> Result<ParsedManifest, String>;
    /// Parses a lockfile.
    fn parse_lockfile(&self, path: &str, content: &str) -> Result<ParsedLockfile, String>;
}

/// Returns the file name of a repository path.
pub fn file_name(path: &str) -> &str {
    repodna_core::paths::file_name(path)
}
