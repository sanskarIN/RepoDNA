//! Architecture: modules, dependency edges, cycles, layers, and centrality.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{SectionStatus, is_false};
use crate::confidence::Confidence;
use crate::evidence::Evidence;

/// How a module boundary was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ModuleKind {
    /// Declared by a package manifest (a crate, npm package, Go module, …).
    Package,
    /// Inferred from the directory layout inside a package or repository.
    Directory,
    /// Files at the repository root that belong to no other module.
    Root,
}

/// A component of the repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModuleRecord {
    /// Stable identifier (normally the module's root path, or `(root)`).
    pub id: String,
    /// Display name (package name or directory name).
    pub name: String,
    /// Repository-relative root directory of the module.
    pub path: String,
    /// How the boundary was established.
    pub kind: ModuleKind,
    /// Language with the most code lines in the module.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Number of files.
    pub files: u64,
    /// Code lines.
    pub code_lines: u64,
    /// Number of distinct modules that depend on this module.
    pub fan_in: u32,
    /// Number of distinct modules this module depends on.
    pub fan_out: u32,
    /// `fan_out / (fan_in + fan_out)`: 0 means only depended upon, 1 means only depending.
    pub instability: f64,
    /// Normalized betweenness centrality in the module graph (0–1).
    pub centrality: f64,
    /// Layer index in the condensed dependency graph (0 = depends on no other module).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<u32>,
    /// `true` when the boundary is inferred rather than declared.
    pub inferred: bool,
    /// Confidence of the boundary.
    pub confidence: Confidence,
    /// Evidence-backed reasons why this module matters ("18 modules depend on it").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub importance: Vec<String>,
    /// External packages imported from this module.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_dependencies: Vec<String>,
    /// Evidence for the module boundary.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// The syntactic mechanism behind a file-level dependency edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeKind {
    /// `import`, `use`, `require`, or equivalent.
    Import,
    /// C-family `#include`.
    Include,
    /// Module declaration that pulls in another file (e.g. Rust `mod x;`).
    Module,
    /// A string literal that references another repository file.
    Reference,
    /// A dependency between two workspace packages declared in manifests.
    Package,
}

/// A dependency edge between two repository files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileEdge {
    /// Dependent file.
    pub from: String,
    /// Dependency file.
    pub to: String,
    /// Mechanism.
    pub kind: EdgeKind,
    /// Line of the declaration in `from` (1-based).
    pub line: u32,
    /// Confidence of the resolution.
    pub confidence: Confidence,
    /// The import specifier as written, truncated to 120 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specifier: Option<String>,
}

/// A dependency edge between two modules, aggregated from file edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModuleEdge {
    /// Dependent module identifier.
    pub from: String,
    /// Dependency module identifier.
    pub to: String,
    /// Number of file-level edges supporting this module edge.
    pub weight: u32,
    /// Strongest confidence among the supporting file edges.
    pub confidence: Confidence,
    /// Sample of the supporting file edges.
    #[serde(default)]
    pub samples: Vec<Evidence>,
}

/// An external package imported by repository code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExternalImport {
    /// Package or module name, e.g. `serde` or `react`.
    pub name: String,
    /// Ecosystem inferred from the importing language, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ecosystem: Option<String>,
    /// Number of files importing the package.
    pub importers: u32,
    /// Modules importing the package.
    #[serde(default)]
    pub modules: Vec<String>,
    /// Sample import locations.
    #[serde(default)]
    pub samples: Vec<Evidence>,
}

/// The graph level at which a cycle was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CycleLevel {
    /// A cycle between modules.
    Module,
    /// A cycle between files.
    File,
}

/// A dependency cycle (a strongly connected component with more than one member).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DependencyCycle {
    /// Stable identifier derived from the sorted members.
    pub id: String,
    /// Graph level.
    pub level: CycleLevel,
    /// All members of the strongly connected component, sorted.
    pub members: Vec<String>,
    /// A shortest concrete cycle through the component; the first node is repeated at the end.
    pub path: Vec<String>,
    /// Languages of the files involved.
    #[serde(default)]
    pub languages: Vec<String>,
    /// Confidence (the weakest confidence of the edges on `path`).
    pub confidence: Confidence,
    /// The edges along `path`.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// A layer of the condensed module dependency graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    /// Layer index; modules in layer 0 depend on no other internal module.
    pub index: u32,
    /// Module identifiers in the layer.
    pub modules: Vec<String>,
}

/// A package boundary inside a monorepo or workspace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PackageBoundary {
    /// Package name as declared in its manifest.
    pub name: String,
    /// Repository-relative directory.
    pub path: String,
    /// Ecosystem, e.g. `cargo`.
    pub ecosystem: String,
    /// Manifest declaring the package.
    pub manifest: String,
    /// Other workspace packages this package depends on.
    #[serde(default)]
    pub internal_dependencies: Vec<String>,
}

/// A workspace (monorepo) configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    /// Workspace tool, e.g. `cargo-workspace` or `npm-workspaces`.
    pub tool: String,
    /// File declaring the workspace.
    pub manifest: String,
    /// Member patterns as declared.
    #[serde(default)]
    pub members: Vec<String>,
}

/// An architecture-style signal with its evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureSignal {
    /// Identifier, e.g. `monorepo`.
    pub id: String,
    /// Short label.
    pub label: String,
    /// Explanation.
    pub description: String,
    /// Confidence.
    pub confidence: Confidence,
    /// Supporting evidence.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// Inferred architecture of the repository.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureReport {
    /// Whether this section was analyzed.
    pub status: SectionStatus,
    /// Notes about limitations or partial results.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Short characterization such as `Modular` or `Layered`; always labeled as inferred.
    pub style: String,
    /// Confidence of the characterization.
    pub style_confidence: Confidence,
    /// Signals that led to the characterization.
    #[serde(default)]
    pub signals: Vec<ArchitectureSignal>,
    /// Modules, sorted by identifier.
    #[serde(default)]
    pub modules: Vec<ModuleRecord>,
    /// Module-level edges.
    #[serde(default)]
    pub module_edges: Vec<ModuleEdge>,
    /// File-level edges (capped; see `fileEdgesTruncated`).
    #[serde(default)]
    pub file_edges: Vec<FileEdge>,
    /// `true` when `fileEdges` was capped.
    #[serde(default, skip_serializing_if = "is_false")]
    pub file_edges_truncated: bool,
    /// External packages imported by the code.
    #[serde(default)]
    pub external: Vec<ExternalImport>,
    /// Dependency cycles.
    #[serde(default)]
    pub cycles: Vec<DependencyCycle>,
    /// Layers of the condensed module graph.
    #[serde(default)]
    pub layers: Vec<Layer>,
    /// Modules with no internal edges in either direction.
    #[serde(default)]
    pub isolated_modules: Vec<String>,
    /// Package boundaries (monorepo members).
    #[serde(default)]
    pub packages: Vec<PackageBoundary>,
    /// Workspace configuration, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceInfo>,
    /// Import statements resolved to repository files.
    pub resolved_imports: u64,
    /// Import statements that referenced neither a repository file nor a known package.
    pub unresolved_imports: u64,
    /// Description of the inference method.
    pub method: String,
}

impl ArchitectureReport {
    /// Looks up a module by identifier.
    pub fn module(&self, id: &str) -> Option<&ModuleRecord> {
        self.modules.iter().find(|module| module.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_edges_and_cycles() {
        let cycle = DependencyCycle {
            id: "c1".into(),
            level: CycleLevel::Module,
            members: vec!["a".into(), "b".into()],
            path: vec!["a".into(), "b".into(), "a".into()],
            languages: vec!["rust".into()],
            confidence: Confidence::High,
            evidence: vec![Evidence::edge("a", "b"), Evidence::edge("b", "a")],
        };
        let json = serde_json::to_value(&cycle).unwrap();
        assert_eq!(json["level"], "module");
        assert_eq!(json["path"].as_array().unwrap().len(), 3);

        let report = ArchitectureReport::default();
        let json = serde_json::to_value(&report).unwrap();
        assert!(json.get("fileEdgesTruncated").is_none());
        assert_eq!(json["status"], "skipped");
    }
}
