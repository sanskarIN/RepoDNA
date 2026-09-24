//! Evidence drawn from the repository-wide sections of the analysis.

use repodna_core::finding::Finding;
use repodna_core::model::artifact::RepositoryDna;
use repodna_core::model::dependencies::DependencyScope;
use repodna_core::model::insights::ChangeKind;
use repodna_core::paths;

use super::{Candidates, EvidenceKind};

fn scope_label(scope: DependencyScope) -> &'static str {
    match scope {
        DependencyScope::Runtime => "runtime",
        DependencyScope::Development => "development",
        DependencyScope::Build => "build",
        DependencyScope::Optional => "optional",
        DependencyScope::Peer => "peer",
    }
}

fn change_label(change: ChangeKind) -> &'static str {
    match change {
        ChangeKind::Added => "added",
        ChangeKind::Removed => "removed",
        ChangeKind::Changed => "changed",
    }
}

pub(super) fn percent(share: f64) -> String {
    format!("{:.0}%", share * 100.0)
}

pub(super) fn plural(count: u64, word: &str) -> String {
    if count == 1 {
        format!("1 {word}")
    } else {
        format!("{count} {word}s")
    }
}

pub(super) fn is_within(path: &str, dir: &str) -> bool {
    dir.is_empty() || path == dir || paths::is_within(path, dir)
}

pub(super) fn overview(dna: &RepositoryDna, c: &mut Candidates) {
    let identity = &dna.identity;
    let mut detail = format!("Repository name: {}.", identity.name);
    if let Some(description) = &identity.description {
        detail.push_str(&format!(
            " Description found in the repository: {description}"
        ));
    }
    c.push(EvidenceKind::Fact, "identity", detail);
    let languages: Vec<String> = dna
        .languages
        .languages
        .iter()
        .filter(|language| language.share >= 0.01)
        .take(6)
        .map(|language| format!("{} {}", language.name, percent(language.share)))
        .collect();
    if !languages.is_empty() {
        c.push(
            EvidenceKind::Metric,
            "languages",
            format!(
                "Code by language (share of code lines): {}.",
                languages.join(", ")
            ),
        );
    }
    let s = &dna.structure;
    if s.total_files > 0 {
        c.push(
            EvidenceKind::Metric,
            "size",
            format!(
                "{} and {} lines of code; size class {}.",
                plural(s.total_files, "file"),
                s.code_lines,
                s.size_class.label()
            ),
        );
    }
    if let Some(license) = &dna.docs.license {
        let spdx = license.spdx.as_deref().unwrap_or("not identified");
        c.push(
            EvidenceKind::File,
            &license.path,
            format!("License file; SPDX identifier {spdx}."),
        );
    }
    let a = &dna.architecture;
    if !a.style.is_empty() {
        c.push(
            EvidenceKind::Fact,
            "architecture style",
            format!(
                "Inferred architecture style: {} ({} confidence), {} modules.",
                a.style,
                a.style_confidence.label(),
                a.modules.len()
            ),
        );
    }
    let g = &dna.git;
    if g.commit_count > 0 {
        let span = match (&g.first_commit, &g.last_commit) {
            (Some(first), Some(last)) => format!(
                ", from {} to {}",
                first.timestamp.date_string(),
                last.timestamp.date_string()
            ),
            _ => String::new(),
        };
        c.push(
            EvidenceKind::Fact,
            "history",
            format!(
                "{}{span} by {}. {}",
                plural(g.commit_count, "commit"),
                plural(u64::from(g.ownership.contributors), "contributor"),
                g.activity.description
            ),
        );
    }
}

pub(super) fn first_look(dna: &RepositoryDna, c: &mut Candidates) {
    for answer in &dna.insights.first_look {
        c.push(
            EvidenceKind::Fact,
            &answer.question,
            format!(
                "{} ({} confidence)",
                answer.answer,
                answer.confidence.label()
            ),
        );
    }
}

pub(super) fn entrypoints(dna: &RepositoryDna, c: &mut Candidates, limit: usize) {
    for entry in dna.structure.entrypoints.iter().take(limit) {
        c.push(
            EvidenceKind::File,
            &entry.path,
            format!("Entry point: {}", entry.reason),
        );
    }
}

pub(super) fn commands(dna: &RepositoryDna, c: &mut Candidates, limit: usize) {
    let commands = dna.builds.commands.iter().chain(&dna.tests.commands);
    let mut seen = Vec::new();
    for command in commands {
        if seen.contains(&command.command) || seen.len() == limit {
            continue;
        }
        seen.push(command.command.clone());
        let place = if command.working_directory.is_empty() {
            String::new()
        } else {
            format!(" in {}", command.working_directory)
        };
        c.push(
            EvidenceKind::Command,
            &command.command,
            format!(
                "Detected {} command{place} (from {}; not executed).",
                command.purpose.label(),
                command.source
            ),
        );
    }
}

pub(super) fn modules(
    dna: &RepositoryDna,
    c: &mut Candidates,
    limit: usize,
    filter: impl Fn(&str) -> bool,
) {
    let mut modules: Vec<_> = dna
        .architecture
        .modules
        .iter()
        .filter(|module| filter(&module.path))
        .collect();
    modules.sort_by(|a, b| {
        b.fan_in
            .cmp(&a.fan_in)
            .then(b.code_lines.cmp(&a.code_lines))
            .then(a.path.cmp(&b.path))
    });
    for module in modules.into_iter().take(limit) {
        let mut detail = format!(
            "Module {}: {}, {} lines of code; used by {} other modules, uses {}.",
            module.name,
            plural(module.files, "file"),
            module.code_lines,
            module.fan_in,
            module.fan_out
        );
        if let Some(language) = &module.language {
            detail.push_str(&format!(" Main language: {language}."));
        }
        if let Some(layer) = module.layer {
            detail.push_str(&format!(" Layer {layer}."));
        }
        if !module.importance.is_empty() {
            detail.push_str(&format!(
                " Why it matters: {}.",
                module.importance.join("; ")
            ));
        }
        if !module.external_dependencies.is_empty() {
            let shown: Vec<&str> = module
                .external_dependencies
                .iter()
                .take(8)
                .map(String::as_str)
                .collect();
            detail.push_str(&format!(" External packages used: {}.", shown.join(", ")));
        }
        let reference = if module.path.is_empty() {
            "(repository root)"
        } else {
            &module.path
        };
        c.push(EvidenceKind::Module, reference, detail);
    }
}

pub(super) fn module_path(dna: &RepositoryDna, id: &str) -> String {
    dna.architecture
        .modules
        .iter()
        .find(|module| module.id == id)
        .map_or_else(|| id.to_owned(), |module| module.path.clone())
}

pub(super) fn edges(
    dna: &RepositoryDna,
    c: &mut Candidates,
    limit: usize,
    filter: impl Fn(&str) -> bool,
) {
    let mut edges: Vec<_> = dna.architecture.module_edges.iter().collect();
    edges.sort_by(|a, b| b.weight.cmp(&a.weight).then(a.from.cmp(&b.from)));
    let mut added = 0;
    for edge in edges {
        let from = module_path(dna, &edge.from);
        let to = module_path(dna, &edge.to);
        if !(filter(&from) || filter(&to)) {
            continue;
        }
        if added == limit {
            break;
        }
        added += 1;
        c.push(
            EvidenceKind::Edge,
            format!("{from} → {to}"),
            format!(
                "{from} depends on {to} through {} ({} confidence).",
                plural(u64::from(edge.weight), "import"),
                edge.confidence.label()
            ),
        );
    }
}

pub(super) fn cycles(dna: &RepositoryDna, c: &mut Candidates, limit: usize) {
    for cycle in dna.architecture.cycles.iter().take(limit) {
        c.push(
            EvidenceKind::Edge,
            cycle.path.join(" → "),
            format!(
                "Dependency cycle between {} ({} confidence).",
                plural(cycle.members.len() as u64, "member"),
                cycle.confidence.label()
            ),
        );
    }
}

pub(super) fn architecture_signals(dna: &RepositoryDna, c: &mut Candidates) {
    let a = &dna.architecture;
    for signal in &a.signals {
        c.push(
            EvidenceKind::Fact,
            &signal.label,
            format!(
                "{} ({} confidence)",
                signal.description,
                signal.confidence.label()
            ),
        );
    }
    if !a.layers.is_empty() {
        let layers: Vec<String> = a
            .layers
            .iter()
            .take(6)
            .map(|layer| {
                let names: Vec<String> = layer
                    .modules
                    .iter()
                    .take(6)
                    .map(|id| module_path(dna, id))
                    .collect();
                format!("layer {}: {}", layer.index, names.join(", "))
            })
            .collect();
        c.push(
            EvidenceKind::Fact,
            "layers",
            format!(
                "Modules grouped by dependency depth (layer 0 depends on no other module): {}.",
                layers.join("; ")
            ),
        );
    }
    if let Some(workspace) = &a.workspace {
        c.push(
            EvidenceKind::File,
            &workspace.manifest,
            format!(
                "{} workspace with {}.",
                workspace.tool,
                plural(workspace.members.len() as u64, "member")
            ),
        );
    }
    if !a.isolated_modules.is_empty() {
        let shown: Vec<String> = a
            .isolated_modules
            .iter()
            .take(10)
            .map(|id| module_path(dna, id))
            .collect();
        c.push(
            EvidenceKind::Fact,
            "isolated modules",
            format!(
                "Modules with no detected imports in either direction: {}.",
                shown.join(", ")
            ),
        );
    }
}

pub(super) fn packages(dna: &RepositoryDna, c: &mut Candidates, limit: usize) {
    let d = &dna.dependencies;
    if d.direct_count > 0 {
        let ecosystems: Vec<String> = d
            .ecosystems
            .iter()
            .map(|e| format!("{} ({} manifests)", e.ecosystem, e.manifests))
            .collect();
        c.push(
            EvidenceKind::Metric,
            "dependencies",
            format!(
                "{} declared directly and {} locked; ecosystems: {}.",
                plural(d.direct_count, "external dependency"),
                d.locked_count,
                ecosystems.join(", ")
            ),
        );
    }
    let usage = |name: &str| {
        dna.architecture
            .external
            .iter()
            .find(|external| external.name == name)
            .map(|external| external.importers)
    };
    let mut external: Vec<_> = d
        .dependencies
        .iter()
        .filter(|dependency| !dependency.internal)
        .collect();
    external.sort_by_key(|dependency| {
        (
            std::cmp::Reverse(usage(&dependency.name).unwrap_or(0)),
            dependency.name.clone(),
        )
    });
    for dependency in external.into_iter().take(limit) {
        let mut detail = format!(
            "{} package, {} dependency",
            dependency.ecosystem,
            scope_label(dependency.scope)
        );
        if let Some(requirement) = &dependency.requirement {
            detail.push_str(&format!(", requirement {requirement}"));
        }
        if let Some(importers) = usage(&dependency.name) {
            detail.push_str(&format!(
                ", imported by {}",
                plural(u64::from(importers), "file")
            ));
        }
        detail.push('.');
        c.push(EvidenceKind::Package, &dependency.name, detail);
    }
    for duplicate in d.duplicates.iter().take(5) {
        c.push(
            EvidenceKind::Package,
            &duplicate.name,
            format!(
                "Locked in several versions ({}) in {}.",
                duplicate.versions.join(", "),
                duplicate.lockfile
            ),
        );
    }
}

pub(super) fn hotspots(
    dna: &RepositoryDna,
    c: &mut Candidates,
    limit: usize,
    filter: impl Fn(&str) -> bool,
) {
    for hotspot in dna
        .git
        .hot_spots
        .iter()
        .filter(|h| filter(&h.path))
        .take(limit)
    {
        c.push(
            EvidenceKind::File,
            &hotspot.path,
            format!(
                "Hotspot rank {}: {}, {} recent; {} lines changed in total; {} lines, complexity {}, {}. {}",
                hotspot.rank,
                plural(u64::from(hotspot.commits), "commit"),
                hotspot.recent_commits,
                hotspot.churn,
                hotspot.lines,
                hotspot.complexity,
                plural(u64::from(hotspot.dependents), "dependent file"),
                hotspot.interpretation
            ),
        );
    }
}

pub(super) fn findings(
    dna: &RepositoryDna,
    c: &mut Candidates,
    limit: usize,
    filter: impl Fn(&Finding) -> bool,
) {
    let mut findings: Vec<&Finding> = dna
        .findings
        .iter()
        .filter(|finding| !finding.is_suppressed() && filter(finding))
        .collect();
    findings.sort_by(|a, b| Finding::display_order(a, b));
    for finding in findings.into_iter().take(limit) {
        let location = finding
            .paths
            .first()
            .map(|path| format!(" ({path})"))
            .unwrap_or_default();
        c.push(
            EvidenceKind::Finding,
            &finding.rule,
            format!(
                "{} severity, {} confidence{location}: {} {}",
                finding.severity.label(),
                finding.confidence.label(),
                finding.title,
                finding.summary
            ),
        );
    }
}

pub(super) fn history(dna: &RepositoryDna, c: &mut Candidates) {
    let g = &dna.git;
    if g.commit_count == 0 {
        return;
    }
    let o = &g.ownership;
    if o.contributors > 0 {
        c.push(
            EvidenceKind::Metric,
            "ownership",
            format!(
                "{} contributors; {} account for half of all commits; the most active contributor made {} of commits.",
                o.contributors,
                o.contributors_for_half_of_commits,
                percent(o.top_contributor_share)
            ),
        );
    }
    for release in g.releases.iter().rev().take(8) {
        c.push(
            EvidenceKind::Commit,
            format!("release {}", release.tag),
            format!(
                "Version {} released on {}, {} after the previous release.",
                release.version,
                release.date.date_string(),
                plural(release.commits_since_previous, "commit")
            ),
        );
    }
    for period in g.dormant_periods.iter().take(4) {
        c.push(
            EvidenceKind::Fact,
            "inactive period",
            format!(
                "No commits for {} days, from {} to {}.",
                period.days,
                period.start.date_string(),
                period.end.date_string()
            ),
        );
    }
    let years: Vec<String> = g
        .timeline
        .iter()
        .rev()
        .take(12)
        .map(|bucket| format!("{}: {}", bucket.period, bucket.commits))
        .collect();
    if !years.is_empty() {
        c.push(
            EvidenceKind::Metric,
            "commits per period",
            format!("Most recent periods first — {}.", years.join(", ")),
        );
    }
    for directory in g.directory_activity.iter().take(6) {
        c.push(
            EvidenceKind::Module,
            &directory.path,
            format!(
                "{} and {} lines changed; last changed {}; {} of its churn is recent.",
                plural(directory.commits, "commit"),
                directory.churn,
                directory.last_changed.date_string(),
                percent(directory.recent_churn_share)
            ),
        );
    }
}

pub(super) fn evolution(dna: &RepositoryDna, c: &mut Candidates) {
    let e = &dna.evolution;
    for epoch in e.epochs.iter().take(8) {
        let focus = if epoch.focus_areas.is_empty() {
            String::new()
        } else {
            format!(" Focus: {}.", epoch.focus_areas.join(", "))
        };
        c.push(
            EvidenceKind::Fact,
            format!("epoch {}", epoch.label),
            format!(
                "From {} to {}: {} by {}.{focus}",
                epoch.start.date_string(),
                epoch.end.date_string(),
                plural(epoch.commits, "commit"),
                plural(u64::from(epoch.contributors), "contributor")
            ),
        );
    }
    for event in e.events.iter().take(12) {
        let reference = event
            .commit
            .as_deref()
            .map_or_else(|| event.title.clone(), |commit| format!("commit {commit}"));
        c.push(
            EvidenceKind::Commit,
            reference,
            format!(
                "{} ({}): {}",
                event.title,
                event.date.date_string(),
                event.description
            ),
        );
    }
}

pub(super) fn recent_commits(dna: &RepositoryDna, c: &mut Candidates, limit: usize) {
    for commit in dna
        .git
        .commits
        .iter()
        .filter(|commit| !commit.merge)
        .take(limit)
    {
        let subject = if commit.subject.is_empty() {
            "subject not recorded".to_owned()
        } else {
            format!("subject: {}", commit.subject)
        };
        c.push(
            EvidenceKind::Commit,
            format!("commit {}", commit.short),
            format!(
                "{}, {} files changed, +{} −{}; {subject}",
                commit.timestamp.date_string(),
                commit.files_changed,
                commit.insertions,
                commit.deletions
            ),
        );
    }
}

pub(super) fn recent_changes(dna: &RepositoryDna, c: &mut Candidates) {
    let Some(recent) = &dna.insights.recent_changes else {
        return;
    };
    let areas: Vec<String> = recent
        .directories
        .iter()
        .take(5)
        .map(|d| format!("{} ({} commits)", d.path, d.commits))
        .collect();
    c.push(
        EvidenceKind::Fact,
        format!("last {} days", recent.window_days),
        format!(
            "{} by {}; {} files changed (+{} −{}); most active areas: {}. {} added and {} removed files; {} test files and {} documentation files changed.",
            plural(recent.commits, "commit"),
            plural(u64::from(recent.contributors), "contributor"),
            recent.files_changed,
            recent.insertions,
            recent.deletions,
            if areas.is_empty() { "none".to_owned() } else { areas.join(", ") },
            recent.added_files.len(),
            recent.removed_files.len(),
            recent.test_files_changed,
            recent.doc_files_changed
        ),
    );
    for change in recent.dependency_changes.iter().take(8) {
        let versions = match (&change.from, &change.to) {
            (Some(from), Some(to)) => format!(" from {from} to {to}"),
            (None, Some(to)) => format!(" at {to}"),
            _ => String::new(),
        };
        c.push(
            EvidenceKind::Package,
            &change.name,
            format!(
                "Recently {}{versions} in {}.",
                change_label(change.change),
                change.manifest
            ),
        );
    }
}

pub(super) fn important_files(dna: &RepositoryDna, c: &mut Candidates, limit: usize) {
    for file in dna.insights.important_files.iter().take(limit) {
        c.push(
            EvidenceKind::File,
            &file.path,
            format!("Suggested early read: {}.", file.reasons.join("; ")),
        );
    }
}

pub(super) fn onboarding(dna: &RepositoryDna, c: &mut Candidates) {
    for (index, step) in dna.insights.onboarding.iter().enumerate() {
        let mut detail = format!("{}: {}", step.title, step.description);
        if !step.paths.is_empty() {
            detail.push_str(&format!(" Paths: {}.", step.paths.join(", ")));
        }
        if !step.commands.is_empty() {
            detail.push_str(&format!(
                " Commands (not executed): {}.",
                step.commands.join("; ")
            ));
        }
        c.push(
            EvidenceKind::Fact,
            format!("onboarding step {}", index + 1),
            detail,
        );
    }
}

pub(super) fn glossary(dna: &RepositoryDna, c: &mut Candidates, limit: usize) {
    for term in dna.insights.glossary.iter().take(limit) {
        c.push(
            EvidenceKind::Fact,
            format!("term {}", term.term),
            &term.definition,
        );
    }
}

pub(super) fn tests_and_docs(dna: &RepositoryDna, c: &mut Candidates) {
    let t = &dna.tests;
    let frameworks: Vec<&str> = t.frameworks.iter().map(|f| f.name.as_str()).collect();
    c.push(
        EvidenceKind::Metric,
        "tests",
        format!(
            "{} and {} with inline tests; {} of code lines are test code; frameworks: {}.",
            plural(t.test_files, "test file"),
            plural(t.inline_test_files, "source file"),
            percent(t.test_ratio),
            if frameworks.is_empty() {
                "none detected".to_owned()
            } else {
                frameworks.join(", ")
            }
        ),
    );
    if let Some(readme) = &dna.docs.readme {
        c.push(
            EvidenceKind::File,
            &readme.path,
            format!(
                "README with {} words and {} headings; installation section: {}; usage section: {}.",
                readme.words,
                readme.headings.len(),
                if readme.has_installation { "yes" } else { "no" },
                if readme.has_usage { "yes" } else { "no" }
            ),
        );
    }
    for ci in dna.builds.ci.iter().take(3) {
        c.push(
            EvidenceKind::File,
            &ci.path,
            format!(
                "{} configuration with jobs: {}.",
                ci.provider,
                ci.jobs.join(", ")
            ),
        );
    }
}

pub(super) fn fingerprint(dna: &RepositoryDna, c: &mut Candidates) {
    for dimension in &dna.fingerprint.dimensions {
        c.push(
            EvidenceKind::Metric,
            &dimension.label,
            format!(
                "{:.2} on a 0–1 scale: {}",
                dimension.value, dimension.description
            ),
        );
    }
}
