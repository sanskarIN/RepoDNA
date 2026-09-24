//! The architecture analysis entry point: from files and packages to a report.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use repodna_core::cancel::{CancellationToken, Cancelled};
use repodna_core::confidence::Confidence;
use repodna_core::evidence::{Evidence, format_number};
use repodna_core::finding::Finding;
use repodna_core::hash::stable_id;
use repodna_core::metric::round4;
use repodna_core::model::SectionStatus;
use repodna_core::model::architecture::{
    ArchitectureReport, CycleLevel, DependencyCycle, EdgeKind, ExternalImport, FileEdge, Layer,
    ModuleEdge, ModuleKind, ModuleRecord, PackageBoundary, WorkspaceInfo,
};
use repodna_core::model::languages::LanguageInteraction;
use repodna_core::model::structure::Entrypoint;
use repodna_dependencies::DeclaredEntrypoint;
use repodna_parser::spec::ImportKind;

use crate::SourceFile;
use crate::entrypoints::detect_entrypoints;
use crate::findings::{architecture_findings, classify_style};
use crate::graph::{Graph, betweenness, layers, shortest_cycle, strongly_connected_components};
use crate::interactions::{CrossEdge, detect_interactions};
use crate::modules::infer_modules;
use crate::resolve::{Resolution, Resolver};

/// Maximum file-level cycles recorded in the report.
pub const MAX_FILE_CYCLES: usize = 50;

/// Maximum external packages recorded in the report.
pub const MAX_EXTERNAL: usize = 500;

/// Maximum sample evidence items per module edge or external package.
const MAX_SAMPLES: usize = 3;

/// Maximum external dependency names listed per module.
const MAX_MODULE_EXTERNALS: usize = 50;

/// Maximum characters of an import specifier stored on a file edge.
const MAX_SPECIFIER_CHARS: usize = 120;

/// Languages in which import cycles between files can fail at runtime (load order), so
/// file-level cycles are reported for them.
const FILE_CYCLE_LANGUAGES: &[&str] = &[
    "c",
    "cpp",
    "javascript",
    "lua",
    "objective-c",
    "perl",
    "php",
    "python",
    "ruby",
    "shell",
    "svelte",
    "typescript",
    "vue",
];

const METHOD: &str = "Modules come from package manifests and directory conventions. Imports are \
extracted lexically and resolved per language (relative paths, module systems, workspace \
packages, aliases). Module edges aggregate file edges; imports of whole packages or namespaces \
(Go packages, C# namespaces, Swift modules, JVM wildcards) link modules only. Cycles are \
strongly connected components, layers come from the condensed module graph, and centrality is \
normalized betweenness. File-level cycles are reported only for languages where import order \
matters at runtime.";

/// Inputs to architecture analysis.
#[derive(Debug, Clone, Copy)]
pub struct ArchitectureInput<'a> {
    /// Files with their analysis. First-party files form modules; all files can be targets.
    pub files: &'a [SourceFile<'a>],
    /// Packages declared by manifests.
    pub packages: &'a [PackageBoundary],
    /// Workspace configuration, when declared.
    pub workspace: Option<&'a WorkspaceInfo>,
    /// Entrypoints declared by manifests.
    pub declared_entrypoints: &'a [DeclaredEntrypoint],
    /// Declared dependencies as `(ecosystem, name)`, used to classify ambiguous imports.
    pub declared_dependencies: &'a [(String, String)],
    /// Maximum file edges stored in the report.
    pub max_file_edges: usize,
}

/// Results of architecture analysis.
#[derive(Debug, Clone, Default)]
pub struct ArchitectureOutput {
    /// The architecture section.
    pub report: ArchitectureReport,
    /// Module identifier of each input file (`None` for files outside modules).
    pub module_of: Vec<Option<String>>,
    /// Number of distinct first-party files that import each input file.
    pub file_dependents: Vec<u32>,
    /// `true` for files included by a module declaration such as Rust's `mod name;`.
    pub module_declared: Vec<bool>,
    /// Cross-language interactions.
    pub interactions: Vec<LanguageInteraction>,
    /// Entrypoints.
    pub entrypoints: Vec<Entrypoint>,
    /// Architecture findings.
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone)]
struct EdgeData {
    line: u32,
    kind: EdgeKind,
    confidence: Confidence,
    specifier: String,
}

#[derive(Debug, Default)]
struct ModuleEdgeData {
    weight: u32,
    confidence: Confidence,
    samples: Vec<Evidence>,
}

#[derive(Debug, Default)]
struct ExternalData {
    ecosystem: &'static str,
    importers: BTreeSet<usize>,
    modules: BTreeSet<usize>,
    samples: Vec<Evidence>,
}

fn edge_kind(kind: ImportKind) -> EdgeKind {
    match kind {
        ImportKind::Import => EdgeKind::Import,
        ImportKind::Include | ImportKind::SystemInclude => EdgeKind::Include,
        ImportKind::Module => EdgeKind::Module,
        ImportKind::Reference => EdgeKind::Reference,
    }
}

fn mechanism(kind: ImportKind) -> &'static str {
    match kind {
        ImportKind::Import => "import",
        ImportKind::Include | ImportKind::SystemInclude => "include",
        ImportKind::Module => "module",
        ImportKind::Reference => "markup-reference",
    }
}

fn truncate(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((index, _)) => format!("{}…", &text[..index]),
        None => text.to_owned(),
    }
}

/// Accumulates edges while imports are resolved.
struct Builder<'a> {
    files: &'a [SourceFile<'a>],
    module_index: Vec<Option<usize>>,
    file_edges: BTreeMap<(usize, usize), EdgeData>,
    module_edges: BTreeMap<(usize, usize), ModuleEdgeData>,
    externals: BTreeMap<String, ExternalData>,
    cross_edges: Vec<CrossEdge>,
}

impl Builder<'_> {
    #[allow(clippy::too_many_arguments)]
    fn add_edge(
        &mut self,
        from: usize,
        to: usize,
        kind: EdgeKind,
        confidence: Confidence,
        package_level: bool,
        line: u32,
        specifier: &str,
        mechanism: &'static str,
    ) {
        let files = self.files;
        if !package_level
            && let (Some(a), Some(b)) = (files[from].language, files[to].language)
            && a != b
        {
            self.cross_edges.push(CrossEdge {
                from,
                to,
                line,
                mechanism,
                confidence,
            });
        }
        let (Some(from_module), Some(to_module)) = (self.module_index[from], self.module_index[to])
        else {
            return;
        };
        if !package_level {
            let entry = self
                .file_edges
                .entry((from, to))
                .or_insert_with(|| EdgeData {
                    line,
                    kind,
                    confidence,
                    specifier: truncate(specifier, MAX_SPECIFIER_CHARS),
                });
            entry.confidence = entry.confidence.max(confidence);
        }
        if kind != EdgeKind::Module && from_module != to_module {
            let entry = self
                .module_edges
                .entry((from_module, to_module))
                .or_default();
            entry.weight = entry.weight.saturating_add(1);
            entry.confidence = entry.confidence.max(confidence);
            if entry.samples.len() < MAX_SAMPLES {
                entry.samples.push(Evidence::edge_at(
                    files[from].path,
                    files[to].path,
                    files[from].path,
                    Some(line),
                ));
            }
        }
    }
}

/// Runs architecture analysis.
pub fn analyze(
    input: &ArchitectureInput<'_>,
    cancel: &CancellationToken,
) -> Result<ArchitectureOutput, Cancelled> {
    let files = input.files;
    let members: Vec<usize> = (0..files.len())
        .filter(|&index| files[index].first_party)
        .collect();
    let mut output = ArchitectureOutput {
        module_of: vec![None; files.len()],
        file_dependents: vec![0; files.len()],
        module_declared: vec![false; files.len()],
        ..ArchitectureOutput::default()
    };
    let mut report = ArchitectureReport {
        packages: input.packages.to_vec(),
        workspace: input.workspace.cloned(),
        method: METHOD.to_owned(),
        ..ArchitectureReport::default()
    };
    output.entrypoints = detect_entrypoints(files, input.declared_entrypoints);
    if members.is_empty() {
        report.status = SectionStatus::Unavailable;
        report.style = "Unknown".to_owned();
        report
            .notes
            .push("No first-party code files were found.".to_owned());
        output.report = report;
        return Ok(output);
    }

    // Modules.
    let member_paths: Vec<&str> = members.iter().map(|&index| files[index].path).collect();
    let map = infer_modules(&member_paths, input.packages);
    let mut module_index = vec![None; files.len()];
    for (position, &file) in members.iter().enumerate() {
        module_index[file] = Some(map.assignment[position]);
        output.module_of[file] = Some(map.modules[map.assignment[position]].id.clone());
    }

    // Imports.
    let mut resolver = Resolver::new(files, input.packages);
    for (ecosystem, name) in input.declared_dependencies {
        resolver.declare_dependency(ecosystem, name);
    }
    let mut builder = Builder {
        files,
        module_index,
        file_edges: BTreeMap::new(),
        module_edges: BTreeMap::new(),
        externals: BTreeMap::new(),
        cross_edges: Vec::new(),
    };
    for (position, &from) in members.iter().enumerate() {
        if position % 256 == 0 {
            cancel.check()?;
        }
        let Some(analysis) = files[from].analysis else {
            continue;
        };
        for import in &analysis.imports {
            match resolver.resolve(from, import) {
                Resolution::Internal {
                    targets,
                    confidence,
                    package_level,
                } => {
                    report.resolved_imports += 1;
                    for to in targets.into_iter().filter(|&to| to != from) {
                        builder.add_edge(
                            from,
                            to,
                            edge_kind(import.kind),
                            confidence,
                            package_level,
                            import.line,
                            &import.specifier,
                            mechanism(import.kind),
                        );
                    }
                }
                Resolution::External { name, ecosystem } => {
                    let entry = builder.externals.entry(name).or_default();
                    entry.ecosystem = ecosystem;
                    entry.importers.insert(from);
                    if let Some(module) = builder.module_index[from] {
                        entry.modules.insert(module);
                    }
                    if entry.samples.len() < MAX_SAMPLES {
                        entry.samples.push(
                            Evidence::line(files[from].path, import.line)
                                .with_note(truncate(&import.specifier, MAX_SPECIFIER_CHARS)),
                        );
                    }
                }
                Resolution::Ignored => {}
                Resolution::Unresolved => report.unresolved_imports += 1,
            }
        }
        for reference in &analysis.references {
            if let Some((to, confidence)) = resolver.resolve_reference(from, &reference.path)
                && to != from
            {
                builder.add_edge(
                    from,
                    to,
                    EdgeKind::Reference,
                    confidence,
                    false,
                    reference.line,
                    &reference.path,
                    "file-reference",
                );
            }
        }
    }
    cancel.check()?;

    // File edges, dependents, and file-level cycles.
    let mut file_edge_list: Vec<(usize, usize, &EdgeData)> = builder
        .file_edges
        .iter()
        .map(|(&(from, to), data)| (from, to, data))
        .collect();
    file_edge_list.sort_by(|a, b| {
        files[a.0]
            .path
            .cmp(files[b.0].path)
            .then_with(|| files[a.1].path.cmp(files[b.1].path))
    });
    for &(_, to, data) in &file_edge_list {
        if data.kind == EdgeKind::Module {
            output.module_declared[to] = true;
        } else {
            output.file_dependents[to] = output.file_dependents[to].saturating_add(1);
        }
    }
    report.file_edges_truncated = file_edge_list.len() > input.max_file_edges;
    if report.file_edges_truncated {
        report.notes.push(format!(
            "{} file edges were found; the report keeps the first {}.",
            file_edge_list.len(),
            input.max_file_edges
        ));
    }
    report.file_edges = file_edge_list
        .iter()
        .take(input.max_file_edges)
        .map(|&(from, to, data)| FileEdge {
            from: files[from].path.to_owned(),
            to: files[to].path.to_owned(),
            kind: data.kind,
            line: data.line,
            confidence: data.confidence,
            specifier: Some(data.specifier.clone()),
        })
        .collect();
    report
        .cycles
        .extend(file_cycles(files, &members, &builder.file_edges));

    // Module graph.
    let modules = &map.modules;
    let module_count = modules.len();
    let mut graph = Graph::new(module_count);
    for &(from, to) in builder.module_edges.keys() {
        graph.add_edge(from, to);
    }
    graph.finish();
    let components = strongly_connected_components(&graph);
    let layer_of = layers(&graph, &components);
    let centrality = betweenness(&graph);
    let mut fan_in = vec![0u32; module_count];
    let mut fan_out = vec![0u32; module_count];
    for &(from, to) in builder.module_edges.keys() {
        fan_out[from] += 1;
        fan_in[to] += 1;
    }

    // Per-module statistics.
    let mut files_per_module = vec![0u64; module_count];
    let mut code_per_module = vec![0u64; module_count];
    let mut language_lines: Vec<BTreeMap<&str, u64>> = vec![BTreeMap::new(); module_count];
    for &file in &members {
        let Some(module) = builder.module_index[file] else {
            continue;
        };
        files_per_module[module] += 1;
        code_per_module[module] += files[file].code_lines;
        if let Some(language) = files[file].language {
            *language_lines[module].entry(language).or_default() += files[file].code_lines;
        }
    }
    let total_code: u64 = code_per_module.iter().sum();
    let mut externals_per_module: Vec<BTreeSet<&str>> = vec![BTreeSet::new(); module_count];
    for (name, data) in &builder.externals {
        for &module in &data.modules {
            externals_per_module[module].insert(name.as_str());
        }
    }

    report.modules = modules
        .iter()
        .enumerate()
        .map(|(index, module)| {
            let language = language_lines[index]
                .iter()
                .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
                .map(|(language, _)| (*language).to_owned());
            let degree = fan_in[index] + fan_out[index];
            ModuleRecord {
                id: module.id.clone(),
                name: module.name.clone(),
                path: module.path.clone(),
                kind: module.kind,
                language,
                files: files_per_module[index],
                code_lines: code_per_module[index],
                fan_in: fan_in[index],
                fan_out: fan_out[index],
                instability: if degree == 0 {
                    0.0
                } else {
                    round4(f64::from(fan_out[index]) / f64::from(degree))
                },
                centrality: round4(centrality[index]),
                layer: Some(layer_of[index]),
                inferred: module.kind != ModuleKind::Package,
                confidence: module.confidence,
                importance: Vec::new(),
                external_dependencies: externals_per_module[index]
                    .iter()
                    .take(MAX_MODULE_EXTERNALS)
                    .map(|name| (*name).to_owned())
                    .collect(),
                evidence: module.evidence.clone(),
            }
        })
        .collect();

    report.module_edges = builder
        .module_edges
        .iter()
        .map(|(&(from, to), data)| ModuleEdge {
            from: modules[from].id.clone(),
            to: modules[to].id.clone(),
            weight: data.weight,
            confidence: data.confidence,
            samples: data.samples.clone(),
        })
        .collect();

    // Module cycles.
    for component in components.iter().filter(|component| component.len() > 1) {
        let members: Vec<String> = component
            .iter()
            .map(|&module| modules[module].id.clone())
            .collect();
        let path = shortest_cycle(&graph, component, 10).unwrap_or_default();
        let mut confidence = Confidence::High;
        let mut evidence = Vec::new();
        for pair in path.windows(2) {
            if let Some(edge) = builder.module_edges.get(&(pair[0], pair[1])) {
                confidence = confidence.weakest(edge.confidence);
                evidence.push(edge.samples.first().cloned().unwrap_or_else(|| {
                    Evidence::edge(modules[pair[0]].id.clone(), modules[pair[1]].id.clone())
                }));
            }
        }
        let languages: BTreeSet<String> = component
            .iter()
            .filter_map(|&module| report.modules[module].language.clone())
            .collect();
        let mut id_parts = vec!["module".to_owned()];
        id_parts.extend(members.iter().cloned());
        report.cycles.push(DependencyCycle {
            id: stable_id(&id_parts),
            level: CycleLevel::Module,
            members,
            path: path
                .iter()
                .map(|&module| modules[module].id.clone())
                .collect(),
            languages: languages.into_iter().collect(),
            confidence,
            evidence,
        });
    }
    report.cycles.sort_by(|a, b| {
        let level = |cycle: &DependencyCycle| u8::from(cycle.level == CycleLevel::File);
        level(a)
            .cmp(&level(b))
            .then_with(|| b.members.len().cmp(&a.members.len()))
            .then_with(|| a.members.cmp(&b.members))
    });

    // Layers and isolated modules.
    let mut by_layer: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for (index, module) in modules.iter().enumerate() {
        by_layer
            .entry(layer_of[index])
            .or_default()
            .push(module.id.clone());
    }
    report.layers = by_layer
        .into_iter()
        .map(|(index, modules)| Layer { index, modules })
        .collect();
    if module_count >= 2 {
        report.isolated_modules = report
            .modules
            .iter()
            .filter(|module| module.fan_in == 0 && module.fan_out == 0)
            .map(|module| module.id.clone())
            .collect();
    }

    // External packages.
    let mut external: Vec<ExternalImport> = builder
        .externals
        .into_iter()
        .map(|(name, data)| ExternalImport {
            name,
            ecosystem: Some(data.ecosystem.to_owned()),
            importers: u32::try_from(data.importers.len()).unwrap_or(u32::MAX),
            modules: data
                .modules
                .iter()
                .map(|&module| modules[module].id.clone())
                .collect(),
            samples: data.samples,
        })
        .collect();
    external.sort_by(|a, b| {
        b.importers
            .cmp(&a.importers)
            .then_with(|| a.name.cmp(&b.name))
    });
    if external.len() > MAX_EXTERNAL {
        report.notes.push(format!(
            "{} external packages are imported; the report lists the {MAX_EXTERNAL} most used.",
            external.len()
        ));
        external.truncate(MAX_EXTERNAL);
    }
    report.external = external;

    // Importance reasons.
    let index_of_path: HashMap<&str, usize> = files
        .iter()
        .enumerate()
        .map(|(index, file)| (file.path, index))
        .collect();
    let mut entrypoint_of_module: Vec<Option<&str>> = vec![None; module_count];
    for entry in &output.entrypoints {
        if let Some(module) = index_of_path
            .get(entry.path.as_str())
            .and_then(|&file| builder.module_index[file])
            && entrypoint_of_module[module].is_none()
        {
            entrypoint_of_module[module] = Some(entry.path.as_str());
        }
    }
    for (index, module) in report.modules.iter_mut().enumerate() {
        if module.fan_in >= 3 {
            module
                .importance
                .push(format!("{} modules depend on it", module.fan_in));
        }
        if module.centrality >= 0.2 {
            module.importance.push(format!(
                "On {}% of the shortest dependency paths between other modules",
                format_number((module.centrality * 1000.0).round() / 10.0)
            ));
        }
        if module_count >= 2 && total_code > 0 {
            let share = code_per_module[index] as f64 / total_code as f64;
            if share >= 0.25 {
                module.importance.push(format!(
                    "Holds {}% of the first-party code",
                    format_number((share * 1000.0).round() / 10.0)
                ));
            }
        }
        if let Some(path) = entrypoint_of_module[index] {
            module
                .importance
                .push(format!("Contains the entrypoint {path}"));
        }
    }

    let (style, style_confidence, signals) = classify_style(&report, total_code, members.len());
    report.style = style;
    report.style_confidence = style_confidence;
    report.signals = signals;
    report.status = SectionStatus::Analyzed;
    output.findings = architecture_findings(&report, total_code);
    output.interactions = detect_interactions(files, &builder.cross_edges);
    output.report = report;
    Ok(output)
}

/// Finds file-level import cycles among first-party files in load-order-sensitive languages.
fn file_cycles(
    files: &[SourceFile<'_>],
    members: &[usize],
    edges: &BTreeMap<(usize, usize), EdgeData>,
) -> Vec<DependencyCycle> {
    let sensitive = |file: usize| {
        files[file]
            .language
            .is_some_and(|l| FILE_CYCLE_LANGUAGES.contains(&l))
    };
    let mut node_of = vec![usize::MAX; files.len()];
    let nodes: Vec<usize> = members.iter().copied().filter(|&f| sensitive(f)).collect();
    for (node, &file) in nodes.iter().enumerate() {
        node_of[file] = node;
    }
    let mut graph = Graph::new(nodes.len());
    for (&(from, to), data) in edges {
        if data.kind != EdgeKind::Module && node_of[from] != usize::MAX && node_of[to] != usize::MAX
        {
            graph.add_edge(node_of[from], node_of[to]);
        }
    }
    graph.finish();
    let mut components: Vec<Vec<usize>> = strongly_connected_components(&graph)
        .into_iter()
        .filter(|component| component.len() > 1)
        .collect();
    components.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    components
        .iter()
        .take(MAX_FILE_CYCLES)
        .map(|component| {
            let path = shortest_cycle(&graph, component, 5).unwrap_or_default();
            let mut confidence = Confidence::High;
            let mut evidence = Vec::new();
            for pair in path.windows(2) {
                let (from, to) = (nodes[pair[0]], nodes[pair[1]]);
                if let Some(edge) = edges.get(&(from, to)) {
                    confidence = confidence.weakest(edge.confidence);
                    evidence.push(Evidence::edge_at(
                        files[from].path,
                        files[to].path,
                        files[from].path,
                        Some(edge.line),
                    ));
                }
            }
            let mut members: Vec<String> = component
                .iter()
                .map(|&node| files[nodes[node]].path.to_owned())
                .collect();
            members.sort();
            let languages: BTreeSet<String> = component
                .iter()
                .filter_map(|&node| files[nodes[node]].language.map(str::to_owned))
                .collect();
            let mut id_parts = vec!["file".to_owned()];
            id_parts.extend(members.iter().cloned());
            DependencyCycle {
                id: stable_id(&id_parts),
                level: CycleLevel::File,
                members,
                path: path
                    .iter()
                    .map(|&node| files[nodes[node]].path.to_owned())
                    .collect(),
                languages: languages.into_iter().collect(),
                confidence,
                evidence,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::structure::EntrypointKind;
    use repodna_parser::{FileAnalysis, LanguageRegistry, analyze_source};

    struct Repo {
        paths: Vec<&'static str>,
        analyses: Vec<Option<FileAnalysis>>,
    }

    impl Repo {
        fn new(files: &[(&'static str, &str)]) -> Self {
            let registry = LanguageRegistry::builtin();
            Self {
                paths: files.iter().map(|(path, _)| *path).collect(),
                analyses: files
                    .iter()
                    .map(|(path, text)| {
                        registry
                            .detect_path(path)
                            .map(|spec| analyze_source(spec, text))
                    })
                    .collect(),
            }
        }

        fn sources(&self) -> Vec<SourceFile<'_>> {
            self.paths
                .iter()
                .zip(&self.analyses)
                .map(|(path, analysis)| SourceFile {
                    path,
                    language: analysis.as_ref().map(|a| a.language.as_str()),
                    code_lines: analysis.as_ref().map_or(0, |a| a.lines.code.max(1)),
                    first_party: !path.ends_with(".md"),
                    test: path.starts_with("tests/"),
                    analysis: analysis.as_ref(),
                })
                .collect()
        }
    }

    fn run(repo: &Repo, packages: &[PackageBoundary]) -> ArchitectureOutput {
        let sources = repo.sources();
        let input = ArchitectureInput {
            files: &sources,
            packages,
            workspace: None,
            declared_entrypoints: &[],
            declared_dependencies: &[],
            max_file_edges: 1_000,
        };
        analyze(&input, &CancellationToken::new()).unwrap()
    }

    fn web_app() -> Repo {
        Repo::new(&[
            (
                "index.html",
                "<script type=\"module\" src=\"/src/main.tsx\"></script>\n",
            ),
            ("src/main.tsx", "import { App } from './components/App';\n"),
            (
                "src/components/App.tsx",
                "import React from 'react';\nimport { useStore } from '../store/store';\n",
            ),
            ("src/components/Button.tsx", "import React from 'react';\n"),
            (
                "src/store/store.ts",
                "import { fetchUser } from '../api/client';\n",
            ),
            (
                "src/api/client.ts",
                "import axios from 'axios';\nimport { useStore } from '../store/store';\n",
            ),
            (
                "src/utils/format.ts",
                "export const format = (x: number) => `${x}`;\n",
            ),
            (
                "tests/app.test.ts",
                "import { App } from '../src/components/App';\n",
            ),
            ("README.md", "# Demo\n"),
        ])
    }

    #[test]
    fn builds_modules_edges_cycles_and_findings() {
        let repo = web_app();
        let output = run(&repo, &[]);
        let report = &output.report;
        assert_eq!(report.status, SectionStatus::Analyzed);
        let ids: Vec<&str> = report.modules.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "(root)",
                "src",
                "src/api",
                "src/components",
                "src/store",
                "src/utils",
                "tests"
            ]
        );
        assert_eq!(report.resolved_imports, 6);
        assert_eq!(report.unresolved_imports, 0);
        let edges: Vec<(&str, &str)> = report
            .module_edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect();
        assert_eq!(
            edges,
            vec![
                ("(root)", "src"),
                ("src", "src/components"),
                ("src/api", "src/store"),
                ("src/components", "src/store"),
                ("src/store", "src/api"),
                ("tests", "src/components"),
            ]
        );

        let module_cycle = report
            .cycles
            .iter()
            .find(|c| c.level == CycleLevel::Module)
            .unwrap();
        assert_eq!(module_cycle.members, vec!["src/api", "src/store"]);
        assert_eq!(module_cycle.path.len(), 3);
        assert_eq!(module_cycle.languages, vec!["typescript"]);
        let file_cycle = report
            .cycles
            .iter()
            .find(|c| c.level == CycleLevel::File)
            .unwrap();
        assert_eq!(
            file_cycle.members,
            vec!["src/api/client.ts", "src/store/store.ts"]
        );
        assert_eq!(report.cycles[0].level, CycleLevel::Module);

        let store = report.module("src/store").unwrap();
        assert_eq!((store.fan_in, store.fan_out), (2, 1));
        assert_eq!(store.layer, Some(0));
        assert_eq!(report.module("(root)").unwrap().layer, Some(3));
        assert_eq!(report.isolated_modules, vec!["src/utils"]);
        assert_eq!(
            report
                .external
                .iter()
                .map(|e| (e.name.as_str(), e.importers))
                .collect::<Vec<_>>(),
            vec![("react", 2), ("axios", 1)]
        );
        assert_eq!(
            report
                .module("src/components")
                .unwrap()
                .external_dependencies,
            vec!["react"]
        );
        assert_eq!(report.style, "Modular");
        assert!(report.signals.iter().any(|s| s.id == "cycles"));

        let rules: BTreeSet<&str> = output.findings.iter().map(|f| f.rule.as_str()).collect();
        assert!(rules.contains("architecture.cycle"));
        assert!(rules.contains("architecture.file-cycle"));
        assert!(rules.contains("architecture.isolated-module"));
        let cycle = output
            .findings
            .iter()
            .find(|f| f.rule == "architecture.cycle")
            .unwrap();
        assert_eq!(cycle.severity, repodna_core::severity::Severity::Warning);

        let app = repo
            .paths
            .iter()
            .position(|p| *p == "src/components/App.tsx")
            .unwrap();
        assert_eq!(output.file_dependents[app], 2);
        assert_eq!(output.module_of[app].as_deref(), Some("src/components"));
        let readme = repo.paths.iter().position(|p| *p == "README.md").unwrap();
        assert_eq!(output.module_of[readme], None);

        assert_eq!(output.interactions.len(), 1);
        assert_eq!(output.interactions[0].from, "html");
        assert_eq!(output.interactions[0].to, "typescript");
        let kinds: Vec<(&str, EntrypointKind)> = output
            .entrypoints
            .iter()
            .map(|e| (e.path.as_str(), e.kind))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("index.html", EntrypointKind::Web),
                ("src/main.tsx", EntrypointKind::Web)
            ]
        );
        assert!(
            report
                .module("(root)")
                .unwrap()
                .importance
                .iter()
                .any(|reason| reason == "Contains the entrypoint index.html")
        );
    }

    #[test]
    fn records_module_declarations_separately_from_dependents() {
        let repo = Repo::new(&[
            ("src/lib.rs", "mod parser;\n"),
            ("src/parser.rs", "pub fn parse() {}\n"),
            ("src/orphan.rs", "pub fn unused() {}\n"),
        ]);
        let output = run(&repo, &[]);
        assert_eq!(output.module_declared, vec![false, true, false]);
        assert_eq!(output.file_dependents, vec![0, 0, 0]);
        assert_eq!(output.report.file_edges.len(), 1);
        assert_eq!(output.report.file_edges[0].kind, EdgeKind::Module);
    }

    #[test]
    fn package_imports_link_modules_but_not_files() {
        let repo = Repo::new(&[
            (
                "cmd/api/main.go",
                "package main\n\nimport \"example.com/shop/internal/store\"\n\nfunc main() {\n}\n",
            ),
            ("internal/store/store.go", "package store\n"),
            ("internal/store/cache.go", "package store\n"),
        ]);
        let packages = [PackageBoundary {
            name: "example.com/shop".into(),
            path: String::new(),
            ecosystem: "go".into(),
            manifest: "go.mod".into(),
            internal_dependencies: Vec::new(),
        }];
        let output = run(&repo, &packages);
        let report = &output.report;
        assert!(report.file_edges.is_empty());
        assert_eq!(report.module_edges.len(), 1);
        assert_eq!(report.module_edges[0].from, "cmd/api");
        assert_eq!(report.module_edges[0].to, "internal/store");
        assert_eq!(report.module_edges[0].weight, 2);
        assert_eq!(report.resolved_imports, 1);
        assert_eq!(output.entrypoints[0].path, "cmd/api/main.go");
    }

    #[test]
    fn caps_file_edges_and_handles_empty_input() {
        let repo = web_app();
        let sources = repo.sources();
        let input = ArchitectureInput {
            files: &sources,
            packages: &[],
            workspace: None,
            declared_entrypoints: &[],
            declared_dependencies: &[],
            max_file_edges: 2,
        };
        let output = analyze(&input, &CancellationToken::new()).unwrap();
        assert_eq!(output.report.file_edges.len(), 2);
        assert!(output.report.file_edges_truncated);
        assert!(output.report.notes[0].contains("file edges were found"));

        let empty = ArchitectureInput {
            files: &[],
            ..input
        };
        let output = analyze(&empty, &CancellationToken::new()).unwrap();
        assert_eq!(output.report.status, SectionStatus::Unavailable);
        assert_eq!(output.report.style, "Unknown");

        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(analyze(&input, &cancel).is_err());
    }
}
