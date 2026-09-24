//! CI/CD configurations: providers, job names, and the test commands CI runs.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;
use repodna_core::evidence::Evidence;
use repodna_core::model::project::{CiConfig, CommandCandidate, CommandPurpose};
use yaml_rust2::Yaml;

use crate::is_ci_file;

/// Maximum jobs listed per configuration.
const MAX_JOBS: usize = 50;

/// Maximum CI test commands returned.
pub const MAX_CI_COMMANDS: usize = 30;

static JENKINS_STAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\bstage\s*\(\s*['"]([^'"]+)['"]"#)
        .unwrap_or_else(|error| panic!("invalid Jenkins stage pattern: {error}"))
});

static TEST_COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b(?:cargo (?:nextest run|test)|(?:npm|pnpm|yarn)(?: run)? test|bun test|npx (?:jest|vitest|playwright test)|(?:python3? -m )?pytest|tox|nox|go test|(?:\./)?mvnw? [^|;&]*\btest\b|(?:\./)?gradlew? [^|;&]*\btest\b|dotnet test|bundle exec (?:rspec|rake test)|phpunit|vendor/bin/(?:phpunit|pest)|flutter test|dart test|swift test|ctest|make (?:test|check)|just test|deno test|mix test)\b",
    )
    .unwrap_or_else(|error| panic!("invalid CI test command pattern: {error}"))
});

fn provider(path: &str) -> &'static str {
    if path.starts_with(".github/workflows/") {
        "GitHub Actions"
    } else if path.starts_with(".buildkite/") {
        "Buildkite"
    } else {
        match path {
            ".gitlab-ci.yml" => "GitLab CI",
            ".circleci/config.yml" => "CircleCI",
            "azure-pipelines.yml" => "Azure Pipelines",
            "Jenkinsfile" => "Jenkins",
            ".travis.yml" => "Travis CI",
            "bitbucket-pipelines.yml" => "Bitbucket Pipelines",
            ".drone.yml" => "Drone",
            "appveyor.yml" => "AppVeyor",
            _ => "Google Cloud Build",
        }
    }
}

fn keys(value: &Yaml) -> Vec<String> {
    value
        .as_hash()
        .map(|hash| {
            hash.keys()
                .filter_map(|key| key.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn names_in_list(value: &Yaml, field: &str) -> Vec<String> {
    value
        .as_vec()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item[field].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

const GITLAB_RESERVED: &[&str] = &[
    "after_script",
    "before_script",
    "cache",
    "default",
    "image",
    "include",
    "pages",
    "services",
    "stages",
    "variables",
    "workflow",
];

fn jobs(path: &str, text: &str) -> Vec<String> {
    if path == "Jenkinsfile" {
        return JENKINS_STAGE
            .captures_iter(text)
            .map(|captures| captures[1].to_owned())
            .collect();
    }
    let Ok(document) = repodna_dependencies::yaml::load(text) else {
        return Vec::new();
    };
    match provider(path) {
        "GitHub Actions" => document["jobs"]
            .as_hash()
            .map(|jobs| {
                jobs.iter()
                    .filter_map(|(id, job)| {
                        let id = id.as_str()?;
                        let name = job["name"].as_str().filter(|name| !name.contains("${{"));
                        Some(name.unwrap_or(id).to_owned())
                    })
                    .collect()
            })
            .unwrap_or_default(),
        "GitLab CI" => keys(&document)
            .into_iter()
            .filter(|key| !key.starts_with('.') && !GITLAB_RESERVED.contains(&key.as_str()))
            .collect(),
        "CircleCI" => keys(&document["jobs"]),
        "Azure Pipelines" => {
            let mut names = names_in_list(&document["jobs"], "job");
            names.extend(names_in_list(&document["stages"], "stage"));
            names
        }
        "Travis CI" => names_in_list(&document["jobs"]["include"], "name"),
        "Bitbucket Pipelines" => keys(&document["pipelines"]),
        "Drone" => names_in_list(&document["steps"], "name"),
        "Buildkite" => names_in_list(&document["steps"], "label"),
        "Google Cloud Build" => names_in_list(&document["steps"], "id"),
        _ => Vec::new(),
    }
}

/// Lists the CI/CD configurations among `contents`.
pub fn ci_configs(contents: &BTreeMap<String, String>) -> Vec<CiConfig> {
    contents
        .iter()
        .filter(|(path, _)| is_ci_file(path))
        .map(|(path, text)| {
            let mut jobs = jobs(path, text);
            jobs.truncate(MAX_JOBS);
            CiConfig {
                provider: provider(path).to_owned(),
                path: path.clone(),
                jobs,
            }
        })
        .collect()
}

/// Extracts the commands a line of CI configuration runs, without YAML list and key syntax.
fn command_text(line: &str) -> &str {
    let mut text = line.trim();
    text = text.strip_prefix("- ").unwrap_or(text).trim_start();
    for key in ["run:", "script:", "command:", "cmd:"] {
        if let Some(rest) = text.strip_prefix(key) {
            text = rest.trim_start();
        }
    }
    text.trim_matches(['"', '\'', '|', '>']).trim()
}

/// Test commands that CI configurations run.
pub fn ci_test_commands(contents: &BTreeMap<String, String>) -> Vec<CommandCandidate> {
    let mut seen = BTreeSet::new();
    let mut commands = Vec::new();
    for (path, text) in contents.iter().filter(|(path, _)| is_ci_file(path)) {
        for (index, line) in text.lines().enumerate() {
            if line.trim_start().starts_with('#') || !TEST_COMMAND.is_match(line) {
                continue;
            }
            let command = command_text(line);
            if command.is_empty() || command.len() > 200 || !seen.insert(command.to_owned()) {
                continue;
            }
            let line_number = u32::try_from(index + 1).unwrap_or(u32::MAX);
            commands.push(CommandCandidate {
                command: command.to_owned(),
                purpose: CommandPurpose::Test,
                working_directory: String::new(),
                source: format!("{path}:{line_number}"),
                verified: false,
                evidence: vec![Evidence::line(path.as_str(), line_number)],
            });
            if commands.len() >= MAX_CI_COMMANDS {
                return commands;
            }
        }
    }
    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contents(files: &[(&str, &str)]) -> BTreeMap<String, String> {
        files
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect()
    }

    #[test]
    fn lists_providers_and_jobs() {
        let files = contents(&[
            (
                ".github/workflows/ci.yml",
                "name: CI\non: push\njobs:\n  test:\n    name: Tests\n    runs-on: ubuntu-latest\n    steps:\n      - run: cargo test --workspace\n      # - run: npm test\n  lint:\n    name: ${{ matrix.os }} lint\n    steps:\n      - run: |\n          npm ci\n          npm run test -- --coverage\n",
            ),
            (
                ".gitlab-ci.yml",
                "stages: [build, test]\n.template: {}\nbuild:\n  script: make\nunit:\n  script:\n    - python -m pytest -q\n",
            ),
            (
                "Jenkinsfile",
                "pipeline { stages { stage('Build') {} stage(\"Test\") {} } }",
            ),
            ("README.md", "run cargo test"),
        ]);
        let configs = ci_configs(&files);
        let summary: Vec<(&str, Vec<&str>)> = configs
            .iter()
            .map(|c| {
                (
                    c.provider.as_str(),
                    c.jobs.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                ("GitHub Actions", vec!["Tests", "lint"]),
                ("GitLab CI", vec!["build", "unit"]),
                ("Jenkins", vec!["Build", "Test"]),
            ]
        );
        let commands: Vec<String> = ci_test_commands(&files)
            .into_iter()
            .map(|c| c.command)
            .collect();
        assert_eq!(
            commands,
            vec![
                "cargo test --workspace",
                "npm run test -- --coverage",
                "python -m pytest -q"
            ]
        );
    }

    #[test]
    fn tolerates_invalid_yaml() {
        let files = contents(&[(".circleci/config.yml", "jobs: [unclosed")]);
        let configs = ci_configs(&files);
        assert_eq!(configs[0].provider, "CircleCI");
        assert!(configs[0].jobs.is_empty());
    }
}
