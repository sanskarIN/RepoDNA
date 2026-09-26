//! Analyzes every fixture repository and checks what RepoDNA finds in each: the fixtures
//! are designed so that each one demonstrates a situation the analysis must recognize.

use repodna_app::{AnalyzeOptions, AppPaths, ConfigOptions, run_analysis};
use repodna_core::CancellationToken;
use repodna_core::config::AnalysisProfile;
use repodna_core::model::artifact::RepositoryDna;
use repodna_core::model::git::ActivityLevel;
use repodna_core::time::Timestamp;
use repodna_engine::Progress;
use repodna_testkit::fixtures::{FixtureOptions, build, fixture};
use repodna_testkit::git_available;

/// Analyzes fixture `name` with `profile`, optionally with a user configuration file.
fn analyze(name: &str, profile: AnalysisProfile, user_config: Option<&str>) -> RepositoryDna {
    let home = tempfile::tempdir().expect("home directory");
    let fixtures = tempfile::tempdir().expect("fixture directory");
    let root = fixtures.path().join(name);
    let options = FixtureOptions {
        large_files: 400,
        large_commits: 8,
    };
    build(name, &root, &options).unwrap_or_else(|error| panic!("fixture {name}: {error}"));
    let paths = AppPaths::in_directory(home.path());
    if let Some(config) = user_config {
        std::fs::write(&paths.config_file, config).expect("user configuration");
    }
    let mut config = ConfigOptions {
        ignore_user_config: user_config.is_none(),
        ..ConfigOptions::default()
    };
    config.overrides.profile = Some(profile);
    let request = AnalyzeOptions {
        input: root.to_string_lossy().into_owned(),
        config,
        no_store: true,
        reproducible: true,
        reference_time: Some(Timestamp::from_unix(1_790_000_000)),
        ..AnalyzeOptions::default()
    };
    run_analysis(
        &paths,
        &request,
        Progress::default(),
        &CancellationToken::new(),
    )
    .unwrap_or_else(|error| panic!("analysis of {name}: {error}"))
    .dna
}

/// Fixtures with history need Git; without it they are skipped.
fn needs_git(name: &str) -> bool {
    let wanted = fixture(name).is_some_and(|f| f.git);
    wanted && !git_available()
}

fn rules(dna: &RepositoryDna) -> Vec<&str> {
    dna.findings.iter().map(|f| f.rule.as_str()).collect()
}

/// Whether `program` runs; the tests that execute build and test commands need it.
#[cfg(unix)]
fn tool_available(program: &str) -> bool {
    std::process::Command::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[test]
fn tiny_project() {
    if needs_git("tiny") {
        return;
    }
    let dna = analyze("tiny", AnalysisProfile::Standard, None);
    assert_eq!(dna.languages.primary, vec!["python".to_owned()]);
    assert_eq!(dna.git.commit_count, 3);
    assert!(dna.docs.readme.is_some());
    assert_eq!(
        dna.docs.license.as_ref().and_then(|l| l.spdx.as_deref()),
        Some("MIT")
    );
    assert!(dna.tests.test_files >= 1);
    assert!(!rules(&dna).contains(&"docs.readme-missing"));
}

#[test]
fn polyglot_service() {
    if needs_git("polyglot") {
        return;
    }
    let dna = analyze("polyglot", AnalysisProfile::Standard, None);
    let languages: Vec<&str> = dna
        .languages
        .languages
        .iter()
        .filter(|l| l.share > 0.0)
        .map(|l| l.id.as_str())
        .collect();
    for language in ["go", "typescript", "python"] {
        assert!(languages.contains(&language), "{language} in {languages:?}");
    }
    assert!(!dna.builds.containers.is_empty());
    assert!(!dna.builds.ci.is_empty());
    assert!(dna.dependencies.ecosystems.len() >= 2);
    assert_eq!(dna.git.ownership.contributors, 2);
}

#[test]
fn monorepo_packages() {
    if needs_git("monorepo") {
        return;
    }
    let dna = analyze("monorepo", AnalysisProfile::Standard, None);
    assert!(dna.architecture.workspace.is_some());
    assert!(dna.architecture.packages.len() >= 5);
    assert!(dna.architecture.module_edges.len() >= 3);
    assert!(dna.architecture.cycles.is_empty());
}

#[test]
fn rich_history() {
    if needs_git("history") {
        return;
    }
    let dna = analyze("history", AnalysisProfile::Deep, None);
    assert_eq!(dna.git.commit_count, 15);
    assert_eq!(dna.git.ownership.contributors, 4);
    assert_eq!(dna.git.releases.len(), 4);
    assert!(!dna.git.dormant_periods.is_empty());
    assert!(!dna.evolution.epochs.is_empty());
    assert!(dna.evolution.snapshots.len() >= 3);
    assert!(!dna.evolution.story.is_empty());
}

#[test]
fn no_git_history() {
    let dna = analyze("no-git", AnalysisProfile::Standard, None);
    assert!(!dna.identity.is_git_repository);
    assert_eq!(dna.git.commit_count, 0);
    assert_eq!(dna.languages.primary, vec!["rust".to_owned()]);
    assert!(
        dna.structure
            .entrypoints
            .iter()
            .any(|e| e.path == "src/main.rs")
    );
}

#[test]
fn duplicated_code() {
    if needs_git("duplicated") {
        return;
    }
    let dna = analyze("duplicated", AnalysisProfile::Deep, None);
    let clusters = &dna.code_quality.duplication.clusters;
    assert!(
        clusters.iter().any(|c| c.occurrence_count >= 3),
        "{clusters:?}"
    );
    assert!(rules(&dna).contains(&"quality.duplicate-block"));
}

#[test]
fn dependency_cycles() {
    if needs_git("cyclic") {
        return;
    }
    let dna = analyze("cyclic", AnalysisProfile::Standard, None);
    assert!(!dna.architecture.cycles.is_empty());
    assert!(
        dna.architecture
            .cycles
            .iter()
            .any(|c| c.members.iter().any(|m| m.contains("lookup"))),
        "{:?}",
        dna.architecture.cycles
    );
}

#[test]
fn generated_code() {
    if needs_git("generated") {
        return;
    }
    let dna = analyze("generated", AnalysisProfile::Standard, None);
    let generated: Vec<&str> = dna
        .structure
        .files
        .iter()
        .filter(|f| f.generated)
        .map(|f| f.path.as_str())
        .collect();
    for path in [
        "api/weather.pb.go",
        "api/weather.pb.ts",
        "web/dist/app.min.js",
    ] {
        assert!(generated.contains(&path), "{path} in {generated:?}");
    }
    assert!(!generated.contains(&"server/handler.go"));
    assert!(
        dna.structure
            .notes
            .iter()
            .any(|note| note.contains(".gitattributes"))
    );
}

#[test]
fn missing_documentation() {
    if needs_git("undocumented") {
        return;
    }
    let dna = analyze("undocumented", AnalysisProfile::Standard, None);
    assert!(dna.docs.readme.is_none());
    assert!(dna.docs.license.is_none());
    assert!(rules(&dna).contains(&"docs.readme-missing"));
}

#[test]
fn suspicious_secrets() {
    if needs_git("secrets") {
        return;
    }
    let dna = analyze("secrets", AnalysisProfile::Standard, None);
    let secrets = &dna.security.secrets;
    let found = |rule: &str, path: &str| {
        secrets
            .iter()
            .find(|s| s.rule == rule && s.path == path)
            .unwrap_or_else(|| panic!("{rule} in {path}: {secrets:?}"))
    };
    assert!(!found("aws-access-key-id", "deploy/config.py").in_test_or_example);
    assert!(found("aws-access-key-id", "tests/fixtures/credentials.json").in_test_or_example);
    found("github-token", "deploy/github.py");
    found("private-key", "deploy/server.key");
    found("env-assignment-secret", ".env");
    assert!(secrets.iter().all(|s| s.path != "settings.example.toml"));
    assert!(
        dna.security
            .patterns
            .iter()
            .any(|p| p.rule == "tls-verification-disabled" && p.path == "deploy/verify.py")
    );
    // Values are never stored, only fingerprints.
    let json = serde_json::to_string(&dna).expect("artifact JSON");
    assert!(!json.contains(&["AK", "IA", "Z7Q2X9W4R5T1Y8U3"].concat()));
}

#[cfg(unix)]
#[test]
fn failing_build_is_reported_when_execution_is_enabled() {
    if needs_git("build-failure") || !tool_available("npm") {
        return;
    }
    let dna = analyze(
        "build-failure",
        AnalysisProfile::Standard,
        Some("[execution]\nallow_build_commands = true\ntimeout_seconds = 120\n"),
    );
    let run = dna
        .builds
        .executions
        .iter()
        .find(|run| run.command.contains("build"))
        .unwrap_or_else(|| panic!("{:?}", dna.builds.executions));
    assert!(!run.success);
    assert!(
        run.output_tail
            .iter()
            .any(|line| line.contains("build failed"))
    );
}

#[cfg(unix)]
#[test]
fn failing_tests_are_reported_when_execution_is_enabled() {
    if needs_git("test-failure") || !tool_available("make") || !tool_available("python3") {
        return;
    }
    let dna = analyze(
        "test-failure",
        AnalysisProfile::Standard,
        Some("[execution]\nallow_test_commands = true\ntimeout_seconds = 120\n"),
    );
    let run = dna
        .tests
        .executions
        .first()
        .unwrap_or_else(|| panic!("no test run: {:?}", dna.tests.commands));
    assert!(!run.success, "{run:?}");
}

#[test]
fn commands_do_not_run_by_default() {
    if needs_git("build-failure") {
        return;
    }
    let dna = analyze("build-failure", AnalysisProfile::Standard, None);
    assert!(dna.builds.executions.is_empty());
    assert!(!dna.builds.commands.is_empty());
    assert!(dna.builds.commands.iter().all(|c| !c.verified));
}

#[test]
fn unsupported_languages() {
    if needs_git("unsupported") {
        return;
    }
    let dna = analyze("unsupported", AnalysisProfile::Standard, None);
    assert!(dna.languages.unrecognized_files >= 2);
    let known: Vec<&str> = dna
        .languages
        .languages
        .iter()
        .map(|l| l.id.as_str())
        .collect();
    assert!(!known.contains(&"zig"));
}

#[test]
fn archived_project() {
    if needs_git("archived") {
        return;
    }
    let dna = analyze("archived", AnalysisProfile::Standard, None);
    assert_eq!(dna.git.activity.level, ActivityLevel::Dormant);
    assert_eq!(dna.git.releases.len(), 3);
    assert!(dna.code_quality.markers.total >= 2);
}

#[test]
fn large_repository() {
    if needs_git("large") {
        return;
    }
    let dna = analyze("large", AnalysisProfile::Quick, None);
    assert!(dna.structure.total_files >= 400);
    assert!(dna.languages.primary.len() >= 2);
}
