//! The Project DNA fingerprint, raw metrics, normalized signals, and section confidence.
//!
//! The fingerprint characterizes a repository along eight dimensions. None of them is a
//! quality score: a single-language repository is not better or worse than a polyglot one,
//! and an old repository is not better than a new one. Each dimension records its raw
//! measurement and the formula used to place it between 0 and 1.

use repodna_core::confidence::Confidence;
use repodna_core::evidence::finite;
use repodna_core::metric::{Metric, round4};
use repodna_core::model::SectionStatus;
use repodna_core::model::artifact::{RepositoryDna, compute_dna_hash};
use repodna_core::model::dependencies::ParseStatus;
use repodna_core::model::fingerprint::{DnaDimension, DnaFingerprint};
use repodna_core::model::metrics::{MetricsReport, NormalizedSignal, SectionConfidence};
use repodna_core::model::project::DocCheckStatus;

/// How the fingerprint is computed.
pub const FINGERPRINT_METHOD: &str = "The DNA hash is a SHA-256 digest of the schema major version and every discovered file's path and content hash (or size, when content was not read), truncated to 128 bits. It identifies a snapshot; it is not a security primitive. Each dimension is normalized to 0–1 with the formula in its description; dimensions characterize the repository and are not quality scores.";

/// Share of the most-changed files used for change concentration.
const TOP_FILE_SHARE: f64 = 0.1;

/// Share of resolved imports below which module-level measurements are low confidence.
const RELIABLE_RESOLUTION: f64 = 0.7;

fn dimension(
    id: &str,
    label: &str,
    value: f64,
    raw: f64,
    unit: &str,
    description: &str,
    confidence: Confidence,
) -> DnaDimension {
    DnaDimension {
        id: id.to_owned(),
        label: label.to_owned(),
        value: round4(finite(value).clamp(0.0, 1.0)),
        raw: round4(finite(raw)),
        unit: unit.to_owned(),
        description: description.to_owned(),
        confidence,
    }
}

/// The inverse Simpson index: the number of equally sized groups that would give the same
/// concentration. `None` when every weight is zero.
fn effective_count(weights: impl IntoIterator<Item = f64>) -> Option<f64> {
    let weights: Vec<f64> = weights.into_iter().filter(|weight| *weight > 0.0).collect();
    let total: f64 = weights.iter().sum();
    if total == 0.0 {
        return None;
    }
    let concentration: f64 = weights.iter().map(|w| (w / total).powi(2)).sum();
    Some(1.0 / concentration)
}

/// Share of imports that resolved, when any were attempted.
fn resolution_rate(dna: &RepositoryDna) -> Option<f64> {
    let architecture = &dna.architecture;
    let attempted = architecture.resolved_imports + architecture.unresolved_imports;
    (attempted > 0).then(|| architecture.resolved_imports as f64 / attempted as f64)
}

fn git_confidence(dna: &RepositoryDna) -> Confidence {
    if dna.git.shallow || dna.git.history_truncated {
        Confidence::Low
    } else {
        Confidence::High
    }
}

fn language_diversity(dna: &RepositoryDna) -> DnaDimension {
    const ID: &str = "language-diversity";
    const LABEL: &str = "Language diversity";
    const UNIT: &str = "effective languages";
    const DESCRIPTION: &str = "How evenly first-party code is spread across languages: 1 − Σ share², using each language's share of first-party code (data, prose, and build-file languages are excluded; when no language has a share, code lines of all languages are used). 0 means a single language; the raw value is the effective number of languages.";
    if !dna.languages.status.has_results() {
        return dimension(
            ID,
            LABEL,
            0.0,
            0.0,
            UNIT,
            DESCRIPTION,
            Confidence::Unavailable,
        );
    }
    let languages = &dna.languages.languages;
    let shares: Vec<f64> = languages
        .iter()
        .map(|language| language.share)
        .filter(|share| *share > 0.0)
        .collect();
    let weights = if shares.is_empty() {
        languages
            .iter()
            .map(|language| language.code_lines as f64)
            .collect()
    } else {
        shares
    };
    match effective_count(weights) {
        Some(count) => dimension(
            ID,
            LABEL,
            1.0 - 1.0 / count,
            count,
            UNIT,
            DESCRIPTION,
            Confidence::High,
        ),
        None => dimension(ID, LABEL, 0.0, 0.0, UNIT, DESCRIPTION, Confidence::Low),
    }
}

fn modularity(dna: &RepositoryDna) -> DnaDimension {
    const ID: &str = "modularity";
    const LABEL: &str = "Modularity";
    const UNIT: &str = "modules";
    const DESCRIPTION: &str = "How many modules the first-party code divides into, on a log scale: log2(modules) ÷ 6, capped at 1 (64 or more modules). Modules are inferred from packages and directories.";
    if !dna.architecture.status.has_results() {
        return dimension(
            ID,
            LABEL,
            0.0,
            0.0,
            UNIT,
            DESCRIPTION,
            Confidence::Unavailable,
        );
    }
    let modules = dna.architecture.modules.len();
    let confidence = if modules == 0 {
        Confidence::Low
    } else {
        Confidence::Medium
    };
    dimension(
        ID,
        LABEL,
        (modules.max(1) as f64).log2() / 6.0,
        modules as f64,
        UNIT,
        DESCRIPTION,
        confidence,
    )
}

fn dependency_centrality(dna: &RepositoryDna) -> DnaDimension {
    const ID: &str = "dependency-centrality";
    const LABEL: &str = "Dependency centrality";
    const UNIT: &str = "dependent modules";
    const DESCRIPTION: &str = "How strongly internal dependencies converge on one module: the number of modules that depend on the most depended-upon module, divided by the number of other modules.";
    if !dna.architecture.status.has_results() {
        return dimension(
            ID,
            LABEL,
            0.0,
            0.0,
            UNIT,
            DESCRIPTION,
            Confidence::Unavailable,
        );
    }
    let modules = &dna.architecture.modules;
    if modules.len() < 2 {
        return dimension(ID, LABEL, 0.0, 0.0, UNIT, DESCRIPTION, Confidence::Low);
    }
    let max_fan_in = modules.iter().map(|m| m.fan_in).max().unwrap_or(0);
    let confidence = if resolution_rate(dna).is_some_and(|rate| rate >= RELIABLE_RESOLUTION) {
        Confidence::Medium
    } else {
        Confidence::Low
    };
    dimension(
        ID,
        LABEL,
        f64::from(max_fan_in) / (modules.len() - 1) as f64,
        f64::from(max_fan_in),
        UNIT,
        DESCRIPTION,
        confidence,
    )
}

fn change_concentration(dna: &RepositoryDna) -> DnaDimension {
    const ID: &str = "change-concentration";
    const LABEL: &str = "Change concentration";
    const UNIT: &str = "share of file changes";
    const DESCRIPTION: &str = "How concentrated commits are in a few files: the share of file changes that touch the most-changed 10% of current files, rescaled so that evenly spread changes give 0 and changes confined to those files give 1.";
    if !dna.git.status.has_results() {
        return dimension(
            ID,
            LABEL,
            0.0,
            0.0,
            UNIT,
            DESCRIPTION,
            Confidence::Unavailable,
        );
    }
    let mut commits: Vec<u64> = dna
        .git
        .file_history
        .iter()
        .map(|record| u64::from(record.commits))
        .filter(|commits| *commits > 0)
        .collect();
    let total: u64 = commits.iter().sum();
    if commits.len() < 2 || total == 0 {
        return dimension(ID, LABEL, 0.0, 0.0, UNIT, DESCRIPTION, Confidence::Low);
    }
    commits.sort_unstable_by(|a, b| b.cmp(a));
    let files = commits.len();
    let top = ((files as f64 * TOP_FILE_SHARE).ceil() as usize).clamp(1, files - 1);
    let share = commits[..top].iter().sum::<u64>() as f64 / total as f64;
    let baseline = top as f64 / files as f64;
    let confidence = if files >= 10 {
        git_confidence(dna)
    } else {
        Confidence::Low
    };
    dimension(
        ID,
        LABEL,
        (share - baseline) / (1.0 - baseline),
        share,
        UNIT,
        DESCRIPTION,
        confidence,
    )
}

fn contributor_spread(dna: &RepositoryDna) -> DnaDimension {
    const ID: &str = "contributor-spread";
    const LABEL: &str = "Contributor spread";
    const UNIT: &str = "effective contributors";
    const DESCRIPTION: &str = "How evenly commits are spread across contributors: 1 − Σ share², with shares of commits per contributor identity. 0 means one contributor made every commit; the raw value is the effective number of contributors.";
    if !dna.git.status.has_results() {
        return dimension(
            ID,
            LABEL,
            0.0,
            0.0,
            UNIT,
            DESCRIPTION,
            Confidence::Unavailable,
        );
    }
    match effective_count(dna.git.contributors.iter().map(|c| c.commits as f64)) {
        Some(count) => dimension(
            ID,
            LABEL,
            1.0 - 1.0 / count,
            count,
            UNIT,
            DESCRIPTION,
            git_confidence(dna).min(Confidence::Medium),
        ),
        None => dimension(ID, LABEL, 0.0, 0.0, UNIT, DESCRIPTION, Confidence::Low),
    }
}

fn age(dna: &RepositoryDna) -> DnaDimension {
    const ID: &str = "age";
    const LABEL: &str = "Age";
    const UNIT: &str = "years";
    const DESCRIPTION: &str = "Time between the first and the latest commit, on a log scale: ln(1 + years) ÷ ln(21), capped at 1 (20 years or more).";
    let (Some(first), Some(last)) = (&dna.git.first_commit, &dna.git.last_commit) else {
        let confidence = if dna.git.status.has_results() {
            Confidence::Low
        } else {
            Confidence::Unavailable
        };
        return dimension(ID, LABEL, 0.0, 0.0, UNIT, DESCRIPTION, confidence);
    };
    let years = first.timestamp.days_until(last.timestamp).max(0) as f64 / 365.25;
    dimension(
        ID,
        LABEL,
        (1.0 + years).ln() / 21f64.ln(),
        years,
        UNIT,
        DESCRIPTION,
        git_confidence(dna),
    )
}

fn testing(dna: &RepositoryDna) -> DnaDimension {
    const ID: &str = "testing";
    const LABEL: &str = "Test presence";
    const UNIT: &str = "ratio";
    const DESCRIPTION: &str = "Share of test code among test and source code lines, doubled and capped at 1 (a ratio of 0.5 or more is 1). Tests are recognized by path and file-name conventions and inline test modules.";
    let tests = &dna.tests;
    if !tests.status.has_results() {
        return dimension(
            ID,
            LABEL,
            0.0,
            0.0,
            UNIT,
            DESCRIPTION,
            Confidence::Unavailable,
        );
    }
    let confidence = if tests.test_files + tests.source_files == 0 {
        Confidence::Low
    } else {
        Confidence::Medium
    };
    dimension(
        ID,
        LABEL,
        tests.test_ratio * 2.0,
        tests.test_ratio,
        UNIT,
        DESCRIPTION,
        confidence,
    )
}

fn documentation(dna: &RepositoryDna) -> DnaDimension {
    const ID: &str = "documentation";
    const LABEL: &str = "Documentation";
    const UNIT: &str = "checks met";
    const DESCRIPTION: &str = "Share of documentation checks that are met (README, license, installation and usage sections, contributing guide, changelog, documentation directory, and others); partial matches count half.";
    let checks = &dna.docs.checks;
    if !dna.docs.status.has_results() || checks.is_empty() {
        return dimension(
            ID,
            LABEL,
            0.0,
            0.0,
            UNIT,
            DESCRIPTION,
            Confidence::Unavailable,
        );
    }
    let met: f64 = checks
        .iter()
        .map(|check| match check.status {
            DocCheckStatus::Present => 1.0,
            DocCheckStatus::Partial => 0.5,
            DocCheckStatus::NotDetected => 0.0,
        })
        .sum();
    dimension(
        ID,
        LABEL,
        met / checks.len() as f64,
        met,
        UNIT,
        DESCRIPTION,
        Confidence::Medium,
    )
}

/// Computes the DNA hash and the characterization dimensions.
pub fn fingerprint(dna: &RepositoryDna) -> DnaFingerprint {
    DnaFingerprint {
        dna_hash: compute_dna_hash(&dna.structure),
        dimensions: vec![
            language_diversity(dna),
            modularity(dna),
            dependency_centrality(dna),
            change_concentration(dna),
            contributor_spread(dna),
            age(dna),
            testing(dna),
            documentation(dna),
        ],
        method: FINGERPRINT_METHOD.to_owned(),
    }
}

/// Collects documented raw metrics.
struct Collector(Vec<Metric>);

impl Collector {
    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        id: &str,
        label: &str,
        value: f64,
        unit: &str,
        definition: &str,
        method: &str,
        confidence: Confidence,
    ) {
        self.0.push(
            Metric::new(id, label, value, unit)
                .definition(definition)
                .method(method)
                .confidence(confidence),
        );
    }
}

fn raw_metrics(dna: &RepositoryDna) -> Vec<Metric> {
    use Confidence::{High, Low, Medium};
    let mut m = Collector(Vec::new());
    let structure = &dna.structure;
    if structure.status.has_results() {
        const DISCOVERY: &str = "Counted during file discovery after ignore rules.";
        m.add(
            "structure.files",
            "Files",
            structure.total_files as f64,
            "files",
            "Files in the analyzed tree.",
            DISCOVERY,
            High,
        );
        m.add(
            "structure.bytes",
            "Size",
            structure.total_bytes as f64,
            "bytes",
            "Total size of the files in the analyzed tree.",
            DISCOVERY,
            High,
        );
        m.add(
            "structure.lines.total",
            "Lines",
            structure.total_lines as f64,
            "lines",
            "Lines in the text files that were read.",
            "Counted per file; files over the size limit are not read.",
            High,
        );
        m.add(
            "structure.lines.code",
            "Code lines",
            structure.code_lines as f64,
            "lines",
            "Lines that are neither blank nor only a comment.",
            "Counted per file by the language-aware line counter; in files without a recognized language every non-blank line counts.",
            Medium,
        );
        m.add(
            "structure.lines.comment",
            "Comment lines",
            structure.comment_lines as f64,
            "lines",
            "Lines that contain only a comment.",
            "Counted per file by the language-aware line counter.",
            Medium,
        );
        m.add(
            "structure.directories",
            "Directories",
            structure.directories.len() as f64,
            "directories",
            "Directories containing analyzed files.",
            DISCOVERY,
            High,
        );
        m.add(
            "structure.files.generated",
            "Generated files",
            structure.generated_files as f64,
            "files",
            "Files recognized as generated.",
            "Recognized by path patterns and generated-code markers in the file header.",
            Medium,
        );
        m.add(
            "structure.files.vendored",
            "Vendored files",
            structure.vendored_files as f64,
            "files",
            "Files recognized as third-party code copied into the repository.",
            "Recognized by path patterns such as vendor/ and third_party/.",
            Medium,
        );
        m.add(
            "structure.files.binary",
            "Binary files",
            structure.binary_files as f64,
            "files",
            "Files that are not text.",
            "Recognized by extension or by NUL bytes in the first 8,000 bytes, as Git does.",
            High,
        );
        m.add(
            "structure.symbols",
            "Symbols",
            structure.symbols.len() as f64,
            "symbols",
            "Functions, types, and other named declarations listed in the artifact.",
            "Extracted by lexical declaration patterns per language; the list is capped.",
            Medium,
        );
        m.add(
            "structure.entrypoints",
            "Entrypoints",
            structure.entrypoints.len() as f64,
            "entrypoints",
            "Files where execution or use of the code starts.",
            "Detected from manifests and conventional file names.",
            Medium,
        );
    }
    let languages = &dna.languages;
    if languages.status.has_results() {
        m.add(
            "languages.count",
            "Languages",
            languages.languages.len() as f64,
            "languages",
            "Languages with at least one file.",
            "Detected from file names, extensions, and shebangs.",
            High,
        );
        if let Some(top) = languages.languages.first() {
            m.add(
                "languages.primary.share",
                "Primary language share",
                top.share,
                "ratio",
                "Share of code lines in the language with the most code lines.",
                "Code lines of the language divided by all code lines.",
                High,
            );
        }
    }
    let architecture = &dna.architecture;
    if architecture.status.has_results() {
        const INFERRED: &str = "Inferred from packages, directories, and resolved imports.";
        m.add(
            "architecture.modules",
            "Modules",
            architecture.modules.len() as f64,
            "modules",
            "Inferred modules of first-party code.",
            INFERRED,
            Medium,
        );
        m.add(
            "architecture.module-edges",
            "Module dependencies",
            architecture.module_edges.len() as f64,
            "edges",
            "Distinct dependencies between modules.",
            INFERRED,
            Medium,
        );
        m.add(
            "architecture.file-edges",
            "File dependencies",
            architecture.file_edges.len() as f64,
            "edges",
            "Resolved imports between files listed in the artifact.",
            "Static import statements resolved per language; the list can be capped.",
            Medium,
        );
        m.add(
            "architecture.layers",
            "Layers",
            architecture.layers.len() as f64,
            "layers",
            "Levels of the module dependency graph once cycles are collapsed.",
            "Longest-path layering of the condensed module graph.",
            Medium,
        );
        m.add(
            "architecture.cycles",
            "Dependency cycles",
            architecture.cycles.len() as f64,
            "cycles",
            "Module and file dependency cycles.",
            "Strongly connected components of the dependency graphs.",
            Medium,
        );
        m.add("architecture.external", "External packages imported", architecture.external.len() as f64, "packages", "Distinct external packages referenced by import statements.", "Import specifiers that did not resolve inside the repository and are not standard library.", Medium);
        m.add(
            "architecture.imports.resolved",
            "Resolved imports",
            architecture.resolved_imports as f64,
            "imports",
            "Import statements resolved to files or packages in the repository.",
            "Static resolution per language.",
            Medium,
        );
        m.add(
            "architecture.imports.unresolved",
            "Unresolved imports",
            architecture.unresolved_imports as f64,
            "imports",
            "Import statements that looked internal but could not be resolved.",
            "Static resolution per language.",
            Medium,
        );
        if let Some(rate) = resolution_rate(dna) {
            m.add(
                "architecture.imports.resolution-rate",
                "Import resolution rate",
                rate,
                "ratio",
                "Share of attempted import resolutions that succeeded.",
                "Resolved imports divided by resolved plus unresolved imports.",
                Medium,
            );
        }
    }
    let git = &dna.git;
    if git.status.has_results() {
        let history = git_confidence(dna);
        m.add(
            "git.commits",
            "Commits",
            git.commit_count as f64,
            "commits",
            "Commits reachable from HEAD.",
            "Read from git log, including merges.",
            history,
        );
        m.add("git.contributors", "Contributors", git.contributors.len() as f64, "contributors", "Distinct commit author identities.", "Authors grouped by normalized e-mail address (or name when there is none) after Git applies .mailmap.", Medium.min(history));
        m.add(
            "git.branches",
            "Branches",
            git.branches.len() as f64,
            "branches",
            "Local and remote-tracking branches.",
            "Read from git for-each-ref.",
            High,
        );
        m.add(
            "git.tags",
            "Tags",
            git.tags.len() as f64,
            "tags",
            "Tags in the repository.",
            "Read from git for-each-ref.",
            High,
        );
        m.add(
            "git.releases",
            "Releases",
            git.releases.len() as f64,
            "releases",
            "Tags that look like version numbers.",
            "Tag names parsed as versions.",
            Medium,
        );
        m.add(
            "git.commits.last-90-days",
            "Commits in the last 90 days",
            git.activity.commits_last_90_days as f64,
            "commits",
            "Commits in the 90 days before the reference time.",
            "Counted from commit dates.",
            High,
        );
        m.add(
            "git.ownership.top-share",
            "Top contributor share",
            git.ownership.top_contributor_share,
            "ratio",
            "Share of commits made by the most active contributor.",
            "Commits of the top contributor divided by all commits.",
            history,
        );
        m.add("git.hotspots", "Hotspots", git.hot_spots.len() as f64, "files", "Files ranked as change hotspots.", "Weighted percentiles of commits, churn, recent commits, complexity, size, authors, and dependents.", Medium);
        if let (Some(first), Some(last)) = (&git.first_commit, &git.last_commit) {
            m.add(
                "git.age.days",
                "History span",
                first.timestamp.days_until(last.timestamp).max(0) as f64,
                "days",
                "Days between the first and the latest commit.",
                "Commit dates of the first and latest commits.",
                history,
            );
        }
    }
    let quality = &dna.code_quality;
    if quality.status.has_results() {
        let complexity = &quality.complexity;
        m.add(
            "quality.functions",
            "Functions analyzed",
            complexity.functions_analyzed as f64,
            "functions",
            "Functions whose boundaries and complexity could be measured.",
            &complexity.method,
            Medium,
        );
        m.add(
            "quality.cyclomatic.average",
            "Average cyclomatic complexity",
            complexity.average_cyclomatic,
            "paths",
            "Mean cyclomatic complexity per function.",
            &complexity.method,
            Medium,
        );
        m.add(
            "quality.cyclomatic.median",
            "Median cyclomatic complexity",
            complexity.median_cyclomatic,
            "paths",
            "Median cyclomatic complexity per function.",
            &complexity.method,
            Medium,
        );
        m.add(
            "quality.files.large",
            "Large files",
            quality.large_files.len() as f64,
            "files",
            "Files above the large-file threshold.",
            "Code lines compared with the configured threshold.",
            High,
        );
        m.add(
            "quality.functions.large",
            "Large functions",
            quality.large_functions.len() as f64,
            "functions",
            "Functions above the long-function threshold.",
            "Function lines compared with the configured threshold.",
            Medium,
        );
        m.add(
            "quality.markers",
            "Work markers",
            quality.markers.total as f64,
            "markers",
            "TODO, FIXME, HACK, XXX, BUG, and deprecation markers in comments.",
            "Matched in comments by the lexical scanner.",
            High,
        );
        m.add(
            "quality.dead-code-candidates",
            "Dead-code candidates",
            quality.dead_code_candidates.len() as f64,
            "candidates",
            "Files or modules that nothing appears to use.",
            "Import graph, entrypoints, and history combined; candidates need manual review.",
            Low,
        );
        let duplication = &quality.duplication;
        if duplication.status.has_results() {
            m.add(
                "quality.duplication.ratio",
                "Duplicated lines",
                duplication.ratio,
                "ratio",
                "Share of analyzed code lines inside duplicated blocks.",
                &duplication.method,
                Medium,
            );
            m.add(
                "quality.duplication.clusters",
                "Duplicate clusters",
                duplication.clusters.len() as f64,
                "clusters",
                "Groups of code blocks that repeat the same token sequence.",
                &duplication.method,
                Medium,
            );
        }
    }
    let dependencies = &dna.dependencies;
    if dependencies.status.has_results() {
        let parsed = if dependencies
            .manifests
            .iter()
            .all(|m| m.status == ParseStatus::Parsed)
        {
            High
        } else {
            Medium
        };
        m.add(
            "dependencies.direct",
            "Direct dependencies",
            dependencies.direct_count as f64,
            "dependencies",
            "Dependencies declared in manifests, excluding workspace-internal packages.",
            "Parsed from manifests per ecosystem.",
            parsed,
        );
        m.add(
            "dependencies.locked",
            "Locked packages",
            dependencies.locked_count as f64,
            "packages",
            "Packages pinned in lockfiles, including transitive ones.",
            "Parsed from lockfiles per ecosystem.",
            parsed,
        );
        m.add(
            "dependencies.manifests",
            "Manifests",
            dependencies.manifests.len() as f64,
            "files",
            "Manifest and lockfile files recognized.",
            "Recognized by file name per ecosystem.",
            High,
        );
        m.add(
            "dependencies.ecosystems",
            "Ecosystems",
            dependencies.ecosystems.len() as f64,
            "ecosystems",
            "Package ecosystems with at least one manifest or lockfile.",
            "Recognized by file name per ecosystem.",
            High,
        );
        m.add(
            "dependencies.duplicates",
            "Packages at several versions",
            dependencies.duplicates.len() as f64,
            "packages",
            "Packages locked at more than one version.",
            "Lockfile entries grouped by package name.",
            High,
        );
        if let Some(top) = dependencies.concentration.first() {
            m.add(
                "dependencies.concentration.share",
                "Most imported package share",
                top.share,
                "ratio",
                "Share of external-package imports that go to the most imported package.",
                "Importing files per package divided by all importing files.",
                Medium,
            );
        }
    }
    let tests = &dna.tests;
    if tests.status.has_results() {
        const CONVENTIONS: &str =
            "Recognized by path and file-name conventions and inline test modules.";
        m.add(
            "tests.files",
            "Test files",
            tests.test_files as f64,
            "files",
            "Files that contain tests.",
            CONVENTIONS,
            Medium,
        );
        m.add(
            "tests.lines",
            "Test lines",
            tests.test_lines as f64,
            "lines",
            "Code lines in test files.",
            CONVENTIONS,
            Medium,
        );
        m.add(
            "tests.ratio",
            "Test code ratio",
            tests.test_ratio,
            "ratio",
            "Test code lines divided by test and source code lines.",
            CONVENTIONS,
            Medium,
        );
        m.add(
            "tests.frameworks",
            "Test frameworks",
            tests.frameworks.len() as f64,
            "frameworks",
            "Test frameworks detected.",
            "Detected from dependencies, configuration files, and imports.",
            Medium,
        );
    }
    let builds = &dna.builds;
    if builds.status.has_results() {
        m.add(
            "builds.systems",
            "Build systems",
            builds.systems.len() as f64,
            "systems",
            "Build systems and package managers detected.",
            "Detected from manifests and build files.",
            High,
        );
        m.add(
            "builds.ci",
            "CI configurations",
            builds.ci.len() as f64,
            "files",
            "CI/CD configuration files.",
            "Recognized by path per CI provider.",
            High,
        );
        m.add(
            "builds.commands",
            "Build and run commands",
            builds.commands.len() as f64,
            "commands",
            "Candidate commands detected from metadata; not verified unless executed.",
            "Read from manifests, task files, and README code blocks.",
            Medium,
        );
    }
    let docs = &dna.docs;
    if docs.status.has_results() {
        m.add(
            "docs.files",
            "Documentation files",
            docs.doc_files as f64,
            "files",
            "Documentation files, including the README.",
            "Classified by extension and location.",
            High,
        );
        m.add(
            "docs.lines",
            "Documentation lines",
            docs.doc_lines as f64,
            "lines",
            "Lines in documentation files.",
            "Counted per file.",
            High,
        );
    }
    let security = &dna.security;
    if security.status.has_results() {
        m.add(
            "security.files-scanned",
            "Files scanned",
            security.files_scanned as f64,
            "files",
            "Text files scanned for secret and risky-pattern candidates.",
            "Every readable text file within the size limit.",
            High,
        );
        m.add(
            "security.secret-candidates",
            "Secret candidates",
            security.secrets.len() as f64,
            "candidates",
            "Values that look like credentials; values are never stored.",
            "Regular expressions with entropy and placeholder filters.",
            Low,
        );
        m.add(
            "security.pattern-candidates",
            "Risky-pattern candidates",
            security.patterns.len() as f64,
            "candidates",
            "Code, configuration, and workflow patterns that often need review.",
            "Regular expressions per language and file type.",
            Low,
        );
    }
    let evolution = &dna.evolution;
    if evolution.status.has_results() {
        m.add(
            "evolution.epochs",
            "Epochs",
            evolution.epochs.len() as f64,
            "epochs",
            "Periods of distinct commit activity.",
            "Commit rate per year compared with the median, split at long gaps.",
            Medium,
        );
        m.add(
            "evolution.events",
            "Evolution events",
            evolution.events.len() as f64,
            "events",
            "Detected milestones such as new modules, languages, and frameworks.",
            "Differences between sampled snapshots of the history.",
            Medium,
        );
        m.add(
            "evolution.snapshots",
            "Snapshots",
            evolution.snapshots.len() as f64,
            "snapshots",
            "Historical revisions sampled for the Time Machine.",
            &evolution.sampling,
            High,
        );
    }
    let counts = dna.finding_counts();
    const FINDINGS: &str = "Counted over active findings after suppressions.";
    m.add(
        "findings.critical",
        "Critical findings",
        counts.critical as f64,
        "findings",
        "Active findings of critical severity.",
        FINDINGS,
        High,
    );
    m.add(
        "findings.warning",
        "Warnings",
        counts.warning as f64,
        "findings",
        "Active findings of warning severity.",
        FINDINGS,
        High,
    );
    m.add(
        "findings.attention",
        "Attention signals",
        counts.attention as f64,
        "findings",
        "Active findings of attention severity.",
        FINDINGS,
        High,
    );
    m.add(
        "findings.info",
        "Informational findings",
        counts.info as f64,
        "findings",
        "Active informational findings.",
        FINDINGS,
        High,
    );
    m.add(
        "findings.suppressed",
        "Suppressed findings",
        counts.suppressed as f64,
        "findings",
        "Findings suppressed by configuration.",
        "Counted over suppressed findings.",
        High,
    );
    let mut metrics = m.0;
    metrics.sort_by(|a, b| a.id.cmp(&b.id));
    metrics
}

fn interpretation(dimension: &DnaDimension) -> String {
    if dimension.confidence == Confidence::Unavailable {
        return "Not measured: the underlying analysis did not run.".to_owned();
    }
    let value = dimension.value;
    let band = |low: &str, middle: &str, high: &str| -> String {
        if value < 0.34 {
            low.to_owned()
        } else if value < 0.67 {
            middle.to_owned()
        } else {
            high.to_owned()
        }
    };
    match dimension.id.as_str() {
        "language-diversity" => band(
            "Code is mostly in one language.",
            "Code is split across a few languages.",
            "Code is spread across many languages.",
        ),
        "modularity" => band(
            "Few modules.",
            "A moderate number of modules.",
            "Many modules.",
        ),
        "dependency-centrality" => band(
            "No module is a strong hub.",
            "Some modules depend on a shared hub.",
            "Most modules depend on one hub module.",
        ),
        "change-concentration" => band(
            "Changes are spread across many files.",
            "Changes lean towards a subset of files.",
            "Changes concentrate in a few files.",
        ),
        "contributor-spread" => band(
            "Most commits come from one contributor.",
            "Commits come from a few contributors.",
            "Commits are spread across many contributors.",
        ),
        "age" => band(
            "A young history.",
            "Several years of history.",
            "A long history.",
        ),
        "testing" => band(
            "Little or no test code was detected.",
            "A moderate share of test code.",
            "A large share of test code.",
        ),
        "documentation" => band(
            "Few documentation conventions were detected.",
            "Some documentation conventions were detected.",
            "Most documentation conventions were detected.",
        ),
        _ => String::new(),
    }
}

fn signals(dna: &RepositoryDna, fingerprint: &DnaFingerprint) -> Vec<NormalizedSignal> {
    let mut signals: Vec<NormalizedSignal> = fingerprint
        .dimensions
        .iter()
        .map(|dimension| NormalizedSignal {
            id: dimension.id.clone(),
            label: dimension.label.clone(),
            value: dimension.value,
            interpretation: interpretation(dimension),
            confidence: dimension.confidence,
        })
        .collect();
    if dna.git.status.has_results() {
        let recent = dna.git.activity.commits_last_90_days as f64;
        signals.push(NormalizedSignal {
            id: "recent-activity".into(),
            label: "Recent activity".into(),
            value: round4(((1.0 + recent).ln() / 101f64.ln()).min(1.0)),
            interpretation: dna.git.activity.level.label().to_owned(),
            confidence: Confidence::High,
        });
    }
    let duplication = &dna.code_quality.duplication;
    if duplication.status.has_results() {
        signals.push(NormalizedSignal {
            id: "duplication".into(),
            label: "Duplication".into(),
            value: round4(duplication.ratio.clamp(0.0, 1.0)),
            interpretation: format!(
                "{:.1}% of analyzed code lines are in duplicated blocks.",
                duplication.ratio * 100.0
            ),
            confidence: Confidence::Medium,
        });
    }
    if let Some(rate) = resolution_rate(dna) {
        signals.push(NormalizedSignal {
            id: "import-resolution".into(),
            label: "Import resolution".into(),
            value: round4(rate),
            interpretation: format!(
                "{:.0}% of imports that looked internal were resolved; lower rates make dependency results less complete.",
                rate * 100.0
            ),
            confidence: Confidence::High,
        });
    }
    signals
}

fn status_confidence(
    section: &str,
    status: SectionStatus,
    notes: &[String],
    analyzed: (Confidence, String),
) -> SectionConfidence {
    let (confidence, reason) = match status {
        SectionStatus::Analyzed => analyzed,
        SectionStatus::Partial => (
            analyzed.0.min(Confidence::Medium),
            format!(
                "{} Some inputs could not be processed{}",
                analyzed.1,
                notes
                    .first()
                    .map_or_else(|| ".".to_owned(), |note| format!(": {note}"))
            ),
        ),
        SectionStatus::Skipped | SectionStatus::Unavailable => (
            Confidence::Unavailable,
            notes
                .first()
                .cloned()
                .unwrap_or_else(|| format!("{}.", status.label())),
        ),
    };
    SectionConfidence {
        section: section.to_owned(),
        confidence,
        reason,
    }
}

fn section_confidence(dna: &RepositoryDna) -> Vec<SectionConfidence> {
    let structure = &dna.structure;
    let structure_reason = if structure.skipped_files > 0 {
        (
            Confidence::Medium,
            format!(
                "{} files were over the size limit or unreadable, so their contents were not analyzed.",
                structure.skipped_files
            ),
        )
    } else if structure.truncated {
        (
            Confidence::Medium,
            "Some lists were truncated; see the section notes.".to_owned(),
        )
    } else {
        (
            Confidence::High,
            "Every discovered file was classified.".to_owned(),
        )
    };
    let resolution = resolution_rate(dna);
    let architecture_reason = match resolution {
        Some(rate) if rate < 0.5 => (
            Confidence::Low,
            format!("Only {:.0}% of internal-looking imports were resolved, so module dependencies are incomplete.", rate * 100.0),
        ),
        Some(rate) => (
            Confidence::Medium,
            format!("Modules are inferred from packages and directories; {:.0}% of internal-looking imports were resolved.", rate * 100.0),
        ),
        None => (
            Confidence::Low,
            "Modules are inferred from packages and directories; no imports were available to confirm dependencies.".to_owned(),
        ),
    };
    let git = &dna.git;
    let git_reason = if git.shallow {
        (
            Confidence::Low,
            "The clone is shallow, so older history is missing.".to_owned(),
        )
    } else if git.history_truncated {
        (
            Confidence::Low,
            "History was limited to the most recent commits.".to_owned(),
        )
    } else {
        (
            Confidence::High,
            "Read directly from the Git history.".to_owned(),
        )
    };
    let dependency_reason = if dna
        .dependencies
        .manifests
        .iter()
        .all(|m| m.status == ParseStatus::Parsed)
    {
        (
            Confidence::High,
            "Every manifest and lockfile was parsed.".to_owned(),
        )
    } else {
        (
            Confidence::Medium,
            "Some manifests or lockfiles could only be parsed partially.".to_owned(),
        )
    };
    let evolution_reason = (
        git_reason.0.min(Confidence::Medium),
        "Built from sampled snapshots of the history; changes between samples are summarized, not replayed.".to_owned(),
    );
    vec![
        status_confidence("structure", structure.status, &structure.notes, structure_reason),
        status_confidence("languages", dna.languages.status, &dna.languages.notes, (Confidence::High, "Detected from file names, extensions, and shebangs.".to_owned())),
        status_confidence("architecture", dna.architecture.status, &dna.architecture.notes, architecture_reason),
        status_confidence("git", git.status, &git.notes, git_reason),
        status_confidence("codeQuality", dna.code_quality.status, &dna.code_quality.notes, (Confidence::Medium, "Measured with lexical analysis, not full parsing.".to_owned())),
        status_confidence("dependencies", dna.dependencies.status, &dna.dependencies.notes, dependency_reason),
        status_confidence("tests", dna.tests.status, &dna.tests.notes, (Confidence::Medium, "Tests are recognized by path and file-name conventions.".to_owned())),
        status_confidence("builds", dna.builds.status, &dna.builds.notes, (Confidence::Medium, "Build systems and commands are detected from metadata; commands are not verified unless executed.".to_owned())),
        status_confidence("docs", dna.docs.status, &dna.docs.notes, (Confidence::Medium, "Documentation is recognized by file names, headings, and locations.".to_owned())),
        status_confidence("security", dna.security.status, &dna.security.notes, (Confidence::Low, "Pattern-based scanning finds candidates for review, not confirmed vulnerabilities.".to_owned())),
        status_confidence("evolution", dna.evolution.status, &dna.evolution.notes, evolution_reason),
        status_confidence("similarity", dna.similarity.status, &dna.similarity.notes, (Confidence::Medium, "Similarity is estimated with MinHash signatures.".to_owned())),
    ]
}

/// Builds the metrics section for an artifact whose other sections are complete.
pub fn metrics(dna: &RepositoryDna, fingerprint: &DnaFingerprint) -> MetricsReport {
    MetricsReport {
        raw: raw_metrics(dna),
        signals: signals(dna, fingerprint),
        confidence: section_confidence(dna),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::architecture::{ModuleKind, ModuleRecord};
    use repodna_core::model::git::{CommitRef, ContributorRecord, FileHistoryRecord};
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::languages::{LanguageKind, LanguageStat, ParserCapability};
    use repodna_core::model::metadata::AnalysisMetadata;
    use repodna_core::time::Timestamp;

    fn language(id: &str, kind: LanguageKind, code_lines: u64, share: f64) -> LanguageStat {
        LanguageStat {
            id: id.into(),
            name: id.into(),
            kind,
            files: 1,
            bytes: code_lines * 10,
            code_lines,
            comment_lines: 0,
            blank_lines: 0,
            share,
            capability: ParserCapability::Lexical,
        }
    }

    fn module(id: &str, fan_in: u32) -> ModuleRecord {
        ModuleRecord {
            id: id.into(),
            name: id.into(),
            path: id.into(),
            kind: ModuleKind::Directory,
            language: None,
            files: 1,
            code_lines: 10,
            fan_in,
            fan_out: 0,
            instability: 0.0,
            centrality: 0.0,
            layer: None,
            inferred: true,
            confidence: Confidence::Medium,
            importance: Vec::new(),
            external_dependencies: Vec::new(),
            evidence: Vec::new(),
        }
    }

    fn at(year: i64) -> CommitRef {
        CommitRef {
            hash: "0".repeat(40),
            short: "0000000".into(),
            timestamp: Timestamp::from_ymd(year, 1, 1).unwrap(),
        }
    }

    fn contributor(id: &str, commits: u64) -> ContributorRecord {
        ContributorRecord {
            id: id.into(),
            name: id.into(),
            commits,
            insertions: 0,
            deletions: 0,
            first_commit: Timestamp::UNIX_EPOCH,
            last_commit: Timestamp::UNIX_EPOCH,
            active_days: 1,
            areas: Vec::new(),
        }
    }

    fn history(path: &str, commits: u32) -> FileHistoryRecord {
        FileHistoryRecord {
            path: path.into(),
            commits,
            authors: 1,
            insertions: 0,
            deletions: 0,
            first_seen: Timestamp::UNIX_EPOCH,
            last_changed: Timestamp::UNIX_EPOCH,
            recent_commits: 0,
            previous_paths: Vec::new(),
        }
    }

    fn artifact() -> RepositoryDna {
        let mut dna =
            RepositoryDna::new(RepositoryIdentity::default(), AnalysisMetadata::default());
        dna.languages.status = SectionStatus::Analyzed;
        dna.languages.languages = vec![
            language("rust", LanguageKind::Programming, 500, 0.5),
            language("python", LanguageKind::Programming, 500, 0.5),
            language("markdown", LanguageKind::Prose, 5000, 0.0),
        ];
        dna.architecture.status = SectionStatus::Analyzed;
        dna.architecture.modules = vec![
            module("a", 3),
            module("b", 0),
            module("c", 1),
            module("d", 0),
        ];
        dna.architecture.resolved_imports = 9;
        dna.architecture.unresolved_imports = 1;
        dna.git.status = SectionStatus::Analyzed;
        dna.git.first_commit = Some(at(2016));
        dna.git.last_commit = Some(at(2026));
        dna.git.contributors = vec![contributor("a", 50), contributor("b", 50)];
        dna.git.file_history = (0..20)
            .map(|i| history(&format!("f{i}"), if i == 0 { 81 } else { 1 }))
            .collect();
        dna.tests.status = SectionStatus::Analyzed;
        dna.tests.test_ratio = 0.2;
        dna.tests.test_files = 2;
        dna.tests.source_files = 8;
        dna
    }

    fn value(fingerprint: &DnaFingerprint, id: &str) -> (f64, f64, Confidence) {
        let d = fingerprint.dimension(id).unwrap();
        (d.value, d.raw, d.confidence)
    }

    #[test]
    fn characterizes_the_repository() {
        let dna = artifact();
        let fp = fingerprint(&dna);
        assert!(fp.dna_hash.starts_with("rdna1-"));
        assert_eq!(fp.dimensions.len(), 8);
        // Two programming languages with equal code: prose is ignored.
        assert_eq!(
            value(&fp, "language-diversity"),
            (0.5, 2.0, Confidence::High)
        );
        assert_eq!(
            value(&fp, "modularity"),
            (round4(4f64.log2() / 6.0), 4.0, Confidence::Medium)
        );
        assert_eq!(
            value(&fp, "dependency-centrality"),
            (1.0, 3.0, Confidence::Medium)
        );
        // Top 2 of 20 files hold 82 of 100 changes; the even baseline is 0.1.
        let (concentration, share, _) = value(&fp, "change-concentration");
        assert_eq!(share, 0.82);
        assert_eq!(concentration, round4((0.82 - 0.1) / 0.9));
        assert_eq!(
            value(&fp, "contributor-spread"),
            (0.5, 2.0, Confidence::Medium)
        );
        let (age, years, _) = value(&fp, "age");
        assert!((years - 10.0).abs() < 0.01);
        assert!(age > 0.7 && age < 0.9);
        assert_eq!(value(&fp, "testing"), (0.4, 0.2, Confidence::Medium));
        assert_eq!(value(&fp, "documentation").2, Confidence::Unavailable);
        assert!(fp.dimensions.iter().all(|d| (0.0..=1.0).contains(&d.value)));
    }

    #[test]
    fn marks_missing_analyses_unavailable() {
        let dna = RepositoryDna::new(RepositoryIdentity::default(), AnalysisMetadata::default());
        let fp = fingerprint(&dna);
        assert!(
            fp.dimensions
                .iter()
                .all(|d| d.confidence == Confidence::Unavailable && d.value == 0.0)
        );
        let report = metrics(&dna, &fp);
        assert!(
            report
                .signals
                .iter()
                .all(|s| s.interpretation.starts_with("Not measured"))
        );
        assert!(
            report
                .confidence
                .iter()
                .all(|c| c.confidence == Confidence::Unavailable)
        );
        assert!(report.raw.iter().all(|m| m.id.starts_with("findings.")));
    }

    #[test]
    fn documents_metrics_signals_and_confidence() {
        let mut dna = artifact();
        dna.git.shallow = true;
        let fp = fingerprint(&dna);
        let report = metrics(&dna, &fp);
        let ids: Vec<&str> = report.raw.iter().map(|m| m.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
        assert!(ids.contains(&"architecture.modules"));
        assert!(ids.contains(&"tests.ratio"));
        assert!(
            report
                .raw
                .iter()
                .all(|m| !m.definition.is_empty() && !m.method.is_empty())
        );
        assert_eq!(
            report
                .get("architecture.imports.resolution-rate")
                .unwrap()
                .value,
            0.9
        );
        let git = report
            .confidence
            .iter()
            .find(|c| c.section == "git")
            .unwrap();
        assert_eq!(git.confidence, Confidence::Low);
        assert!(git.reason.contains("shallow"));
        assert_eq!(value(&fp, "age").2, Confidence::Low);
        let signal_ids: Vec<&str> = report.signals.iter().map(|s| s.id.as_str()).collect();
        assert!(signal_ids.contains(&"recent-activity"));
        assert!(signal_ids.contains(&"import-resolution"));
    }
}
