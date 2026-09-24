//! Architecture, dependencies, hotspots, complexity, and duplication.

use std::collections::HashSet;

use repodna_core::model::architecture::CycleLevel;
use repodna_core::model::artifact::RepositoryDna;
use repodna_core::model::dependencies::{DependencyScope, ManifestKind, ParseStatus};
use repodna_core::model::quality::{FunctionSignal, MarkerKind};

use super::{ContentOptions, confidence, heading, not_analyzed, notes};
use crate::charts::{GraphNode, bars, columns, layered_graph};
use crate::doc::{Blocks, Inline, Rich, Table, code, plain, truncate};
use crate::sections::Section;
use crate::text::{number, percent, thousands};

/// Modules drawn in the dependency diagram (largest first).
const MAX_GRAPH_MODULES: usize = 40;

fn function_name(function: &FunctionSignal) -> Rich {
    if function.name.is_empty() {
        plain("(name removed)")
    } else {
        code(function.name.clone())
    }
}

fn location(path: &str, line: u32) -> Rich {
    code(format!("{path}:{line}"))
}

pub(super) fn architecture(blocks: &mut Blocks, dna: &RepositoryDna, options: ContentOptions) {
    heading(blocks, Section::Architecture);
    let report = &dna.architecture;
    if not_analyzed(blocks, report.status, &report.notes) {
        return;
    }
    blocks.rich(vec![
        Inline::Text("Inferred style: ".to_owned()),
        Inline::Strong(report.style.clone()),
        Inline::Text(format!(
            " ({} confidence). {} modules, {} module dependencies, {} layers.",
            report.style_confidence.label().to_lowercase(),
            report.modules.len(),
            report.module_edges.len(),
            report.layers.len()
        )),
    ]);
    blocks.list(
        report
            .signals
            .iter()
            .map(|signal| {
                vec![
                    Inline::Strong(signal.label.clone()),
                    Inline::Text(format!(
                        ": {} ({} confidence)",
                        signal.description,
                        signal.confidence.label().to_lowercase()
                    )),
                ]
            })
            .collect(),
    );
    let mut modules: Vec<_> = report.modules.iter().collect();
    modules.sort_by(|a, b| {
        b.code_lines
            .cmp(&a.code_lines)
            .then_with(|| a.id.cmp(&b.id))
    });
    if options.figures && modules.len() > 1 {
        let shown: Vec<_> = modules.iter().take(MAX_GRAPH_MODULES).collect();
        let ids: HashSet<&str> = shown.iter().map(|m| m.id.as_str()).collect();
        let nodes: Vec<GraphNode> = shown
            .iter()
            .map(|module| GraphNode {
                id: module.id.clone(),
                label: module.name.clone(),
                layer: module.layer.unwrap_or(0),
            })
            .collect();
        let edges: Vec<(String, String, u32)> = report
            .module_edges
            .iter()
            .filter(|edge| ids.contains(edge.from.as_str()) && ids.contains(edge.to.as_str()))
            .map(|edge| (edge.from.clone(), edge.to.clone(), edge.weight))
            .collect();
        let caption = if modules.len() > MAX_GRAPH_MODULES {
            format!(
                "Module dependencies (the {MAX_GRAPH_MODULES} largest modules; dependents above their dependencies)"
            )
        } else {
            "Module dependencies (dependents above their dependencies)".to_owned()
        };
        blocks.figure(layered_graph(&nodes, &edges, &caption), caption);
    }
    blocks.heading(3, "Modules", None);
    let mut table = Table::new(&[
        "Module",
        "Path",
        "Files#",
        "Code lines#",
        "Depends on#",
        "Used by#",
        "Layer#",
        "Why it stands out",
    ]);
    let mut rows = modules.clone();
    table.omitted = truncate(&mut rows, 30);
    for module in rows {
        table.row(vec![
            plain(module.name.clone()),
            code(if module.path.is_empty() {
                "(root)".to_owned()
            } else {
                format!("{}/", module.path)
            }),
            plain(thousands(module.files)),
            plain(thousands(module.code_lines)),
            plain(module.fan_out.to_string()),
            plain(module.fan_in.to_string()),
            plain(
                module
                    .layer
                    .map_or_else(|| "–".to_owned(), |l| l.to_string()),
            ),
            plain(module.importance.join("; ")),
        ]);
    }
    blocks.table(table);
    if !report.cycles.is_empty() {
        blocks.heading(3, "Dependency cycles", None);
        blocks.list(
            report
                .cycles
                .iter()
                .map(|cycle| {
                    let level = match cycle.level {
                        CycleLevel::Module => "Module cycle",
                        CycleLevel::File => "File cycle",
                    };
                    vec![
                        Inline::Strong(level.to_owned()),
                        Inline::Text(": ".to_owned()),
                        Inline::Code(cycle.path.join(" → ")),
                        Inline::Text(format!(
                            " ({}; {} confidence)",
                            cycle.languages.join(", "),
                            cycle.confidence.label().to_lowercase()
                        )),
                    ]
                })
                .collect(),
        );
    }
    if !report.module_edges.is_empty() {
        blocks.heading(3, "Strongest module dependencies", None);
        let mut edges: Vec<_> = report.module_edges.iter().collect();
        edges.sort_by(|a, b| {
            b.weight
                .cmp(&a.weight)
                .then_with(|| a.from.cmp(&b.from))
                .then_with(|| a.to.cmp(&b.to))
        });
        let mut table = Table::new(&["From", "To", "Imports#", "Confidence"]);
        table.omitted = truncate(&mut edges, 20);
        for edge in edges {
            table.row(vec![
                plain(edge.from.clone()),
                plain(edge.to.clone()),
                plain(thousands(u64::from(edge.weight))),
                plain(confidence(edge.confidence)),
            ]);
        }
        blocks.table(table);
    }
    if !report.external.is_empty() {
        blocks.heading(3, "Most imported external packages", None);
        let mut table = Table::new(&["Package", "Ecosystem", "Importing files#", "Modules"]);
        let mut external: Vec<_> = report.external.iter().collect();
        table.omitted = truncate(&mut external, 20);
        for package in external {
            table.row(vec![
                code(package.name.clone()),
                plain(package.ecosystem.clone().unwrap_or_else(|| "–".to_owned())),
                plain(thousands(u64::from(package.importers))),
                plain(package.modules.join(", ")),
            ]);
        }
        blocks.table(table);
    }
    if !report.isolated_modules.is_empty() {
        blocks.text(format!(
            "Modules with no internal dependencies in either direction: {}.",
            report.isolated_modules.join(", ")
        ));
    }
    if let Some(workspace) = &report.workspace {
        blocks.text(format!(
            "Workspace: {} declared in {} with {} members.",
            workspace.tool,
            workspace.manifest,
            workspace.members.len()
        ));
    }
    let attempted = report.resolved_imports + report.unresolved_imports;
    if attempted > 0 {
        blocks.text(format!(
            "Import resolution: {} of {} internal-looking imports resolved ({}).",
            thousands(report.resolved_imports),
            thousands(attempted),
            percent(report.resolved_imports as f64 / attempted as f64)
        ));
    }
    if !report.method.is_empty() {
        blocks.note(report.method.clone());
    }
    notes(blocks, &report.notes);
}

fn scope_label(scope: DependencyScope) -> &'static str {
    match scope {
        DependencyScope::Runtime => "Runtime",
        DependencyScope::Development => "Development",
        DependencyScope::Build => "Build",
        DependencyScope::Optional => "Optional",
        DependencyScope::Peer => "Peer",
    }
}

fn parse_label(status: ParseStatus) -> &'static str {
    match status {
        ParseStatus::Parsed => "Parsed",
        ParseStatus::Partial => "Partially parsed",
        ParseStatus::Failed => "Could not be parsed",
    }
}

pub(super) fn dependencies(blocks: &mut Blocks, dna: &RepositoryDna) {
    heading(blocks, Section::Dependencies);
    let report = &dna.dependencies;
    if not_analyzed(blocks, report.status, &report.notes) {
        return;
    }
    blocks.stats(vec![
        (
            "Direct dependencies".to_owned(),
            thousands(report.direct_count),
        ),
        ("Locked packages".to_owned(), thousands(report.locked_count)),
        (
            "Manifests".to_owned(),
            thousands(
                report
                    .manifests
                    .iter()
                    .filter(|m| m.kind == ManifestKind::Manifest)
                    .count() as u64,
            ),
        ),
        (
            "Lockfiles".to_owned(),
            thousands(report.lockfiles.len() as u64),
        ),
        (
            "Ecosystems".to_owned(),
            thousands(report.ecosystems.len() as u64),
        ),
    ]);
    let mut table = Table::new(&[
        "Ecosystem",
        "Manifests#",
        "Lockfiles#",
        "Runtime#",
        "Other#",
        "Locked#",
    ]);
    for ecosystem in &report.ecosystems {
        table.row(vec![
            plain(ecosystem.ecosystem.clone()),
            plain(ecosystem.manifests.to_string()),
            plain(ecosystem.lockfiles.to_string()),
            plain(ecosystem.runtime.to_string()),
            plain(ecosystem.other.to_string()),
            plain(thousands(u64::from(ecosystem.locked_packages))),
        ]);
    }
    blocks.table(table);
    blocks.heading(3, "Manifests and lockfiles", None);
    let mut table = Table::new(&[
        "Path",
        "Ecosystem",
        "Kind",
        "Package",
        "Entries#",
        "Parsing",
    ]);
    let mut manifests: Vec<_> = report.manifests.iter().collect();
    table.omitted = truncate(&mut manifests, 40);
    for manifest in manifests {
        table.row(vec![
            code(manifest.path.clone()),
            plain(manifest.ecosystem.clone()),
            plain(match manifest.kind {
                ManifestKind::Manifest => "Manifest",
                ManifestKind::Lockfile => "Lockfile",
            }),
            plain(
                manifest
                    .package_name
                    .clone()
                    .unwrap_or_else(|| "–".to_owned()),
            ),
            plain(thousands(u64::from(manifest.entries))),
            plain(match &manifest.message {
                Some(message) => format!("{}: {message}", parse_label(manifest.status)),
                None => parse_label(manifest.status).to_owned(),
            }),
        ]);
    }
    blocks.table(table);
    let mut direct: Vec<_> = report.dependencies.iter().filter(|d| !d.internal).collect();
    if !direct.is_empty() {
        blocks.heading(3, "Declared dependencies", None);
        direct.sort_by(|a, b| {
            a.ecosystem
                .cmp(&b.ecosystem)
                .then_with(|| (a.scope as u8).cmp(&(b.scope as u8)))
                .then_with(|| a.name.cmp(&b.name))
        });
        let mut table = Table::new(&[
            "Package",
            "Ecosystem",
            "Requirement",
            "Scope",
            "Declared in",
        ]);
        table.omitted = truncate(&mut direct, 80);
        for dependency in direct {
            table.row(vec![
                code(dependency.name.clone()),
                plain(dependency.ecosystem.clone()),
                plain(
                    dependency
                        .requirement
                        .clone()
                        .unwrap_or_else(|| "–".to_owned()),
                ),
                plain(scope_label(dependency.scope)),
                plain(dependency.manifests.join(", ")),
            ]);
        }
        blocks.table(table);
    }
    for lockfile in report
        .lockfiles
        .iter()
        .filter(|l| !l.missing_from_lock.is_empty())
    {
        blocks.rich(vec![
            Inline::Code(lockfile.path.clone()),
            Inline::Text(format!(
                " does not lock {} declared dependencies: {}.",
                lockfile.missing_from_lock.len(),
                lockfile
                    .missing_from_lock
                    .iter()
                    .take(15)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        ]);
    }
    if !report.duplicates.is_empty() {
        blocks.heading(3, "Packages locked at several versions", None);
        let mut table = Table::new(&["Package", "Ecosystem", "Versions", "Lockfile"]);
        let mut duplicates: Vec<_> = report.duplicates.iter().collect();
        table.omitted = truncate(&mut duplicates, 30);
        for duplicate in duplicates {
            table.row(vec![
                code(duplicate.name.clone()),
                plain(duplicate.ecosystem.clone()),
                plain(duplicate.versions.join(", ")),
                code(duplicate.lockfile.clone()),
            ]);
        }
        blocks.table(table);
    }
    if !report.concentration.is_empty() {
        blocks.heading(3, "Import concentration", None);
        let mut table = Table::new(&["Package", "Importing files#", "Share#"]);
        for package in &report.concentration {
            table.row(vec![
                code(package.name.clone()),
                plain(thousands(u64::from(package.importers))),
                plain(percent(package.share)),
            ]);
        }
        blocks.table(table);
    }
    blocks.list(
        report
            .stale_signals
            .iter()
            .map(|signal| {
                vec![
                    Inline::Code(signal.manifest.clone()),
                    Inline::Text(format!(": {}", signal.description)),
                ]
            })
            .collect(),
    );
    if !report.advisories.note.is_empty() {
        blocks.note(report.advisories.note.clone());
    }
    notes(blocks, &report.notes);
}

pub(super) fn hotspots(blocks: &mut Blocks, dna: &RepositoryDna, options: ContentOptions) {
    heading(blocks, Section::Hotspots);
    let git = &dna.git;
    if !git.status.has_results() {
        blocks.caution(format!(
            "Hotspots need Git history, which was not analyzed. {}",
            git.notes.first().cloned().unwrap_or_default()
        ));
        return;
    }
    if git.hot_spots.is_empty() {
        blocks.text(format!(
            "No file had at least {} commits, so no hotspots were ranked.",
            dna.analysis_metadata.thresholds.hotspot_min_commits
        ));
        return;
    }
    if options.figures {
        let items: Vec<(String, f64)> = git
            .hot_spots
            .iter()
            .take(15)
            .map(|h| (h.path.clone(), (h.score * 100.0).round() / 100.0))
            .collect();
        blocks.figure(
            bars(&items, "score", "Hotspot scores"),
            "Hotspot scores of the 15 highest-ranked files",
        );
    }
    let mut table = Table::new(&[
        "Rank#",
        "File",
        "Score#",
        "Commits#",
        "Authors#",
        "Churn#",
        "Recent#",
        "Code lines#",
        "Complexity#",
        "Dependents#",
        "Why",
    ]);
    let mut rows: Vec<_> = git.hot_spots.iter().collect();
    table.omitted = truncate(&mut rows, 30);
    for hotspot in rows {
        table.row(vec![
            plain(hotspot.rank.to_string()),
            code(hotspot.path.clone()),
            plain(number(hotspot.score)),
            plain(thousands(u64::from(hotspot.commits))),
            plain(hotspot.authors.to_string()),
            plain(thousands(hotspot.churn)),
            plain(hotspot.recent_commits.to_string()),
            plain(thousands(hotspot.lines)),
            plain(hotspot.complexity.to_string()),
            plain(hotspot.dependents.to_string()),
            plain(hotspot.reasons.join("; ")),
        ]);
    }
    blocks.table(table);
    blocks.note("Files changed in at least the minimum number of commits are ranked by a weighted sum of percentile ranks: commits 30%, churn 20%, recent commits 15%, highest function complexity 15%, code lines 10%, distinct authors 5%, and importing files 5%. A hotspot shows where change concentrates; it is not evidence of a defect.");
}

fn marker_label(kind: MarkerKind) -> &'static str {
    match kind {
        MarkerKind::Todo => "TODO",
        MarkerKind::Fixme => "FIXME",
        MarkerKind::Hack => "HACK",
        MarkerKind::Xxx => "XXX",
        MarkerKind::Bug => "BUG",
        MarkerKind::Deprecated => "Deprecated",
    }
}

fn functions_table(functions: &[FunctionSignal]) -> Table {
    let mut table = Table::new(&["Function", "Location", "Cyclomatic#", "Lines#", "Nesting#"]);
    let mut rows: Vec<_> = functions.iter().collect();
    table.omitted = truncate(&mut rows, 15);
    for function in rows {
        table.row(vec![
            function_name(function),
            location(&function.path, function.line),
            plain(function.cyclomatic.to_string()),
            plain(thousands(u64::from(function.lines))),
            plain(function.nesting.to_string()),
        ]);
    }
    table
}

pub(super) fn complexity(blocks: &mut Blocks, dna: &RepositoryDna, options: ContentOptions) {
    heading(blocks, Section::Complexity);
    let report = &dna.code_quality;
    if not_analyzed(blocks, report.status, &report.notes) {
        return;
    }
    let complexity = &report.complexity;
    blocks.stats(vec![
        (
            "Functions measured".to_owned(),
            thousands(complexity.functions_analyzed),
        ),
        (
            "Average cyclomatic complexity".to_owned(),
            number(complexity.average_cyclomatic),
        ),
        (
            "Median cyclomatic complexity".to_owned(),
            number(complexity.median_cyclomatic),
        ),
        ("Work markers".to_owned(), thousands(report.markers.total)),
    ]);
    if options.figures {
        let points: Vec<(String, f64)> = complexity
            .distribution
            .iter()
            .map(|bucket| (bucket.label.clone(), bucket.count as f64))
            .collect();
        blocks.figure(
            columns(&points, "functions", "Functions by cyclomatic complexity"),
            "Functions by cyclomatic complexity",
        );
    }
    let mut table = Table::new(&["Cyclomatic complexity", "Functions#"]);
    for bucket in &complexity.distribution {
        table.row(vec![
            plain(bucket.label.clone()),
            plain(thousands(bucket.count)),
        ]);
    }
    blocks.table(table);
    if !complexity.by_language.is_empty() {
        let mut table = Table::new(&["Language", "Functions#", "Average#", "Highest#"]);
        for language in &complexity.by_language {
            table.row(vec![
                plain(language.language.clone()),
                plain(thousands(language.functions)),
                plain(number(language.average)),
                plain(language.max.to_string()),
            ]);
        }
        blocks.table(table);
    }
    if !complexity.top_functions.is_empty() {
        blocks.heading(3, "Most complex functions", None);
        blocks.table(functions_table(&complexity.top_functions));
    }
    if !report.large_functions.is_empty() {
        blocks.heading(3, "Long functions", None);
        blocks.table(functions_table(&report.large_functions));
    }
    if !report.deep_nesting.is_empty() {
        blocks.heading(3, "Deeply nested functions", None);
        blocks.table(functions_table(&report.deep_nesting));
    }
    if !report.large_files.is_empty() {
        blocks.heading(3, "Large files", None);
        let mut table = Table::new(&["File", "Size#", "Threshold#"]);
        let mut rows: Vec<_> = report.large_files.iter().collect();
        table.omitted = truncate(&mut rows, 20);
        for file in rows {
            table.row(vec![
                code(file.path.clone()),
                plain(format!("{} {}", thousands(file.value), file.unit)),
                plain(thousands(file.threshold)),
            ]);
        }
        blocks.table(table);
    }
    if report.markers.total > 0 {
        blocks.heading(3, "Work markers", None);
        blocks.text(
            report
                .markers
                .counts
                .iter()
                .filter(|count| count.count > 0)
                .map(|count| format!("{} {}", thousands(count.count), marker_label(count.kind)))
                .collect::<Vec<_>>()
                .join(" · "),
        );
        let mut table = Table::new(&["Marker", "Location", "Text"]);
        let mut items: Vec<_> = report.markers.items.iter().collect();
        table.omitted = truncate(&mut items, 20);
        for item in items {
            table.row(vec![
                plain(marker_label(item.kind)),
                location(&item.path, item.line),
                plain(if item.text.is_empty() {
                    "(text removed)".to_owned()
                } else {
                    item.text.clone()
                }),
            ]);
        }
        blocks.table(table);
    }
    if !report.dead_code_candidates.is_empty() {
        blocks.heading(3, "Dead-code candidates", None);
        let mut table = Table::new(&["Path", "Candidate", "Why", "Confidence"]);
        let mut rows: Vec<_> = report.dead_code_candidates.iter().collect();
        table.omitted = truncate(&mut rows, 25);
        for candidate in rows {
            table.row(vec![
                code(candidate.path.clone()),
                plain(candidate.label.clone()),
                plain(candidate.reason.clone()),
                plain(confidence(candidate.confidence)),
            ]);
        }
        blocks.table(table);
        blocks.note("Candidates are files or modules that nothing appears to use. Dynamic loading, reflection, and external consumers are invisible to static analysis, so review each one before removing anything.");
    }
    if !report.maintainability.is_empty() {
        blocks.heading(3, "Maintainability signals", None);
        let mut table = Table::new(&["Signal", "Value#", "What it means"]);
        for signal in &report.maintainability {
            table.row(vec![
                plain(signal.label.clone()),
                plain(format!("{} {}", number(signal.value), signal.unit)),
                plain(signal.description.clone()),
            ]);
        }
        blocks.table(table);
    }
    if !complexity.method.is_empty() {
        blocks.note(complexity.method.clone());
    }
    notes(blocks, &report.notes);
}

pub(super) fn duplication(blocks: &mut Blocks, dna: &RepositoryDna) {
    heading(blocks, Section::Duplication);
    let duplication = &dna.code_quality.duplication;
    let similarity = &dna.similarity;
    if !duplication.status.has_results() && !similarity.status.has_results() {
        blocks.caution(
            "Duplication and similarity were not analyzed; they run with the deep profile.",
        );
        return;
    }
    if duplication.status.has_results() {
        blocks.stats(vec![
            (
                "Duplicated lines".to_owned(),
                thousands(duplication.duplicated_lines),
            ),
            (
                "Analyzed lines".to_owned(),
                thousands(duplication.analyzed_lines),
            ),
            ("Duplication ratio".to_owned(), percent(duplication.ratio)),
            (
                "Clusters".to_owned(),
                thousands(duplication.clusters.len() as u64),
            ),
        ]);
        let mut table = Table::new(&["Language", "Tokens#", "Lines#", "Copies#", "Locations"]);
        let mut clusters: Vec<_> = duplication.clusters.iter().collect();
        table.omitted = truncate(&mut clusters, 20);
        for cluster in clusters {
            let mut locations: Vec<String> = cluster
                .occurrences
                .iter()
                .take(3)
                .map(|o| format!("{}:{}-{}", o.path, o.start_line, o.end_line))
                .collect();
            if cluster.occurrence_count as usize > locations.len() {
                locations.push(format!(
                    "+{} more",
                    cluster.occurrence_count as usize - locations.len()
                ));
            }
            table.row(vec![
                plain(cluster.language.clone()),
                plain(thousands(u64::from(cluster.tokens))),
                plain(thousands(u64::from(cluster.lines))),
                plain(cluster.occurrence_count.to_string()),
                code(locations.join(", ")),
            ]);
        }
        blocks.table(table);
        if !duplication.method.is_empty() {
            blocks.note(duplication.method.clone());
        }
    }
    if similarity.status.has_results() {
        blocks.heading(3, "Similar files", None);
        if similarity.similar_files.is_empty() {
            blocks.text(format!(
                "No pair of files reached the similarity threshold of {}.",
                percent(similarity.threshold)
            ));
        } else {
            let mut table = Table::new(&["File", "Similar to", "Estimated similarity#"]);
            let mut pairs: Vec<_> = similarity.similar_files.iter().collect();
            table.omitted = truncate(&mut pairs, 25);
            for pair in pairs {
                table.row(vec![
                    code(pair.a.clone()),
                    code(pair.b.clone()),
                    plain(percent(pair.similarity)),
                ]);
            }
            blocks.table(table);
        }
        if !similarity.method.is_empty() {
            blocks.note(similarity.method.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{ContentOptions, section};
    use crate::markdown::render;
    use crate::sections::Section;
    use repodna_core::confidence::Confidence;
    use repodna_core::model::SectionStatus;
    use repodna_core::model::architecture::{ModuleEdge, ModuleKind, ModuleRecord};
    use repodna_core::model::artifact::RepositoryDna;
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::metadata::AnalysisMetadata;

    fn module(id: &str, lines: u64, layer: u32) -> ModuleRecord {
        ModuleRecord {
            id: id.into(),
            name: id.into(),
            path: id.into(),
            kind: ModuleKind::Directory,
            language: Some("rust".into()),
            files: 2,
            code_lines: lines,
            fan_in: 0,
            fan_out: 0,
            instability: 0.0,
            centrality: 0.0,
            layer: Some(layer),
            inferred: true,
            confidence: Confidence::Medium,
            importance: vec!["Holds most of the code".into()],
            external_dependencies: Vec::new(),
            evidence: Vec::new(),
        }
    }

    #[test]
    fn describes_architecture_with_a_diagram() {
        let mut dna =
            RepositoryDna::new(RepositoryIdentity::default(), AnalysisMetadata::default());
        dna.architecture.status = SectionStatus::Analyzed;
        dna.architecture.style = "Layered".into();
        dna.architecture.style_confidence = Confidence::Medium;
        dna.architecture.modules = vec![module("app", 100, 1), module("core", 300, 0)];
        dna.architecture.module_edges = vec![ModuleEdge {
            from: "app".into(),
            to: "core".into(),
            weight: 4,
            confidence: Confidence::High,
            samples: Vec::new(),
        }];
        let options = ContentOptions {
            figures: true,
            ..ContentOptions::default()
        };
        let blocks = section(&dna, Section::Architecture, options);
        assert!(
            blocks
                .0
                .iter()
                .any(|b| matches!(b, crate::doc::Block::Figure { .. }))
        );
        let markdown = render(&blocks);
        assert!(markdown.contains("Inferred style: **Layered** (medium confidence). 2 modules"));
        assert!(markdown.contains("| core | `core/` | 2 | 300 |"));
        assert!(markdown.contains("| app | core | 4 | High |"));
    }

    #[test]
    fn explains_missing_prerequisites() {
        let dna = RepositoryDna::new(RepositoryIdentity::default(), AnalysisMetadata::default());
        let hotspots = render(&section(&dna, Section::Hotspots, ContentOptions::default()));
        assert!(hotspots.contains("Hotspots need Git history"));
        let duplication = render(&section(
            &dna,
            Section::Duplication,
            ContentOptions::default(),
        ));
        assert!(duplication.contains("they run with the deep profile"));
    }
}
