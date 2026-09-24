//! Combining provider results into the dependency section.

use std::collections::{BTreeMap, BTreeSet};

use repodna_core::model::SectionStatus;
use repodna_core::model::architecture::{PackageBoundary, WorkspaceInfo};
use repodna_core::model::dependencies::{
    AdvisoryStatus, DependencyRecord, DependencyReport, DependencyScope, DuplicateVersions,
    EcosystemSummary, LockfileRecord, ManifestKind, ManifestRecord, ParseStatus,
};
use repodna_core::model::project::{EnvironmentRequirement, RequirementKind};
use repodna_core::model::structure::EntrypointKind;
use repodna_core::paths;

use crate::model::{EcosystemProvider, ParsedLockfile, ParsedManifest};
use crate::providers::{builtin_providers, find_provider};

/// A manifest or lockfile to analyze.
#[derive(Debug, Clone, Copy)]
pub struct DependencyFile<'a> {
    /// Repository-relative path.
    pub path: &'a str,
    /// File content.
    pub content: &'a str,
}

/// An entrypoint declared in a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredEntrypoint {
    /// Manifest that declares it.
    pub manifest: String,
    /// Repository-relative path of the entrypoint.
    pub path: String,
    /// Kind of entrypoint.
    pub kind: EntrypointKind,
}

/// Everything learned from manifests and lockfiles.
#[derive(Debug, Clone, Default)]
pub struct DependencyAnalysis {
    /// The dependency section (stale signals, concentration, and advisories are completed
    /// by the engine).
    pub report: DependencyReport,
    /// Packages declared by manifests (for architecture and monorepo views).
    pub packages: Vec<PackageBoundary>,
    /// Workspace configuration, when declared.
    pub workspace: Option<WorkspaceInfo>,
    /// Runtime and toolchain requirements declared by manifests.
    pub requirements: Vec<EnvironmentRequirement>,
    /// Package descriptions as `(manifest path, description)`.
    pub descriptions: Vec<(String, String)>,
    /// Entrypoints declared by manifests.
    pub entrypoints: Vec<DeclaredEntrypoint>,
}

/// Maximum duplicate-version entries reported.
const MAX_DUPLICATES: usize = 200;

/// Returns `true` if `path` is a manifest or lockfile understood by a built-in provider.
pub fn is_dependency_file(path: &str) -> bool {
    builtin_providers()
        .iter()
        .any(|provider| provider.matches(path).is_some())
}

fn requirement_kind(name: &str) -> RequirementKind {
    match name {
        "Rust" | "Go" | "Go toolchain" | "Java" | "Swift" => RequirementKind::Toolchain,
        ".NET" | "Dart" | "Flutter" => RequirementKind::Sdk,
        "npm" | "pnpm" | "yarn" | "bun" => RequirementKind::PackageManager,
        _ => RequirementKind::Runtime,
    }
}

fn scope_rank(scope: DependencyScope) -> u8 {
    match scope {
        DependencyScope::Runtime => 0,
        DependencyScope::Peer => 1,
        DependencyScope::Optional => 2,
        DependencyScope::Build => 3,
        DependencyScope::Development => 4,
    }
}

struct Parsed<'a> {
    path: &'a str,
    ecosystem: &'static str,
    manifest: Option<ParsedManifest>,
    lockfile: Option<ParsedLockfile>,
}

/// Analyzes manifests and lockfiles.
pub fn analyze(files: &[DependencyFile<'_>]) -> DependencyAnalysis {
    let providers = builtin_providers();
    let mut analysis = DependencyAnalysis::default();
    let mut parsed: Vec<Parsed<'_>> = Vec::new();
    let mut failures = 0usize;

    let mut sorted: Vec<&DependencyFile<'_>> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(b.path));
    for file in sorted {
        let Some((provider, found)) = find_provider(&providers, file.path) else {
            continue;
        };
        let record = parse_one(provider, found.kind, file, &mut parsed);
        if record.status != ParseStatus::Parsed {
            failures += 1;
        }
        if record.kind == ManifestKind::Lockfile {
            analysis.report.lockfiles.push(LockfileRecord {
                path: record.path.clone(),
                ecosystem: record.ecosystem.clone(),
                packages: record.entries,
                status: record.status,
                missing_from_lock: Vec::new(),
                message: record.message.clone(),
            });
        }
        analysis.report.manifests.push(record);
    }

    // Declared packages and workspace configuration.
    let mut declared: BTreeMap<(&str, String), (String, String)> = BTreeMap::new();
    for item in &parsed {
        let Some(manifest) = &item.manifest else {
            continue;
        };
        if let Some(name) = &manifest.package_name {
            declared.insert(
                (item.ecosystem, name.to_lowercase()),
                (name.clone(), item.path.to_owned()),
            );
        }
        if let Some(description) = &manifest.description
            && !description.trim().is_empty()
        {
            analysis
                .descriptions
                .push((item.path.to_owned(), description.trim().to_owned()));
        }
        if let Some(tool) = &manifest.workspace_tool {
            let candidate = WorkspaceInfo {
                tool: tool.clone(),
                manifest: item.path.to_owned(),
                members: manifest.workspace_members.clone(),
            };
            let replace = analysis
                .workspace
                .as_ref()
                .is_none_or(|current| paths::depth(item.path) < paths::depth(&current.manifest));
            if replace {
                analysis.workspace = Some(candidate);
            }
        }
        for (relative, kind) in &manifest.entrypoints {
            if let Some(path) = paths::join(paths::parent(item.path), relative) {
                analysis.entrypoints.push(DeclaredEntrypoint {
                    manifest: item.path.to_owned(),
                    path,
                    kind: *kind,
                });
            }
        }
        for (name, version) in &manifest.requirements {
            analysis.requirements.push(EnvironmentRequirement {
                kind: requirement_kind(name),
                name: name.clone(),
                version: Some(version.clone()),
                source: item.path.to_owned(),
            });
        }
    }

    // Direct dependencies, merged per ecosystem and name.
    let mut records: BTreeMap<(&str, String), DependencyRecord> = BTreeMap::new();
    for item in &parsed {
        let Some(manifest) = &item.manifest else {
            continue;
        };
        for dependency in &manifest.dependencies {
            let key = (item.ecosystem, dependency.name.to_lowercase());
            let internal = matches!(dependency.source.as_str(), "workspace" | "path")
                || declared.contains_key(&key);
            let record = records.entry(key).or_insert_with(|| DependencyRecord {
                name: dependency.name.clone(),
                ecosystem: item.ecosystem.to_owned(),
                requirement: dependency.requirement.clone(),
                scope: dependency.scope,
                manifests: Vec::new(),
                resolved: Vec::new(),
                source: dependency.source.clone(),
                internal,
            });
            if scope_rank(dependency.scope) < scope_rank(record.scope) {
                record.scope = dependency.scope;
            }
            record.internal |= internal;
            if record.requirement.is_none() {
                record.requirement = dependency.requirement.clone();
            }
            if !record.manifests.iter().any(|m| m == item.path) {
                record.manifests.push(item.path.to_owned());
            }
        }
    }

    // Lockfiles: resolved versions, consistency, and duplicates.
    let mut locked: BTreeSet<(&str, String, String)> = BTreeSet::new();
    for item in &parsed {
        let Some(lockfile) = &item.lockfile else {
            continue;
        };
        let directory = paths::parent(item.path);
        let mut versions_by_name: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (name, version) in &lockfile.packages {
            versions_by_name
                .entry(name.to_lowercase())
                .or_default()
                .insert(version.clone());
            locked.insert((item.ecosystem, name.to_lowercase(), version.clone()));
        }
        for record in records
            .values_mut()
            .filter(|r| r.ecosystem == item.ecosystem)
        {
            let covered = record
                .manifests
                .iter()
                .any(|manifest| paths::is_within(paths::parent(manifest), directory));
            if covered && let Some(versions) = versions_by_name.get(&record.name.to_lowercase()) {
                for version in versions {
                    if !record.resolved.contains(version) {
                        record.resolved.push(version.clone());
                    }
                }
            }
        }
        let missing: Vec<String> = records
            .values()
            .filter(|record| {
                record.ecosystem == item.ecosystem
                    && record.source == "registry"
                    && !record.internal
                    && record
                        .manifests
                        .iter()
                        .any(|manifest| paths::parent(manifest) == directory)
                    && !versions_by_name.contains_key(&record.name.to_lowercase())
            })
            .map(|record| record.name.clone())
            .collect();
        if let Some(lock_record) = analysis
            .report
            .lockfiles
            .iter_mut()
            .find(|l| l.path == item.path)
        {
            lock_record.missing_from_lock = missing;
        }
        for (name, versions) in versions_by_name.iter().filter(|(_, v)| v.len() > 1) {
            if analysis.report.duplicates.len() >= MAX_DUPLICATES {
                break;
            }
            let display = lockfile
                .packages
                .iter()
                .find(|(n, _)| n.to_lowercase() == *name)
                .map_or_else(|| name.clone(), |(n, _)| n.clone());
            analysis.report.duplicates.push(DuplicateVersions {
                ecosystem: item.ecosystem.to_owned(),
                name: display,
                versions: versions.iter().cloned().collect(),
                lockfile: item.path.to_owned(),
            });
        }
    }

    // Package boundaries with their internal dependencies.
    for ((ecosystem, _), (name, manifest)) in &declared {
        let internal_dependencies = parsed
            .iter()
            .filter(|item| item.path == manifest)
            .filter_map(|item| item.manifest.as_ref())
            .flat_map(|parsed| parsed.dependencies.iter())
            .filter(|dependency| {
                declared.contains_key(&(*ecosystem, dependency.name.to_lowercase()))
            })
            .map(|dependency| dependency.name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        analysis.packages.push(PackageBoundary {
            name: name.clone(),
            path: paths::parent(manifest).to_owned(),
            ecosystem: (*ecosystem).to_owned(),
            manifest: manifest.clone(),
            internal_dependencies,
        });
    }
    analysis
        .packages
        .sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.name.cmp(&b.name)));

    let mut dependencies: Vec<DependencyRecord> = records.into_values().collect();
    for record in &mut dependencies {
        record.resolved.sort();
    }
    let mut ecosystems: BTreeMap<&str, EcosystemSummary> = BTreeMap::new();
    for manifest in &analysis.report.manifests {
        let summary = ecosystems
            .entry(ecosystem_key(&manifest.ecosystem))
            .or_insert_with(|| EcosystemSummary {
                ecosystem: manifest.ecosystem.clone(),
                manifests: 0,
                lockfiles: 0,
                runtime: 0,
                other: 0,
                locked_packages: 0,
            });
        match manifest.kind {
            ManifestKind::Manifest => summary.manifests += 1,
            ManifestKind::Lockfile => summary.lockfiles += 1,
        }
    }
    for record in dependencies.iter().filter(|r| !r.internal) {
        if let Some(summary) = ecosystems.get_mut(ecosystem_key(&record.ecosystem)) {
            if record.scope == DependencyScope::Runtime {
                summary.runtime += 1;
            } else {
                summary.other += 1;
            }
        }
    }
    for (ecosystem, _, _) in &locked {
        if let Some(summary) = ecosystems.get_mut(ecosystem) {
            summary.locked_packages += 1;
        }
    }

    let report = &mut analysis.report;
    report.direct_count = dependencies.iter().filter(|r| !r.internal).count() as u64;
    report.locked_count = locked.len() as u64;
    report.dependencies = dependencies;
    report.ecosystems = ecosystems.into_values().collect();
    report.advisories = AdvisoryStatus {
        provider: None,
        checked_at: None,
        note: "No vulnerability advisory provider was used, so this report makes no claim about known vulnerabilities in these dependencies.".to_owned(),
    };
    report.status = if failures > 0 {
        report.notes.push(format!(
            "{failures} dependency file(s) could not be fully parsed; see the manifest list for details."
        ));
        SectionStatus::Partial
    } else {
        SectionStatus::Analyzed
    };
    if report.manifests.is_empty() {
        report
            .notes
            .push("No supported manifests or lockfiles were detected.".to_owned());
    }
    analysis
}

/// Maps an owned ecosystem name back to the static key used for grouping.
fn ecosystem_key(ecosystem: &str) -> &'static str {
    match ecosystem {
        "cargo" => "cargo",
        "npm" => "npm",
        "pypi" => "pypi",
        "go" => "go",
        "maven" => "maven",
        "gradle" => "gradle",
        "nuget" => "nuget",
        "composer" => "composer",
        "rubygems" => "rubygems",
        "pub" => "pub",
        "swiftpm" => "swiftpm",
        _ => "other",
    }
}

fn parse_one<'a>(
    provider: &dyn EcosystemProvider,
    kind: ManifestKind,
    file: &DependencyFile<'a>,
    parsed: &mut Vec<Parsed<'a>>,
) -> ManifestRecord {
    let ecosystem = provider.ecosystem();
    let mut record = ManifestRecord {
        path: file.path.to_owned(),
        ecosystem: ecosystem.to_owned(),
        kind,
        package_name: None,
        package_version: None,
        description: None,
        entries: 0,
        status: ParseStatus::Parsed,
        message: None,
    };
    match kind {
        ManifestKind::Manifest => {
            match provider.parse_manifest(file.path, file.content) {
                Ok(manifest) => {
                    record.package_name = manifest.package_name.clone();
                    record.package_version = manifest.package_version.clone();
                    record.description = manifest.description.clone();
                    record.entries = u32::try_from(manifest.dependencies.len()).unwrap_or(u32::MAX);
                    if manifest.partial {
                        record.status = ParseStatus::Partial;
                        record.message = Some("Some entries are computed or indirect and could not be read literally.".to_owned());
                    }
                    parsed.push(Parsed {
                        path: file.path,
                        ecosystem,
                        manifest: Some(manifest),
                        lockfile: None,
                    });
                }
                Err(message) => {
                    record.status = ParseStatus::Failed;
                    record.message = Some(message);
                }
            }
        }
        ManifestKind::Lockfile => match provider.parse_lockfile(file.path, file.content) {
            Ok(lockfile) => {
                record.entries = u32::try_from(lockfile.packages.len()).unwrap_or(u32::MAX);
                parsed.push(Parsed {
                    path: file.path,
                    ecosystem,
                    manifest: None,
                    lockfile: Some(lockfile),
                });
            }
            Err(message) => {
                record.status = ParseStatus::Failed;
                record.message = Some(message);
            }
        },
    }
    record
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file<'a>(path: &'a str, content: &'a str) -> DependencyFile<'a> {
        DependencyFile { path, content }
    }

    #[test]
    fn merges_workspace_manifests_and_lockfiles() {
        let root = "[workspace]\nmembers = [\"crates/*\"]\n";
        let core = "[package]\nname = \"core\"\nversion = \"0.1.0\"\ndescription = \"Core library\"\n[dependencies]\nserde = \"1\"\nregex = \"1\"\n";
        let cli = "[package]\nname = \"cli\"\nversion = \"0.1.0\"\n[dependencies]\ncore = { path = \"../core\" }\nserde = \"1\"\nclap = \"4\"\n[dev-dependencies]\nregex = \"1\"\n";
        let lock = "[[package]]\nname = \"serde\"\nversion = \"1.0.200\"\n[[package]]\nname = \"regex\"\nversion = \"1.10.0\"\n[[package]]\nname = \"regex\"\nversion = \"1.11.0\"\n[[package]]\nname = \"core\"\nversion = \"0.1.0\"\n";
        let analysis = analyze(&[
            file("Cargo.toml", root),
            file("crates/core/Cargo.toml", core),
            file("crates/cli/Cargo.toml", cli),
            file("Cargo.lock", lock),
            file("README.md", "ignored"),
        ]);
        let report = &analysis.report;
        assert_eq!(report.status, SectionStatus::Analyzed);
        assert_eq!(report.manifests.len(), 4);
        let names: Vec<_> = report
            .dependencies
            .iter()
            .map(|d| (d.name.as_str(), d.internal))
            .collect();
        assert_eq!(
            names,
            vec![
                ("clap", false),
                ("core", true),
                ("regex", false),
                ("serde", false)
            ]
        );
        let serde = report
            .dependencies
            .iter()
            .find(|d| d.name == "serde")
            .unwrap();
        assert_eq!(serde.manifests.len(), 2);
        assert_eq!(serde.resolved, vec!["1.0.200"]);
        let regex = report
            .dependencies
            .iter()
            .find(|d| d.name == "regex")
            .unwrap();
        assert_eq!(
            regex.scope,
            DependencyScope::Runtime,
            "runtime wins over dev"
        );
        assert_eq!(report.direct_count, 3);
        assert_eq!(report.duplicates.len(), 1);
        assert_eq!(report.duplicates[0].versions, vec!["1.10.0", "1.11.0"]);
        assert_eq!(analysis.workspace.as_ref().unwrap().tool, "cargo-workspace");
        let cli_package = analysis.packages.iter().find(|p| p.name == "cli").unwrap();
        assert_eq!(cli_package.path, "crates/cli");
        assert_eq!(cli_package.internal_dependencies, vec!["core"]);
        assert_eq!(
            analysis.descriptions,
            vec![(
                "crates/core/Cargo.toml".to_owned(),
                "Core library".to_owned()
            )]
        );
    }

    #[test]
    fn reports_lockfile_inconsistencies_and_failures() {
        let files = [
            file(
                "package.json",
                r#"{"name":"web","dependencies":{"react":"^19","lodash":"^4"}}"#,
            ),
            file(
                "package-lock.json",
                r#"{"packages":{"node_modules/react":{"version":"19.0.0"}}}"#,
            ),
            file("services/api/go.mod", "not a go module"),
        ];
        let analysis = analyze(&files);
        assert_eq!(analysis.report.status, SectionStatus::Partial);
        let lock = analysis
            .report
            .lockfiles
            .iter()
            .find(|l| l.path == "package-lock.json")
            .unwrap();
        assert_eq!(lock.missing_from_lock, vec!["lodash"]);
        assert_eq!(lock.packages, 1);
        let failed = analysis
            .report
            .manifests
            .iter()
            .find(|m| m.path == "services/api/go.mod")
            .unwrap();
        assert_eq!(failed.status, ParseStatus::Failed);
        assert!(failed.message.is_some());
        assert_eq!(analyze(&files[..2]).report.status, SectionStatus::Analyzed);
    }

    #[test]
    fn empty_input_is_explained() {
        let analysis = analyze(&[]);
        assert_eq!(analysis.report.status, SectionStatus::Analyzed);
        assert!(analysis.report.notes[0].contains("No supported manifests"));
        assert!(analysis.report.advisories.note.contains("no claim"));
    }
}
