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

use crate::model::{DeclaredDependency, EcosystemProvider, ParsedLockfile, ParsedManifest};
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

// Index actual workspace roots, including those without shared declarations.
// An empty inner table is a boundary: do not fall back to another workspace.
type SharedDependencies<'a> =
    BTreeMap<&'a str, BTreeMap<&'a str, BTreeMap<String, &'a DeclaredDependency>>>;

fn shared_dependencies<'a>(parsed: &'a [Parsed<'_>]) -> SharedDependencies<'a> {
    let mut index = SharedDependencies::new();
    for item in parsed {
        let Some(manifest) = &item.manifest else {
            continue;
        };
        if manifest.workspace_tool.is_none() {
            continue;
        }
        let entries = index
            .entry(item.ecosystem)
            .or_default()
            .entry(paths::parent(item.path))
            .or_default();
        for dependency in &manifest.shared_dependencies {
            entries
                .entry(dependency.name.to_lowercase())
                .or_insert(dependency);
        }
    }
    index
}

// Cargo's explicit package.workspace can point sideways or upwards within
// the repository. Reject absolute paths and escapes; do not inspect the host
// filesystem or silently fall back to another root when an override is invalid.
fn explicit_workspace_directory(base: &str, root: &str) -> Option<String> {
    let bytes = root.as_bytes();
    if root.trim().is_empty()
        || root.starts_with(['/', '\\'])
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
    {
        return None;
    }
    paths::join(base, root)
}

// Lexical approximation of Cargo's workspace lookup. The nearest discovered
// enclosing workspace wins unless package.workspace explicitly selects one.
// This does not evaluate membership globs, excludes, or Cargo metadata.
fn inherited_dependency<'a>(
    index: &SharedDependencies<'a>,
    item: &Parsed<'_>,
    manifest: &ParsedManifest,
    name: &str,
) -> Option<&'a DeclaredDependency> {
    let workspaces = index.get(item.ecosystem)?;
    let mut directory = paths::parent(item.path);
    if let Some(root) = &manifest.workspace_root {
        let explicit = explicit_workspace_directory(directory, root)?;
        return workspaces.get(explicit.as_str())?.get(name).copied();
    }
    loop {
        if let Some(entries) = workspaces.get(directory) {
            return entries.get(name).copied();
        }
        if directory.is_empty() {
            return None;
        }
        directory = paths::parent(directory);
    }
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

    let shared = shared_dependencies(&parsed);

    // Direct dependencies, merged per ecosystem and name.
    let mut records: BTreeMap<(&str, String), DependencyRecord> = BTreeMap::new();
    for item in &parsed {
        let Some(manifest) = &item.manifest else {
            continue;
        };
        for declared_dependency in &manifest.dependencies {
            let key = (item.ecosystem, declared_dependency.name.to_lowercase());
            let resolved;
            let dependency = if declared_dependency.source == "inherited" {
                resolved = match inherited_dependency(&shared, item, manifest, &key.1) {
                    Some(template) => DeclaredDependency {
                        name: declared_dependency.name.clone(),
                        requirement: declared_dependency
                            .requirement
                            .clone()
                            .or_else(|| template.requirement.clone()),
                        scope: declared_dependency.scope,
                        source: template.source.clone(),
                    },
                    None => declared_dependency.clone(),
                };
                &resolved
            } else {
                declared_dependency
            };
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
            // Parsed manifests are in ascending path order.
            if record.manifests.last().is_none_or(|m| m != item.path) {
                record.manifests.push(item.path.to_owned());
            }
        }
    }

    // Index direct external registry declarations by their exact sibling
    // directory. Missing-lock diagnostics need only this small subset,
    // not every dependency in the repository for each lockfile.
    type SiblingIndex<'a> = BTreeMap<(&'a str, &'a str), Vec<(&'a str, &'a str)>>;
    let mut siblings = SiblingIndex::new();
    for ((ecosystem, key), record) in &records {
        if record.source != "registry" || record.internal {
            continue;
        }
        for manifest in &record.manifests {
            let entries = siblings
                .entry((*ecosystem, paths::parent(manifest)))
                .or_default();
            let pair = (key.as_str(), record.name.as_str());
            if entries.last() != Some(&pair) {
                entries.push(pair);
            }
        }
    }

    // Keep resolved versions separately until the borrowed sibling index
    // is no longer needed. Ordered sets avoid repeated linear version
    // membership checks and produce the same sorted report.
    let mut resolved_versions: BTreeMap<(&str, &str), BTreeSet<String>> = BTreeMap::new();
    let mut locked: BTreeSet<(&str, String, String)> = BTreeSet::new();
    // Reverse insertion preserves the original first-record lookup
    // even if an input path appears more than once.
    let lock_indices: BTreeMap<String, usize> = analysis
        .report
        .lockfiles
        .iter()
        .enumerate()
        .rev()
        .map(|(index, lock)| (lock.path.clone(), index))
        .collect();
    for item in &parsed {
        let Some(lockfile) = &item.lockfile else {
            continue;
        };
        let directory = paths::parent(item.path);
        let mut versions_by_name: BTreeMap<String, (&str, BTreeSet<String>)> = BTreeMap::new();
        for (name, version) in &lockfile.packages {
            let normalized = name.to_lowercase();
            versions_by_name
                .entry(normalized.clone())
                .or_insert_with(|| (name.as_str(), BTreeSet::new()))
                .1
                .insert(version.clone());
            locked.insert((item.ecosystem, normalized, version.clone()));
        }
        // Probe only names actually present in this lockfile. Preserve
        // the existing directory-containment rule for resolved versions.
        for (name, (_, versions)) in &versions_by_name {
            let Some((key, record)) = records.get_key_value(&(item.ecosystem, name.clone())) else {
                continue;
            };
            if record
                .manifests
                .iter()
                .any(|manifest| paths::is_within(paths::parent(manifest), directory))
            {
                resolved_versions
                    .entry((key.0, key.1.as_str()))
                    .or_default()
                    .extend(versions.iter().cloned());
            }
        }
        let missing = siblings
            .get(&(item.ecosystem, directory))
            .into_iter()
            .flatten()
            .filter(|(key, _)| !versions_by_name.contains_key(*key))
            .map(|(_, display)| (*display).to_owned())
            .collect();
        if let Some(&index) = lock_indices.get(item.path) {
            analysis.report.lockfiles[index].missing_from_lock = missing;
        }
        for (display, versions) in versions_by_name.values().filter(|(_, v)| v.len() > 1) {
            if analysis.report.duplicates.len() >= MAX_DUPLICATES {
                break;
            }
            analysis.report.duplicates.push(DuplicateVersions {
                ecosystem: item.ecosystem.to_owned(),
                name: (*display).to_owned(),
                versions: versions.iter().cloned().collect(),
                lockfile: item.path.to_owned(),
            });
        }
    }
    let resolved_versions: BTreeMap<(&str, String), BTreeSet<String>> = resolved_versions
        .into_iter()
        .map(|((ecosystem, name), versions)| ((ecosystem, name.to_owned()), versions))
        .collect();
    for (key, versions) in resolved_versions {
        if let Some(record) = records.get_mut(&key) {
            record.resolved = versions.into_iter().collect();
        }
    }

    // Package boundaries with their internal dependencies. Index parsed
    // manifests once, preserving multiple entries for the same path.
    let mut manifests_by_path: BTreeMap<&str, Vec<&ParsedManifest>> = BTreeMap::new();
    for item in &parsed {
        if let Some(manifest) = &item.manifest {
            manifests_by_path.entry(item.path).or_default().push(manifest);
        }
    }
    for ((ecosystem, _), (name, manifest)) in &declared {
        let internal_dependencies = manifests_by_path
            .get(manifest.as_str())
            .into_iter()
            .flatten()
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
    fn resolves_dependencies_inherited_from_the_workspace() {
        let root = "[workspace]\nmembers = [\"crates/*\"]\n[workspace.dependencies]\nserde = { version = \"1.0\", features = [\"derive\"] }\ncore = { path = \"crates/core\" }\n";
        let core = "[package]\nname = \"core\"\n[dependencies]\nserde.workspace = true\n";
        let app = "[package]\nname = \"app\"\n[dependencies]\ncore.workspace = true\nserde = { workspace = true, optional = true }\nunknown.workspace = true\n";
        let analysis = analyze(&[
            file("Cargo.toml", root),
            file("crates/core/Cargo.toml", core),
            file("crates/app/Cargo.toml", app),
        ]);
        let find = |name: &str| {
            analysis
                .report
                .dependencies
                .iter()
                .find(|d| d.name == name)
                .unwrap()
        };
        let serde = find("serde");
        assert!(!serde.internal);
        assert_eq!(serde.requirement.as_deref(), Some("1.0"));
        assert_eq!(serde.source, "registry");
        assert_eq!(serde.manifests.len(), 2);
        assert!(find("core").internal);
        assert!(!find("unknown").internal);
        assert_eq!(analysis.report.direct_count, 2);
    }

    fn inherited_record<'a>(analysis: &'a DependencyAnalysis, name: &str) -> &'a DependencyRecord {
        analysis
            .report
            .dependencies
            .iter()
            .find(|dependency| dependency.name == name)
            .expect("expected declared dependency")
    }

    #[test]
    fn inherited_dependencies_never_use_a_sibling_workspace() {
        let first = "[workspace]\n[workspace.dependencies]\nserde = \"1\"\n";
        let second = "[workspace]\n[workspace.dependencies]\nserde = \"2\"\n";
        let member = "[package]\nname = \"member\"\n[dependencies]\nserde.workspace = true\n";
        let analysis = analyze(&[
            file("first/Cargo.toml", first),
            file("second/Cargo.toml", second),
            file("second/member/Cargo.toml", member),
        ]);
        let dependency = inherited_record(&analysis, "serde");
        assert_eq!(dependency.requirement.as_deref(), Some("2"));
        assert_eq!(dependency.source, "registry");
        assert_eq!(dependency.manifests, vec!["second/member/Cargo.toml"]);
    }

    #[test]
    fn nearest_workspace_is_a_boundary_even_when_it_lacks_the_dependency() {
        let outer = "[workspace]\n[workspace.dependencies]\nserde = \"1\"\nregex = \"1\"\n";
        let inner = "[workspace]\n[workspace.dependencies]\nserde = \"2\"\n";
        let empty = "[workspace]\n";
        let member = "[package]\nname = \"member\"\n[dependencies]\nserde.workspace = true\nregex.workspace = true\n";
        let analysis = analyze(&[
            file("Cargo.toml", outer),
            file("nested/Cargo.toml", inner),
            file("nested/member/Cargo.toml", member),
        ]);
        assert_eq!(
            inherited_record(&analysis, "serde").requirement.as_deref(),
            Some("2")
        );
        let missing = inherited_record(&analysis, "regex");
        assert_eq!(missing.requirement, None);
        assert_eq!(missing.source, "inherited");

        let analysis = analyze(&[
            file("Cargo.toml", outer),
            file("nested/Cargo.toml", empty),
            file("nested/member/Cargo.toml", member),
        ]);
        assert_eq!(inherited_record(&analysis, "serde").source, "inherited");
        assert_eq!(inherited_record(&analysis, "regex").source, "inherited");
    }

    #[test]
    fn explicit_workspace_override_can_select_a_different_discovered_root() {
        let outer = "[workspace]\n[workspace.dependencies]\nserde = \"1\"\n";
        let selected = "[workspace]\n[workspace.dependencies]\nserde = \"3\"\n";
        let member = "[package]\nname = \"member\"\nworkspace = \"../../shared\"\n[dependencies]\nserde.workspace = true\n";
        let analysis = analyze(&[
            file("Cargo.toml", outer),
            file("shared/Cargo.toml", selected),
            file("crates/member/Cargo.toml", member),
        ]);
        let dependency = inherited_record(&analysis, "serde");
        assert_eq!(dependency.requirement.as_deref(), Some("3"));
        assert_eq!(dependency.source, "registry");
    }

    #[test]
    fn invalid_explicit_workspace_never_falls_back_to_an_ancestor() {
        let outer = "[workspace]\n[workspace.dependencies]\nserde = \"1\"\n";
        for root in ["../missing", "../../outside", "/outside", "C:/outside", ""] {
            let member = format!(
                "[package]\nname = \"member\"\nworkspace = \"{root}\"\n[dependencies]\nserde.workspace = true\n"
            );
            let analysis = analyze(&[
                file("Cargo.toml", outer),
                file("member/Cargo.toml", &member),
            ]);
            let dependency = inherited_record(&analysis, "serde");
            assert_eq!(dependency.requirement, None, "{root}");
            assert_eq!(dependency.source, "inherited", "{root}");
        }
        assert!(explicit_workspace_directory("member", "\\outside").is_none());
        assert_eq!(
            explicit_workspace_directory("crates/member", "../../shared").as_deref(),
            Some("shared")
        );
    }

    #[test]
    fn unrelated_workspace_is_not_a_fallback_for_an_orphan_package() {
        let root = "[workspace]\n[workspace.dependencies]\nserde = \"1\"\n";
        let member = "[package]\nname = \"member\"\n[dependencies]\nserde.workspace = true\n";
        let analysis = analyze(&[
            file("workspace/Cargo.toml", root),
            file("workspace-extra/member/Cargo.toml", member),
        ]);
        let dependency = inherited_record(&analysis, "serde");
        assert_eq!(dependency.requirement, None);
        assert_eq!(dependency.source, "inherited");
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
    fn indexed_lockfile_join_preserves_directory_boundaries_and_version_union() {
        let files = [
            file(
                "apps/web/package.json",
                r#"{"name":"web","dependencies":{"Shared":"^1","local-only":"1","missing":"1","git-only":"git+https://example.test/a.git"}}"#,
            ),
            file(
                "apps/web/package-lock.json",
                r#"{"packages":{"node_modules/shared":{"version":"1.1.0"},"node_modules/local-only":{"version":"1.0.0"},"node_modules/a/node_modules/shared":{"version":"1.2.0"}}}"#,
            ),
            file(
                "apps/web-admin/package.json",
                r#"{"name":"admin","dependencies":{"shared":"2","admin-only":"1"}}"#,
            ),
            file(
                "apps/web-admin/package-lock.json",
                r#"{"packages":{"node_modules/shared":{"version":"2.0.0"},"node_modules/admin-only":{"version":"1.0.0"},"node_modules/missing":{"version":"99.0.0"}}}"#,
            ),
            file(
                "apps/package-lock.json",
                r#"{"packages":{"node_modules/shared":{"version":"1.0.0"},"node_modules/local-only":{"version":"1.0.0"}}}"#,
            ),
            file(
                "unrelated/package-lock.json",
                r#"{"packages":{"node_modules/shared":{"version":"99.0.0"}}}"#,
            ),
        ];
        let report = analyze(&files).report;
        let dependency = |name: &str| {
            report
                .dependencies
                .iter()
                .find(|d| d.name.eq_ignore_ascii_case(name))
                .unwrap()
        };
        assert_eq!(
            dependency("shared").resolved,
            vec!["1.0.0", "1.1.0", "1.2.0", "2.0.0"]
        );
        assert_eq!(dependency("local-only").resolved, vec!["1.0.0"]);
        assert!(dependency("missing").resolved.is_empty());
        let lock = |path: &str| report.lockfiles.iter().find(|l| l.path == path).unwrap();
        assert_eq!(
            lock("apps/web/package-lock.json").missing_from_lock,
            vec!["missing"]
        );
        assert!(lock("apps/web-admin/package-lock.json").missing_from_lock.is_empty());
        assert!(lock("apps/package-lock.json").missing_from_lock.is_empty());
        assert_eq!(report.duplicates.len(), 1);
        assert_eq!(report.duplicates[0].versions, vec!["1.1.0", "1.2.0"]);
        let mut reversed = files;
        reversed.reverse();
        assert_eq!(report, analyze(&reversed).report);
    }

    #[test]
    fn indexed_lockfile_join_isolated_by_ecosystem_and_exact_sibling_directory() {
        let files = [
            file(
                "package.json",
                r#"{"name":"frontend","dependencies":{"serde":"1","z":"1","a":"1"}}"#,
            ),
            file("package-lock.json", r#"{"packages":{}}"#),
            file(
                "Cargo.toml",
                "[package]\nname = \"backend\"\n[dependencies]\nserde = \"1\"\n",
            ),
            file(
                "Cargo.lock",
                "[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\n",
            ),
            file(
                "nested/package.json",
                r#"{"name":"nested","dependencies":{"nested-only":"1"}}"#,
            ),
        ];
        let report = analyze(&files).report;
        let npm = report
            .dependencies
            .iter()
            .find(|d| d.ecosystem == "npm" && d.name == "serde")
            .unwrap();
        let cargo = report
            .dependencies
            .iter()
            .find(|d| d.ecosystem == "cargo" && d.name == "serde")
            .unwrap();
        assert!(npm.resolved.is_empty());
        assert_eq!(cargo.resolved, vec!["1.0.0"]);
        let lock = report
            .lockfiles
            .iter()
            .find(|l| l.path == "package-lock.json")
            .unwrap();
        assert_eq!(lock.missing_from_lock, vec!["a", "serde", "z"]);
    }

    #[test]
    fn indexed_joins_handle_many_disjoint_packages_without_cross_attribution() {
        let mut owned = Vec::new();
        for index in 0..256 {
            let dir = format!("packages/p{index:03}");
            let name = format!("dep{index:03}");
            owned.push((
                format!("{dir}/package.json"),
                format!(r#"{{"name":"p{index:03}","dependencies":{{"{name}":"1","common":"1"}}}}"#),
            ));
            owned.push((
                format!("{dir}/package-lock.json"),
                format!(r#"{{"packages":{{"node_modules/{name}":{{"version":"1.0.0"}},"node_modules/common":{{"version":"1.0.{index}"}}}}}}"#),
            ));
        }
        let files: Vec<_> = owned.iter().map(|(p, c)| file(p, c)).collect();
        let result = analyze(&files);
        assert_eq!(result.packages.len(), 256);
        assert_eq!(result.report.direct_count, 257);
        assert!(
            result
                .report
                .lockfiles
                .iter()
                .all(|l| l.missing_from_lock.is_empty())
        );
        for dependency in &result.report.dependencies {
            if dependency.name == "common" {
                assert_eq!(dependency.manifests.len(), 256);
                assert_eq!(dependency.resolved.len(), 256);
            } else {
                assert_eq!(dependency.manifests.len(), 1);
                assert_eq!(dependency.resolved, vec!["1.0.0"]);
            }
        }
    }

    #[test]
    fn indexed_lockfile_joins_respect_inherited_workspace_source_and_scope() {
        let files = [
            file(
                "alpha/Cargo.toml",
                "[workspace]\n[workspace.dependencies]\nserde = \"1\"\n",
            ),
            file(
                "beta/Cargo.toml",
                "[workspace]\n[workspace.dependencies]\nserde = \"2\"\nlocal = { path = \"local\" }\n",
            ),
            file(
                "beta/member/Cargo.toml",
                "[package]\nname = \"member\"\n[dependencies]\nserde.workspace = true\nlocal.workspace = true\n",
            ),
            file("alpha/Cargo.lock", "[[package]]\nname = \"serde\"\nversion = \"1.9.0\"\n"),
            file("beta/Cargo.lock", "[[package]]\nname = \"serde\"\nversion = \"2.1.0\"\n"),
            file("beta/member/Cargo.lock", "[[package]]\nname = \"serde\"\nversion = \"2.2.0\"\n"),
        ];
        let report = analyze(&files).report;
        let serde = report
            .dependencies
            .iter()
            .find(|d| d.name == "serde")
            .unwrap();
        assert_eq!(serde.requirement.as_deref(), Some("2"));
        assert_eq!(serde.resolved, vec!["2.1.0", "2.2.0"]);
        assert_eq!(serde.source, "registry");
        let local = report
            .dependencies
            .iter()
            .find(|d| d.name == "local")
            .unwrap();
        assert!(local.internal);
        assert_eq!(local.source, "path");
        assert!(
            report
                .lockfiles
                .iter()
                .all(|lock| lock.missing_from_lock.is_empty())
        );
        let mut reverse = files;
        reverse.reverse();
        assert_eq!(report, analyze(&reverse).report);
    }

    #[test]
    fn empty_input_is_explained() {
        let analysis = analyze(&[]);
        assert_eq!(analysis.report.status, SectionStatus::Analyzed);
        assert!(analysis.report.notes[0].contains("No supported manifests"));
        assert!(analysis.report.advisories.note.contains("no claim"));
    }
}
