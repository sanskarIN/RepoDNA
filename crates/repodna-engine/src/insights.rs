//! Insights: first-look answers, an onboarding path, important files, and a glossary.
//!
//! Everything here is written from the finished artifact. Each answer is labeled as a fact
//! or an interpretation and links to the evidence it was built from.

use std::collections::{BTreeMap, HashMap};

use repodna_core::confidence::Confidence;
use repodna_core::evidence::Evidence;
use repodna_core::metric::round4;
use repodna_core::model::architecture::EdgeKind;
use repodna_core::model::artifact::RepositoryDna;
use repodna_core::model::evolution::StatementKind;
use repodna_core::model::git::ActivityLevel;
use repodna_core::model::insights::{
    GlossaryTerm, GuideStep, ImportantFile, Insights, QuestionAnswer, RecentChanges,
};
use repodna_core::model::project::{CommandPurpose, DocCheckStatus};
use repodna_core::model::structure::FileCategory;
use repodna_core::paths;
use repodna_core::severity::Severity;
use repodna_core::text::{continue_sentence, count, join_alternatives};

/// Important files listed.
const MAX_IMPORTANT: usize = 15;

/// Glossary terms listed.
const MAX_TERMS: usize = 40;

fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

fn answer(
    id: &str,
    question: &str,
    text: String,
    kind: StatementKind,
    confidence: Confidence,
    evidence: Vec<Evidence>,
) -> QuestionAnswer {
    QuestionAnswer {
        id: id.to_owned(),
        question: question.to_owned(),
        answer: text,
        kind,
        confidence,
        evidence,
    }
}

fn language_name(dna: &RepositoryDna, id: &str) -> String {
    dna.languages
        .languages
        .iter()
        .find(|l| l.id == id)
        .map_or_else(|| id.to_owned(), |l| l.name.clone())
}

fn language_names(dna: &RepositoryDna) -> Vec<String> {
    dna.languages
        .primary
        .iter()
        .map(|id| language_name(dna, id))
        .collect()
}

fn commands(dna: &RepositoryDna, purposes: &[CommandPurpose], limit: usize) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for command in dna.builds.commands.iter().chain(&dna.tests.commands) {
        if purposes.contains(&command.purpose) && !found.contains(&command.command) {
            let text = if command.working_directory.is_empty() {
                command.command.clone()
            } else {
                format!("{} (in {})", command.command, command.working_directory)
            };
            found.push(text);
        }
        if found.len() >= limit {
            break;
        }
    }
    found
}

fn first_look(dna: &RepositoryDna) -> Vec<QuestionAnswer> {
    let mut answers = Vec::new();
    let languages = language_names(dna);

    // What is it?
    let mut text = match (&dna.docs.description, &dna.docs.description_source) {
        (Some(description), Some(source)) => {
            format!("\"{}\" (from {source}).", description.trim_end_matches('.'))
        }
        _ => "No description was found in the README or manifests.".to_owned(),
    };
    if !languages.is_empty() {
        text.push_str(&format!(" It is written mainly in {}.", list(&languages)));
    }
    if dna.architecture.status.has_results() && dna.architecture.style != "Unknown" {
        text.push_str(&format!(
            " Inferred architecture style: {}.",
            dna.architecture.style
        ));
    }
    let mut evidence: Vec<Evidence> = dna
        .docs
        .description_source
        .iter()
        .map(|s| Evidence::file(s.as_str()))
        .collect();
    evidence.push(Evidence::metric(
        "structure.files",
        dna.structure.total_files as f64,
    ));
    answers.push(answer(
        "what",
        "What is this repository?",
        text,
        StatementKind::Interpretation,
        Confidence::Medium,
        evidence,
    ));

    // Where does it start?
    let entrypoints = &dna.structure.entrypoints;
    let (text, confidence) = if entrypoints.is_empty() {
        ("No entrypoint was detected.".to_owned(), Confidence::Low)
    } else {
        let items: Vec<String> = entrypoints
            .iter()
            .take(5)
            .map(|e| format!("{} ({})", e.path, continue_sentence(&e.reason)))
            .collect();
        (
            format!("Execution or use starts at {}.", list(&items)),
            entrypoints
                .iter()
                .map(|e| e.confidence)
                .max()
                .unwrap_or(Confidence::Low),
        )
    };
    answers.push(answer(
        "entrypoints",
        "Where does it start?",
        text,
        StatementKind::Fact,
        confidence,
        entrypoints
            .iter()
            .take(5)
            .map(|e| Evidence::file(e.path.as_str()))
            .collect(),
    ));

    // How is it organized?
    if dna.architecture.status.has_results() {
        let mut modules: Vec<_> = dna.architecture.modules.iter().collect();
        modules.sort_by(|a, b| {
            b.code_lines
                .cmp(&a.code_lines)
                .then_with(|| a.id.cmp(&b.id))
        });
        let names: Vec<String> = modules.iter().take(5).map(|m| m.name.clone()).collect();
        let mut text = format!(
            "{} modules; the largest are {}. Style: {} (inferred).",
            dna.architecture.modules.len(),
            list(&names),
            dna.architecture.style
        );
        if let Some(signal) = dna.architecture.signals.first() {
            text.push_str(&format!(" {}", signal.description));
        }
        answers.push(answer(
            "structure",
            "How is it organized?",
            text,
            StatementKind::Interpretation,
            dna.architecture.style_confidence,
            modules
                .iter()
                .take(5)
                .map(|m| Evidence::directory(m.path.as_str()))
                .collect(),
        ));
    }

    // How do I build and run it?
    let run = commands(
        dna,
        &[
            CommandPurpose::Install,
            CommandPurpose::Build,
            CommandPurpose::Run,
            CommandPurpose::Dev,
        ],
        5,
    );
    let text = if run.is_empty() {
        "No build or run commands were detected.".to_owned()
    } else {
        format!("Detected (not verified): {}.", run.join("; "))
    };
    answers.push(answer(
        "run",
        "How do I build and run it?",
        text,
        StatementKind::Fact,
        if run.is_empty() {
            Confidence::Low
        } else {
            Confidence::Medium
        },
        dna.builds
            .commands
            .iter()
            .take(5)
            .flat_map(|c| c.evidence.first().cloned())
            .collect(),
    ));

    // How is it tested?
    let tests = &dna.tests;
    let frameworks: Vec<String> = tests.frameworks.iter().map(|f| f.name.clone()).collect();
    let mut text = count(tests.test_files, "test file", "test files");
    if tests.inline_test_files > 0 {
        text.push_str(&format!(
            " and {} with inline tests",
            count(tests.inline_test_files, "source file", "source files")
        ));
    }
    if !frameworks.is_empty() {
        text.push_str(&format!(" using {}", list(&frameworks)));
    }
    text.push('.');
    let test_commands = commands(dna, &[CommandPurpose::Test], 3);
    if !test_commands.is_empty() {
        text.push_str(&format!(
            " Run them with {}.",
            join_alternatives(&test_commands)
        ));
    }
    if !tests.ci_commands.is_empty() {
        text.push_str(" CI runs tests.");
    }
    answers.push(answer(
        "tests",
        "How is it tested?",
        text,
        StatementKind::Fact,
        Confidence::Medium,
        tests
            .frameworks
            .iter()
            .flat_map(|f| f.evidence.first().cloned())
            .take(5)
            .collect(),
    ));

    // Is it active?
    if dna.git.status.has_results() {
        let activity = &dna.git.activity;
        let mut text = format!("{}. {}", activity.level.label(), activity.description);
        // Busy repositories are described by their last 30 days; add the longer window
        // when it says something new.
        if matches!(
            activity.level,
            ActivityLevel::VeryActive | ActivityLevel::Active
        ) && activity.commits_last_90_days > activity.commits_last_30_days
        {
            text.push_str(&format!(
                " {} in the last 90 days.",
                count(activity.commits_last_90_days, "commit", "commits")
            ));
        }
        answers.push(answer(
            "activity",
            "Is it actively developed?",
            text,
            StatementKind::Fact,
            Confidence::High,
            dna.git
                .last_commit
                .iter()
                .map(|c| Evidence::commit(c.hash.clone(), Some(c.timestamp)))
                .collect(),
        ));
    }

    // Where does change concentrate?
    if !dna.git.hot_spots.is_empty() {
        let items: Vec<String> = dna
            .git
            .hot_spots
            .iter()
            .take(3)
            .map(|h| format!("{} ({} commits)", h.path, h.commits))
            .collect();
        answers.push(answer(
            "hotspots",
            "Where does change concentrate?",
            format!("The strongest change hotspots are {}.", list(&items)),
            StatementKind::Interpretation,
            Confidence::Medium,
            dna.git
                .hot_spots
                .iter()
                .take(3)
                .map(|h| Evidence::file(h.path.as_str()))
                .collect(),
        ));
    }

    // What should I look at first?
    let urgent: Vec<_> = dna
        .findings
        .iter()
        .filter(|f| !f.is_suppressed() && f.severity >= Severity::Warning)
        .collect();
    let text = if urgent.is_empty() {
        "No critical or warning findings.".to_owned()
    } else {
        let titles: Vec<String> = urgent.iter().take(3).map(|f| f.title.clone()).collect();
        format!(
            "{}, starting with: {}.",
            count(
                urgent.len() as u64,
                "critical or warning finding",
                "critical or warning findings"
            ),
            titles.join("; ")
        )
    };
    answers.push(answer(
        "risks",
        "What should I look at first?",
        text,
        StatementKind::Fact,
        Confidence::High,
        urgent
            .iter()
            .take(3)
            .flat_map(|f| f.evidence.first().cloned())
            .collect(),
    ));
    answers
}

fn onboarding(dna: &RepositoryDna, recent: Option<&RecentChanges>) -> Vec<GuideStep> {
    let mut steps = Vec::new();
    if let Some(readme) = &dna.docs.readme {
        steps.push(GuideStep {
            title: "Read the README".to_owned(),
            description: format!(
                "It has {} words and {} sections.",
                readme.words,
                readme.headings.len()
            ),
            paths: vec![readme.path.clone()],
            commands: Vec::new(),
        });
    }
    let requirements: Vec<String> = dna
        .builds
        .requirements
        .iter()
        .take(6)
        .map(|r| match &r.version {
            Some(version) => format!("{} {version}", r.name),
            None => r.name.clone(),
        })
        .collect();
    if !requirements.is_empty() {
        steps.push(GuideStep {
            title: "Install the prerequisites".to_owned(),
            description: format!("The repository declares {}.", list(&requirements)),
            paths: dna
                .builds
                .requirements
                .iter()
                .take(6)
                .map(|r| r.source.clone())
                .collect(),
            commands: Vec::new(),
        });
    }
    let build = commands(dna, &[CommandPurpose::Install, CommandPurpose::Build], 3);
    if !build.is_empty() {
        steps.push(GuideStep {
            title: "Install dependencies and build".to_owned(),
            description: "These commands were detected from the repository's metadata; they have not been run.".to_owned(),
            paths: Vec::new(),
            commands: build,
        });
    }
    let tests = commands(dna, &[CommandPurpose::Test], 2);
    if !tests.is_empty() {
        steps.push(GuideStep {
            title: "Run the tests".to_owned(),
            description: if dna.tests.inline_test_files > 0 {
                format!(
                    "{} test files and {} source files with inline tests were found.",
                    dna.tests.test_files, dna.tests.inline_test_files
                )
            } else {
                format!("{} test files were found.", dna.tests.test_files)
            },
            paths: dna.tests.test_directories.iter().take(3).cloned().collect(),
            commands: tests,
        });
    }
    if !dna.structure.entrypoints.is_empty() {
        steps.push(GuideStep {
            title: "Start at the entrypoints".to_owned(),
            description: "Follow the code from where execution or use begins.".to_owned(),
            paths: dna
                .structure
                .entrypoints
                .iter()
                .take(3)
                .map(|e| e.path.clone())
                .collect(),
            commands: Vec::new(),
        });
    }
    let mut central: Vec<_> = dna
        .architecture
        .modules
        .iter()
        .filter(|m| !m.importance.is_empty())
        .collect();
    central.sort_by(|a, b| {
        b.importance
            .len()
            .cmp(&a.importance.len())
            .then_with(|| b.fan_in.cmp(&a.fan_in))
            .then_with(|| a.id.cmp(&b.id))
    });
    if !central.is_empty() {
        steps.push(GuideStep {
            title: "Explore the central modules".to_owned(),
            description: central
                .iter()
                .take(3)
                .map(|m| {
                    let reasons: Vec<String> =
                        m.importance.iter().map(|r| continue_sentence(r)).collect();
                    format!("{}: {}.", m.name, reasons.join("; "))
                })
                .collect::<Vec<_>>()
                .join(" "),
            paths: central.iter().take(3).map(|m| m.path.clone()).collect(),
            commands: Vec::new(),
        });
    }
    if let Some(recent) = recent
        && recent.commits > 0
    {
        steps.push(GuideStep {
            title: "Review recent changes".to_owned(),
            description: format!(
                "{} commits in the last {} days, mostly in {}.",
                recent.commits,
                recent.window_days,
                list(
                    &recent
                        .directories
                        .iter()
                        .take(3)
                        .map(|d| format!("{}/", d.path))
                        .collect::<Vec<_>>()
                )
            ),
            paths: recent
                .directories
                .iter()
                .take(3)
                .map(|d| d.path.clone())
                .collect(),
            commands: Vec::new(),
        });
    }
    steps
}

fn important_files(dna: &RepositoryDna) -> Vec<ImportantFile> {
    let mut scores: BTreeMap<String, (f64, Vec<String>)> = BTreeMap::new();
    let mut add = |path: &str, score: f64, reason: String| {
        let entry = scores.entry(path.to_owned()).or_default();
        entry.0 += score;
        entry.1.push(reason);
    };
    for entry in &dna.structure.entrypoints {
        add(&entry.path, 0.35, format!("Entrypoint: {}", entry.reason));
    }
    if let Some(readme) = &dna.docs.readme {
        add(&readme.path, 0.3, "Project README".to_owned());
    }
    for manifest in dna
        .dependencies
        .manifests
        .iter()
        .filter(|m| paths::depth(&m.path) == 1)
    {
        add(
            &manifest.path,
            0.15,
            format!("Root {} manifest", manifest.ecosystem),
        );
    }
    let mut dependents: HashMap<&str, u32> = HashMap::new();
    for edge in dna
        .architecture
        .file_edges
        .iter()
        .filter(|e| e.kind != EdgeKind::Module)
    {
        *dependents.entry(edge.to.as_str()).or_default() += 1;
    }
    let max_dependents = dependents.values().copied().max().unwrap_or(0);
    if max_dependents >= 2 {
        for (path, count) in &dependents {
            if *count >= 2 {
                add(
                    path,
                    0.3 * f64::from(*count) / f64::from(max_dependents),
                    format!("{count} files import it"),
                );
            }
        }
    }
    for hotspot in dna.git.hot_spots.iter().take(20) {
        add(
            &hotspot.path,
            0.25 * (1.0 - f64::from(hotspot.rank - 1) / 20.0),
            format!("Change hotspot (rank {})", hotspot.rank),
        );
    }
    let categories: HashMap<&str, FileCategory> = dna
        .structure
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.category))
        .collect();
    let mut files: Vec<ImportantFile> = scores
        .into_iter()
        .filter(|(path, _)| {
            categories
                .get(path.as_str())
                .is_none_or(|c| *c != FileCategory::Lockfile)
        })
        .map(|(path, (score, reasons))| ImportantFile {
            path,
            score: round4(score.min(1.0)),
            reasons,
        })
        .collect();
    files.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.path.cmp(&b.path))
    });
    files.truncate(MAX_IMPORTANT);
    files
}

fn glossary(dna: &RepositoryDna) -> Vec<GlossaryTerm> {
    let mut terms = Vec::new();
    let mut modules: Vec<_> = dna.architecture.modules.iter().collect();
    modules.sort_by(|a, b| {
        b.code_lines
            .cmp(&a.code_lines)
            .then_with(|| a.id.cmp(&b.id))
    });
    for module in modules.iter().take(20) {
        let location = if module.path.is_empty() {
            "the repository root".to_owned()
        } else {
            format!("{}/", module.path)
        };
        let mut definition = format!("Module at {location} with {} files", module.files);
        if let Some(language) = &module.language {
            definition.push_str(&format!(", mostly {}", language_name(dna, language)));
        }
        definition.push('.');
        if let Some(reason) = module.importance.first() {
            definition.push_str(&format!(" {reason}."));
        }
        terms.push(GlossaryTerm {
            term: module.name.clone(),
            definition,
        });
    }
    for tool in dna.builds.systems.iter().chain(&dna.tests.frameworks) {
        let source = tool
            .evidence
            .first()
            .and_then(|e| e.path().map(str::to_owned));
        terms.push(GlossaryTerm {
            term: tool.name.clone(),
            definition: match source {
                Some(path) => format!("Build or test tool detected from {path}."),
                None => "Build or test tool detected from the declared dependencies.".to_owned(),
            },
        });
    }
    for ci in &dna.builds.ci {
        terms.push(GlossaryTerm {
            term: ci.provider.clone(),
            definition: format!("CI/CD service configured in {}.", ci.path),
        });
    }
    let mut seen = std::collections::HashSet::new();
    terms.retain(|term| seen.insert(term.term.clone()));
    terms.truncate(MAX_TERMS);
    terms
}

/// Builds the insights for a finished artifact.
pub fn build_insights(dna: &RepositoryDna, recent: Option<RecentChanges>) -> Insights {
    let readme_present = dna
        .docs
        .checks
        .iter()
        .any(|check| check.id == "readme" && check.status != DocCheckStatus::NotDetected);
    let mut insights = Insights {
        first_look: first_look(dna),
        onboarding: onboarding(dna, recent.as_ref()),
        important_files: important_files(dna),
        recent_changes: recent,
        glossary: glossary(dna),
    };
    if !readme_present {
        insights
            .onboarding
            .retain(|step| step.title != "Read the README");
    }
    insights
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::SectionStatus;
    use repodna_core::model::architecture::{FileEdge, ModuleKind, ModuleRecord};
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::metadata::AnalysisMetadata;
    use repodna_core::model::project::{CommandCandidate, ReadmeInfo};
    use repodna_core::model::structure::{Entrypoint, EntrypointKind};

    fn dna() -> RepositoryDna {
        let mut dna =
            RepositoryDna::new(RepositoryIdentity::default(), AnalysisMetadata::default());
        dna.docs.description = Some("A tool that does things.".into());
        dna.docs.description_source = Some("Cargo.toml".into());
        dna.docs.readme = Some(ReadmeInfo {
            path: "README.md".into(),
            lines: 10,
            words: 50,
            headings: vec!["Tool".into()],
            code_blocks: 1,
            links: 0,
            images: 0,
            has_installation: true,
            has_usage: true,
        });
        dna.docs.checks = vec![repodna_core::model::project::DocCheck {
            id: "readme".into(),
            label: "README".into(),
            status: DocCheckStatus::Present,
            optional: false,
            evidence: Vec::new(),
        }];
        dna.structure.entrypoints = vec![Entrypoint {
            path: "src/main.rs".into(),
            kind: EntrypointKind::Binary,
            language: Some("rust".into()),
            reason: "Cargo builds main.rs as a binary crate root".into(),
            confidence: Confidence::High,
            evidence: Vec::new(),
        }];
        dna.builds.commands = vec![CommandCandidate {
            command: "cargo build".into(),
            purpose: CommandPurpose::Build,
            working_directory: String::new(),
            source: "Cargo.toml".into(),
            verified: false,
            evidence: vec![Evidence::file("Cargo.toml")],
        }];
        dna.architecture.status = SectionStatus::Analyzed;
        dna.architecture.style = "Modular".into();
        dna.architecture.modules = vec![ModuleRecord {
            id: "src".into(),
            name: "src".into(),
            path: "src".into(),
            kind: ModuleKind::Directory,
            language: Some("rust".into()),
            files: 3,
            code_lines: 300,
            fan_in: 2,
            fan_out: 0,
            instability: 0.0,
            centrality: 0.0,
            layer: Some(0),
            inferred: true,
            confidence: Confidence::Medium,
            importance: vec!["Holds 90% of the first-party code".into()],
            external_dependencies: Vec::new(),
            evidence: Vec::new(),
        }];
        dna.architecture.file_edges = ["src/a.rs", "src/b.rs"]
            .iter()
            .map(|from| FileEdge {
                from: (*from).into(),
                to: "src/util.rs".into(),
                kind: EdgeKind::Import,
                line: 1,
                confidence: Confidence::High,
                specifier: None,
            })
            .collect();
        dna
    }

    #[test]
    fn answers_first_look_questions() {
        let insights = build_insights(&dna(), None);
        let ids: Vec<&str> = insights.first_look.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["what", "entrypoints", "structure", "run", "tests", "risks"]
        );
        assert!(
            insights.first_look[0]
                .answer
                .starts_with("\"A tool that does things\" (from Cargo.toml).")
        );
        assert!(insights.first_look[1].answer.contains("src/main.rs"));
        assert_eq!(
            insights.first_look[3].answer,
            "Detected (not verified): cargo build."
        );
        assert_eq!(
            insights.first_look[5].answer,
            "No critical or warning findings."
        );
    }

    #[test]
    fn builds_onboarding_important_files_and_glossary() {
        let insights = build_insights(&dna(), None);
        let titles: Vec<&str> = insights
            .onboarding
            .iter()
            .map(|s| s.title.as_str())
            .collect();
        assert_eq!(
            titles,
            vec![
                "Read the README",
                "Install dependencies and build",
                "Start at the entrypoints",
                "Explore the central modules"
            ]
        );
        let paths: Vec<&str> = insights
            .important_files
            .iter()
            .map(|f| f.path.as_str())
            .collect();
        assert_eq!(paths, vec!["src/main.rs", "README.md", "src/util.rs"]);
        assert_eq!(
            insights.important_files[2].reasons,
            vec!["2 files import it"]
        );
        assert_eq!(insights.glossary[0].term, "src");
        assert!(insights.glossary[0].definition.contains("mostly rust"));
    }
}
