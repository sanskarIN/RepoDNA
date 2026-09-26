//! Declared dependencies: manifests, lockfiles, and consistency signals.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{SectionStatus, is_false};
use crate::time::Timestamp;

/// The scope in which a dependency is used.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyScope {
    /// Needed at runtime.
    Runtime,
    /// Needed only for development or tests.
    Development,
    /// Needed only to build the project.
    Build,
    /// Optional feature dependency.
    Optional,
    /// Expected to be provided by the consumer (npm peer dependency).
    Peer,
}

/// Role of a dependency file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ManifestKind {
    /// Declares dependencies (e.g. `package.json`).
    Manifest,
    /// Pins resolved versions (e.g. `package-lock.json`).
    Lockfile,
}

/// Outcome of parsing a dependency file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ParseStatus {
    /// Parsed completely.
    Parsed,
    /// Parsed with some entries skipped.
    Partial,
    /// Could not be parsed; see the message.
    Failed,
}

/// A manifest or lockfile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManifestRecord {
    /// Repository-relative path.
    pub path: String,
    /// Ecosystem identifier, e.g. `cargo`, `npm`, or `pypi`.
    pub ecosystem: String,
    /// Manifest or lockfile.
    pub kind: ManifestKind,
    /// Declared package name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_name: Option<String>,
    /// Declared package version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_version: Option<String>,
    /// Declared package description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Number of dependencies declared (manifests) or packages pinned (lockfiles).
    pub entries: u32,
    /// Parse outcome.
    pub status: ParseStatus,
    /// Explanation when parsing was partial or failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// A direct dependency declared in one or more manifests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DependencyRecord {
    /// Package name.
    pub name: String,
    /// Ecosystem identifier.
    pub ecosystem: String,
    /// Declared version requirement, as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement: Option<String>,
    /// Usage scope.
    pub scope: DependencyScope,
    /// Manifests declaring the dependency.
    pub manifests: Vec<String>,
    /// Versions resolved in lockfiles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolved: Vec<String>,
    /// Source kind: `registry`, `git`, `path`, or `workspace`.
    pub source: String,
    /// `true` when the dependency is another package of the same repository.
    #[serde(default, skip_serializing_if = "is_false")]
    pub internal: bool,
}

/// A lockfile and its consistency with the manifests it belongs to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LockfileRecord {
    /// Repository-relative path.
    pub path: String,
    /// Ecosystem identifier.
    pub ecosystem: String,
    /// Packages pinned.
    pub packages: u32,
    /// Parse outcome.
    pub status: ParseStatus,
    /// Direct registry dependencies of the sibling manifest that the lockfile does not contain.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_from_lock: Vec<String>,
    /// Explanation when parsing was partial or failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// A package resolved at several versions in one lockfile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateVersions {
    /// Ecosystem identifier.
    pub ecosystem: String,
    /// Package name.
    pub name: String,
    /// Distinct resolved versions, sorted.
    pub versions: Vec<String>,
    /// Lockfile containing the duplicates.
    pub lockfile: String,
}

/// A manifest whose dependency declarations have not changed for a long time.
///
/// Without an advisory or registry provider RepoDNA cannot know whether newer versions exist;
/// this signal only reports how long the declarations have been unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaleSignal {
    /// Manifest path.
    pub manifest: String,
    /// Latest commit that changed the manifest.
    pub last_changed: Timestamp,
    /// Days between that change and the latest commit in the repository.
    pub days_unchanged: i64,
    /// Neutral description.
    pub description: String,
}

/// Counts per ecosystem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EcosystemSummary {
    /// Ecosystem identifier.
    pub ecosystem: String,
    /// Manifests found.
    pub manifests: u32,
    /// Lockfiles found.
    pub lockfiles: u32,
    /// Direct runtime dependencies.
    pub runtime: u32,
    /// Direct development, build, optional, and peer dependencies.
    pub other: u32,
    /// Packages pinned across lockfiles (direct and transitive).
    pub locked_packages: u32,
}

/// Status of vulnerability-advisory integration.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdvisoryStatus {
    /// Advisory provider used, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// When the advisory data was retrieved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<Timestamp>,
    /// Explanation, e.g. that no provider was configured.
    pub note: String,
}

/// How often an external package is imported by repository code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DependencyConcentration {
    /// Package name.
    pub name: String,
    /// Files importing it.
    pub importers: u32,
    /// Share of all external-import sites (0–1).
    pub share: f64,
}

/// Dependency analysis.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DependencyReport {
    /// Whether this section was analyzed.
    pub status: SectionStatus,
    /// Notes about limitations or partial results.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Manifests and lockfiles found.
    #[serde(default)]
    pub manifests: Vec<ManifestRecord>,
    /// Direct dependencies, sorted by ecosystem and name.
    #[serde(default)]
    pub dependencies: Vec<DependencyRecord>,
    /// Lockfiles with consistency information.
    #[serde(default)]
    pub lockfiles: Vec<LockfileRecord>,
    /// Packages resolved at multiple versions.
    #[serde(default)]
    pub duplicates: Vec<DuplicateVersions>,
    /// Long-unchanged manifests.
    #[serde(default)]
    pub stale_signals: Vec<StaleSignal>,
    /// Counts per ecosystem.
    #[serde(default)]
    pub ecosystems: Vec<EcosystemSummary>,
    /// Most frequently imported external packages.
    #[serde(default)]
    pub concentration: Vec<DependencyConcentration>,
    /// Number of distinct direct dependencies.
    pub direct_count: u64,
    /// Number of distinct packages pinned in lockfiles (direct and transitive).
    pub locked_count: u64,
    /// Advisory integration status.
    #[serde(default)]
    pub advisories: AdvisoryStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_dependency_records() {
        let record = DependencyRecord {
            name: "serde".into(),
            ecosystem: "cargo".into(),
            requirement: Some("1.0".into()),
            scope: DependencyScope::Runtime,
            manifests: vec!["Cargo.toml".into()],
            resolved: vec!["1.0.229".into()],
            source: "registry".into(),
            internal: false,
        };
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["scope"], "runtime");
        assert!(json.get("internal").is_none());
        assert_eq!(
            serde_json::from_value::<DependencyRecord>(json).unwrap(),
            record
        );
    }
}
