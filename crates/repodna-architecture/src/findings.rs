//! Architecture style signals and findings derived from a finished report.
//!
//! Everything here is a pure function of the [`ArchitectureReport`], so the rules are easy
//! to test and to explain: each finding lists the measured values and thresholds it used.

use repodna_core::confidence::Confidence;
use repodna_core::evidence::{Evidence, format_number};
use repodna_core::finding::{Finding, FindingCategory};
use repodna_core::model::architecture::{
    ArchitectureReport, ArchitectureSignal, CycleLevel, DependencyCycle, ModuleKind, ModuleRecord,
};
use repodna_core::paths;
use repodna_core::severity::Severity;

/// Share of first-party code in one module at or above which code counts as concentrated.
pub const CODE_CONCENTRATION: f64 = 0.5;

/// Share of first-party code in one module at or above which the style is monolithic.
pub const MONOLITH_SHARE: f64 = 0.6;

/// Normalized betweenness centrality at or above which a module is reported as a hub.
pub const HUB_CENTRALITY: f64 = 0.3;

/// Minimum number of modules before concentration and hub findings are meaningful.
pub const MIN_MODULES: usize = 4;

/// Maximum file-cycle findings (all cycles stay in the report).
pub const MAX_FILE_CYCLE_FINDINGS: usize = 20;

/// Maximum isolated-module findings.
pub const MAX_ISOLATED_FINDINGS: usize = 10;

/// Minimum number of repository-style imports before the unresolved share is reported.
const MIN_IMPORTS_FOR_UNRESOLVED: u64 = 20;

/// Unresolved share of repository-style imports at or above which a finding is reported.
const UNRESOLVED_SHARE: f64 = 0.3;

/// Languages whose compilers resolve cyclic references between modules, so a module cycle
/// is a design concern rather than a potential load-order failure.
const CYCLE_TOLERANT: &[&str] = &[
    "csharp", "dart", "go", "java", "kotlin", "rust", "scala", "swift",
];

/// Module directory names that usually hold consumers (tests, examples, tooling), which
/// are expected to have no incoming edges.
const CONSUMER_DIRS: &[&str] = &[
    "__tests__",
    "bench",
    "benches",
    "benchmarks",
    "docs",
    "e2e",
    "example",
    "examples",
    "fixtures",
    "samples",
    "scripts",
    "spec",
    "specs",
    "test",
    "tests",
    "tools",
    "xtask",
];

const LIMITATION: &str = "Imports are resolved statically; dynamic imports, reflection, and generated code are not seen.";

fn percent(value: f64) -> String {
    format_number((value * 100.0 * 10.0).round() / 10.0)
}

fn code_share(module: &ModuleRecord, total_code_lines: u64) -> f64 {
    if total_code_lines == 0 {
        0.0
    } else {
        module.code_lines as f64 / total_code_lines as f64
    }
}

/// The module holding the most code, with its share of all first-party code.
fn largest_module(
    report: &ArchitectureReport,
    total_code_lines: u64,
) -> Option<(&ModuleRecord, f64)> {
    report
        .modules
        .iter()
        .max_by(|a, b| {
            a.code_lines
                .cmp(&b.code_lines)
                .then_with(|| b.id.cmp(&a.id))
        })
        .map(|module| (module, code_share(module, total_code_lines)))
}

/// Classifies the architecture style. Returns the label, its confidence, and every signal
/// that was observed (the label comes from the highest-priority signal).
pub fn classify_style(
    report: &ArchitectureReport,
    total_code_lines: u64,
    member_files: usize,
) -> (String, Confidence, Vec<ArchitectureSignal>) {
    let mut signals = Vec::new();
    let module_count = report.modules.len();
    if module_count == 0 {
        return ("Unknown".to_owned(), Confidence::Unavailable, signals);
    }
    let packages: Vec<_> = report
        .packages
        .iter()
        .filter(|package| !package.path.is_empty())
        .collect();
    if packages.len() >= 2 || report.workspace.is_some() {
        let mut evidence: Vec<Evidence> = report
            .workspace
            .iter()
            .map(|workspace| {
                Evidence::file(&workspace.manifest)
                    .with_note(format!("declares a {} workspace", workspace.tool))
            })
            .collect();
        evidence.extend(packages.iter().take(5).map(|package| {
            Evidence::file(&package.manifest).with_note(format!("package {}", package.name))
        }));
        signals.push(ArchitectureSignal {
            id: "monorepo".into(),
            label: "Monorepo".into(),
            description: format!(
                "{} packages are declared in one repository{}.",
                packages.len(),
                if report.workspace.is_some() {
                    " with a workspace configuration"
                } else {
                    ""
                }
            ),
            confidence: Confidence::High,
            evidence,
        });
    }
    let module_cycles = report
        .cycles
        .iter()
        .filter(|cycle| cycle.level == CycleLevel::Module)
        .count();
    if module_count >= 3 && module_cycles == 0 && report.layers.len() >= 3 {
        signals.push(ArchitectureSignal {
            id: "layered".into(),
            label: "Layered".into(),
            description: format!(
                "Dependencies flow one way through {} layers and no module cycles were found.",
                report.layers.len()
            ),
            confidence: Confidence::Medium,
            evidence: vec![
                Evidence::metric("architecture.layers", report.layers.len() as f64),
                Evidence::metric("architecture.module_cycles", 0.0),
            ],
        });
    }
    let largest = largest_module(report, total_code_lines);
    let largest_share = largest.map_or(0.0, |(_, share)| share);
    if module_count >= MIN_MODULES && !report.module_edges.is_empty() {
        let average_fan_out = report
            .modules
            .iter()
            .map(|module| f64::from(module.fan_out))
            .sum::<f64>()
            / module_count as f64;
        if average_fan_out <= 3.0 && largest_share < CODE_CONCENTRATION {
            signals.push(ArchitectureSignal {
                id: "modular".into(),
                label: "Modular".into(),
                description: format!(
                    "{module_count} modules with {} dependencies between them; no module holds most of the code.",
                    report.module_edges.len()
                ),
                confidence: Confidence::Medium,
                evidence: vec![
                    Evidence::metric("architecture.modules", module_count as f64),
                    Evidence::metric("architecture.average_fan_out", average_fan_out),
                ],
            });
        }
    }
    if let Some((module, share)) = largest
        && ((module_count >= 2 && share >= MONOLITH_SHARE)
            || (module_count == 1 && member_files > 30))
    {
        signals.push(ArchitectureSignal {
            id: "monolithic".into(),
            label: "Monolithic".into(),
            description: format!(
                "{} holds {}% of the first-party code.",
                module.name,
                percent(share)
            ),
            confidence: if module_count == 1 {
                Confidence::Low
            } else {
                Confidence::Medium
            },
            evidence: vec![Evidence::metric_with_threshold(
                "architecture.module.code_share",
                share,
                MONOLITH_SHARE,
                "ratio",
            )],
        });
    }
    if module_count <= 2 && member_files <= 30 {
        signals.push(ArchitectureSignal {
            id: "flat".into(),
            label: "Flat".into(),
            description: format!(
                "{member_files} code files in {module_count} module{}.",
                if module_count == 1 { "" } else { "s" }
            ),
            confidence: Confidence::Low,
            evidence: vec![Evidence::metric(
                "architecture.modules",
                module_count as f64,
            )],
        });
    }
    if module_cycles > 0 {
        signals.push(ArchitectureSignal {
            id: "cycles".into(),
            label: "Module cycles".into(),
            description: format!(
                "{module_cycles} group{} of modules depend on each other.",
                if module_cycles == 1 { "" } else { "s" }
            ),
            confidence: Confidence::High,
            evidence: vec![Evidence::metric(
                "architecture.module_cycles",
                module_cycles as f64,
            )],
        });
    }
    let (label, confidence) = ["monorepo", "layered", "modular", "monolithic", "flat"]
        .iter()
        .find_map(|id| signals.iter().find(|signal| signal.id == *id))
        .map_or(("Mixed".to_owned(), Confidence::Low), |signal| {
            (signal.label.clone(), signal.confidence)
        });
    (label, confidence, signals)
}

fn cycle_is_tolerated(cycle: &DependencyCycle) -> bool {
    !cycle.languages.is_empty()
        && cycle
            .languages
            .iter()
            .all(|language| CYCLE_TOLERANT.contains(&language.as_str()))
}

fn module_cycle_finding(report: &ArchitectureReport, cycle: &DependencyCycle) -> Finding {
    let names: Vec<String> = cycle
        .path
        .iter()
        .map(|id| {
            report
                .module(id)
                .map_or_else(|| id.clone(), |m| m.name.clone())
        })
        .collect();
    let title = if cycle.members.len() == 2 {
        format!("Dependency cycle between {} and {}", names[0], names[1])
    } else {
        format!("Dependency cycle between {} modules", cycle.members.len())
    };
    let tolerated = cycle_is_tolerated(cycle);
    let mut rationale = "Modules in a cycle cannot be built, tested, or understood in isolation, and a change in one can ripple through all of them.".to_owned();
    if tolerated {
        rationale.push_str(
            " The language resolves these references at compile time, so this is a design concern rather than a load-order risk.",
        );
    }
    let mut finding = Finding::new(
        "architecture.cycle",
        &cycle.members.join(","),
        FindingCategory::Architecture,
        if tolerated {
            Severity::Attention
        } else {
            Severity::Warning
        },
        cycle.confidence,
        title,
    )
    .summary(format!(
        "{} modules depend on each other: {}.",
        cycle.members.len(),
        names.join(" → ")
    ))
    .rationale(rationale)
    .method("Strongly connected components (Tarjan's algorithm) of the module dependency graph built from statically resolved imports; the path is a shortest cycle through the component.")
    .with_evidence(cycle.evidence.iter().cloned())
    .limitation(LIMITATION)
    .next_step("Follow the edges on the path and move the shared code into a module that both sides can depend on, or invert one of the dependencies.");
    for id in &cycle.members {
        if let Some(module) = report.module(id) {
            finding = finding.path(module.path.clone());
        }
    }
    finding
}

fn file_cycle_finding(cycle: &DependencyCycle) -> Finding {
    let title = if cycle.members.len() == 2 {
        format!(
            "Import cycle between {} and {}",
            paths::file_name(&cycle.members[0]),
            paths::file_name(&cycle.members[1])
        )
    } else {
        format!("Import cycle between {} files", cycle.members.len())
    };
    let mut finding = Finding::new(
        "architecture.file-cycle",
        &cycle.members.join(","),
        FindingCategory::Architecture,
        Severity::Attention,
        cycle.confidence,
        title,
    )
    .summary(format!("Files import each other: {}.", cycle.path.join(" → ")))
    .rationale("Circular imports can fail at runtime depending on load order (a module sees another one only partially initialized) and prevent reusing either file on its own.")
    .method("Strongly connected components of the file import graph, for languages where import order matters at runtime; the path is a shortest cycle.")
    .with_evidence(cycle.evidence.iter().cloned())
    .limitation(LIMITATION)
    .next_step("Move the shared definitions into a separate file, or defer one of the imports to the place where it is used.");
    for member in &cycle.members {
        finding = finding.path(member.clone());
    }
    finding
}

/// Derives findings from a finished report.
pub fn architecture_findings(report: &ArchitectureReport, total_code_lines: u64) -> Vec<Finding> {
    let mut findings = Vec::new();
    for cycle in &report.cycles {
        if cycle.level == CycleLevel::Module {
            findings.push(module_cycle_finding(report, cycle));
        }
    }
    findings.extend(
        report
            .cycles
            .iter()
            .filter(|cycle| cycle.level == CycleLevel::File)
            .take(MAX_FILE_CYCLE_FINDINGS)
            .map(file_cycle_finding),
    );

    let module_count = report.modules.len();
    if module_count >= MIN_MODULES
        && let Some((module, share)) = largest_module(report, total_code_lines)
        && share >= CODE_CONCENTRATION
    {
        findings.push(
            Finding::new(
                "architecture.concentration",
                &module.id,
                FindingCategory::Architecture,
                Severity::Attention,
                Confidence::High,
                format!("{} holds {}% of the code", module.name, percent(share)),
            )
            .summary(format!(
                "{} of {} first-party code lines are in {}, one of {module_count} modules.",
                module.code_lines, total_code_lines, module.name
            ))
            .rationale("When most code lives in one module, its internal structure matters more than the module map suggests, and changes there affect a large part of the system.")
            .method("Code lines per inferred module divided by all first-party code lines.")
            .evidence(Evidence::metric_with_threshold(
                "architecture.module.code_share",
                share,
                CODE_CONCENTRATION,
                "ratio",
            ))
            .evidence(Evidence::directory(&module.path))
            .limitation("Module boundaries are inferred from manifests and directories and may not match how the team thinks about the code.")
            .next_step("Look for natural seams inside the module, such as subdirectories that change independently.")
            .path(module.path.clone()),
        );
    }

    if module_count >= MIN_MODULES {
        for module in report
            .modules
            .iter()
            .filter(|module| module.centrality >= HUB_CENTRALITY)
        {
            findings.push(
                Finding::new(
                    "architecture.centrality",
                    &module.id,
                    FindingCategory::Architecture,
                    Severity::Info,
                    Confidence::Medium,
                    format!("{} connects many parts of the system", module.name),
                )
                .summary(format!(
                    "{}% of the shortest dependency paths between other modules pass through {} (fan-in {}, fan-out {}).",
                    percent(module.centrality),
                    module.name,
                    module.fan_in,
                    module.fan_out
                ))
                .rationale("A module that sits between many others is a natural bottleneck: changes to it need broad review, and it is a good place to document contracts.")
                .method("Normalized betweenness centrality (Brandes' algorithm) of the module dependency graph.")
                .evidence(Evidence::metric_with_threshold(
                    "architecture.module.centrality",
                    module.centrality,
                    HUB_CENTRALITY,
                    "ratio",
                ))
                .evidence(Evidence::directory(&module.path))
                .limitation(LIMITATION)
                .next_step("Keep its interface small and documented, and check whether it mixes unrelated responsibilities.")
                .path(module.path.clone()),
            );
        }
    }

    if module_count >= 3 {
        let isolated = report
            .isolated_modules
            .iter()
            .filter_map(|id| report.module(id))
            .filter(|module| {
                module.kind != ModuleKind::Root
                    && !module
                        .path
                        .split('/')
                        .any(|part| CONSUMER_DIRS.contains(&part))
            })
            .take(MAX_ISOLATED_FINDINGS);
        for module in isolated {
            findings.push(
                Finding::new(
                    "architecture.isolated-module",
                    &module.id,
                    FindingCategory::Architecture,
                    Severity::Info,
                    Confidence::Low,
                    format!("{} has no detected links to other modules", module.name),
                )
                .summary(format!(
                    "No resolved import connects {} ({} files) with any other module, in either direction.",
                    module.name, module.files
                ))
                .rationale("Isolated code is either independent, connected in ways static analysis cannot see (configuration, runtime loading, code generation), or unused.")
                .method("Modules with zero fan-in and zero fan-out in the module dependency graph.")
                .evidence(Evidence::directory(&module.path).with_note("no resolved imports in or out"))
                .limitation(LIMITATION)
                .next_step("Confirm how the module is used; if it is loaded dynamically, document the mechanism.")
                .path(module.path.clone()),
            );
        }
    }

    let attempted = report.resolved_imports + report.unresolved_imports;
    if attempted >= MIN_IMPORTS_FOR_UNRESOLVED {
        let share = report.unresolved_imports as f64 / attempted as f64;
        if share >= UNRESOLVED_SHARE {
            findings.push(
                Finding::new(
                    "architecture.unresolved",
                    "imports",
                    FindingCategory::Architecture,
                    Severity::Info,
                    Confidence::High,
                    format!("{}% of repository imports could not be resolved", percent(share)),
                )
                .summary(format!(
                    "{} of {attempted} imports that look like repository imports matched no file.",
                    report.unresolved_imports
                ))
                .rationale("Unresolved imports leave the dependency graph incomplete, so cycles, layers, and centrality may be missing edges.")
                .method("Imports classified as repository imports (relative paths, crate paths, workspace packages) that no file matched, divided by all such imports.")
                .evidence(Evidence::metric("architecture.imports.resolved", report.resolved_imports as f64))
                .evidence(Evidence::metric_with_threshold(
                    "architecture.imports.unresolved_share",
                    share,
                    UNRESOLVED_SHARE,
                    "ratio",
                ))
                .limitation("Build-time path aliases, generated code, and custom module roots are not configured automatically.")
                .next_step("Check for path aliases or generated sources, or treat the architecture view as partial."),
            );
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::SectionStatus;
    use repodna_core::model::architecture::{Layer, ModuleEdge, PackageBoundary};

    fn module(id: &str, code_lines: u64, fan_in: u32, fan_out: u32) -> ModuleRecord {
        ModuleRecord {
            id: id.into(),
            name: paths::file_name(id).into(),
            path: id.into(),
            kind: ModuleKind::Directory,
            language: Some("python".into()),
            files: 3,
            code_lines,
            fan_in,
            fan_out,
            instability: 0.0,
            centrality: 0.0,
            layer: Some(0),
            inferred: true,
            confidence: Confidence::Medium,
            importance: Vec::new(),
            external_dependencies: Vec::new(),
            evidence: Vec::new(),
        }
    }

    fn edge(from: &str, to: &str) -> ModuleEdge {
        ModuleEdge {
            from: from.into(),
            to: to.into(),
            weight: 1,
            confidence: Confidence::High,
            samples: Vec::new(),
        }
    }

    fn layered_report() -> ArchitectureReport {
        ArchitectureReport {
            status: SectionStatus::Analyzed,
            modules: vec![
                module("app/api", 100, 0, 1),
                module("app/core", 100, 2, 0),
                module("app/services", 100, 1, 1),
                module("tests", 50, 0, 1),
            ],
            module_edges: vec![
                edge("app/api", "app/services"),
                edge("app/services", "app/core"),
                edge("tests", "app/core"),
            ],
            layers: vec![
                Layer {
                    index: 0,
                    modules: vec!["app/core".into()],
                },
                Layer {
                    index: 1,
                    modules: vec!["app/services".into(), "tests".into()],
                },
                Layer {
                    index: 2,
                    modules: vec!["app/api".into()],
                },
            ],
            ..ArchitectureReport::default()
        }
    }

    #[test]
    fn classifies_layered_and_monorepo_styles() {
        let report = layered_report();
        let (style, confidence, signals) = classify_style(&report, 350, 12);
        assert_eq!(style, "Layered");
        assert_eq!(confidence, Confidence::Medium);
        assert!(signals.iter().any(|signal| signal.id == "modular"));

        let mut monorepo = layered_report();
        monorepo.packages = ["a", "b"]
            .iter()
            .map(|name| PackageBoundary {
                name: (*name).into(),
                path: format!("packages/{name}"),
                ecosystem: "npm".into(),
                manifest: format!("packages/{name}/package.json"),
                internal_dependencies: Vec::new(),
            })
            .collect();
        assert_eq!(classify_style(&monorepo, 350, 12).0, "Monorepo");

        let empty = ArchitectureReport::default();
        assert_eq!(classify_style(&empty, 0, 0).0, "Unknown");
    }

    #[test]
    fn classifies_monolithic_and_flat_styles() {
        let mut report = layered_report();
        report.modules[0].code_lines = 2_000;
        report.layers.truncate(2);
        let (style, _, _) = classify_style(&report, 2_250, 40);
        assert_eq!(style, "Monolithic");

        let small = ArchitectureReport {
            modules: vec![module("src", 100, 0, 0)],
            ..ArchitectureReport::default()
        };
        assert_eq!(classify_style(&small, 100, 5).0, "Flat");
    }

    #[test]
    fn reports_cycles_with_language_aware_severity() {
        let mut report = layered_report();
        let cycle = |languages: &[&str]| DependencyCycle {
            id: "c".into(),
            level: CycleLevel::Module,
            members: vec!["app/api".into(), "app/services".into()],
            path: vec!["app/api".into(), "app/services".into(), "app/api".into()],
            languages: languages.iter().map(|l| (*l).to_owned()).collect(),
            confidence: Confidence::Medium,
            evidence: vec![Evidence::edge("app/api", "app/services")],
        };
        report.cycles = vec![cycle(&["python"])];
        let findings = architecture_findings(&report, 350);
        let found = findings
            .iter()
            .find(|finding| finding.rule == "architecture.cycle")
            .unwrap();
        assert_eq!(found.severity, Severity::Warning);
        assert_eq!(found.title, "Dependency cycle between api and services");
        assert!(found.summary.contains("api → services → api"));
        assert_eq!(found.paths, vec!["app/api", "app/services"]);

        report.cycles = vec![cycle(&["rust"])];
        let findings = architecture_findings(&report, 350);
        assert_eq!(findings[0].severity, Severity::Attention);
        assert!(findings[0].rationale.contains("compile time"));
    }

    #[test]
    fn reports_concentration_hubs_isolation_and_unresolved_imports() {
        let mut report = layered_report();
        report.modules[1].code_lines = 1_000;
        report.modules[2].centrality = 0.5;
        report.modules.push(module("plugins/legacy", 10, 0, 0));
        report.modules.push(module("examples", 10, 0, 0));
        report.isolated_modules = vec!["examples".into(), "plugins/legacy".into()];
        report.resolved_imports = 10;
        report.unresolved_imports = 15;
        let findings = architecture_findings(&report, 1_270);
        let rules: Vec<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
        assert_eq!(
            rules,
            vec![
                "architecture.concentration",
                "architecture.centrality",
                "architecture.isolated-module",
                "architecture.unresolved",
            ]
        );
        assert!(findings[0].title.starts_with("core holds 78.7%"));
        assert_eq!(findings[2].paths, vec!["plugins/legacy"]);
        assert!(findings[3].title.starts_with("60%"));
    }
}
