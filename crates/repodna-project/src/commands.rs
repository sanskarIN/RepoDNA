//! Command candidates derived from repository metadata.
//!
//! Candidates are what the repository itself declares: package.json scripts, Makefile and
//! justfile targets, Taskfile tasks, Deno tasks, Composer scripts, and the conventional
//! commands of each detected build system. None of them is run here.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;
use repodna_core::evidence::Evidence;
use repodna_core::model::project::{CommandCandidate, CommandPurpose};
use repodna_core::paths;
use yaml_rust2::Yaml;

use crate::tools::{Index, ToolContext, node_package_manager};

/// Maximum command candidates returned.
pub const MAX_COMMANDS: usize = 100;

static MAKE_TARGET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([A-Za-z0-9][A-Za-z0-9_.\-/]*)\s*:(?:[^=]|$)")
        .unwrap_or_else(|error| panic!("invalid make target pattern: {error}"))
});
static JUST_RECIPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^@?([A-Za-z0-9][A-Za-z0-9_\-]*)(?:\s+[^:=]*)?:(?:[^=]|$)")
        .unwrap_or_else(|error| panic!("invalid just recipe pattern: {error}"))
});

/// Maps a script, target, or task name to its purpose.
pub fn purpose_of(name: &str) -> Option<CommandPurpose> {
    let lower = name.to_ascii_lowercase();
    let head = lower.split([':', '-', '_', '.']).next().unwrap_or(&lower);
    let purpose = match head {
        "test" | "tests" | "check" | "e2e" | "coverage" | "spec" => CommandPurpose::Test,
        "build" | "compile" | "all" | "dist" | "package" | "release" => CommandPurpose::Build,
        "start" | "serve" | "run" | "preview" => CommandPurpose::Run,
        "dev" | "watch" | "develop" => CommandPurpose::Dev,
        "lint" | "clippy" | "vet" | "typecheck" | "tsc" => CommandPurpose::Lint,
        "format" | "fmt" | "prettier" => CommandPurpose::Format,
        "bench" | "benchmark" | "benchmarks" => CommandPurpose::Bench,
        "install" | "deps" | "setup" | "bootstrap" => CommandPurpose::Install,
        _ => return None,
    };
    Some(purpose)
}

struct Collector {
    commands: Vec<CommandCandidate>,
    seen: BTreeSet<(String, String)>,
}

impl Collector {
    fn add(
        &mut self,
        command: String,
        purpose: CommandPurpose,
        dir: &str,
        source: String,
        evidence: Evidence,
    ) {
        if self.seen.insert((command.clone(), dir.to_owned())) {
            self.commands.push(CommandCandidate {
                command,
                purpose,
                working_directory: dir.to_owned(),
                source,
                verified: false,
                evidence: vec![evidence],
            });
        }
    }
}

fn package_json_scripts(text: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|json| json.get("scripts").and_then(|s| s.as_object()).cloned())
        .map(|scripts| scripts.keys().cloned().collect())
        .unwrap_or_default()
}

fn json_object_keys(text: &str, key: &str) -> Vec<String> {
    let text: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|json| json.get(key).and_then(|v| v.as_object()).cloned())
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default()
}

fn make_targets(text: &str) -> Vec<String> {
    let mut targets = Vec::new();
    for line in text.lines() {
        if line.starts_with(['\t', ' ', '#', '.']) {
            continue;
        }
        if let Some(captures) = MAKE_TARGET.captures(line) {
            let target = &captures[1];
            if !target.contains('%')
                && !target.contains('$')
                && !targets.iter().any(|t| t == target)
            {
                targets.push(target.to_owned());
            }
        }
    }
    targets
}

fn just_recipes(text: &str) -> Vec<String> {
    let mut recipes = Vec::new();
    for line in text.lines() {
        if line.starts_with([' ', '\t', '#', '[']) || line.contains(":=") {
            continue;
        }
        if let Some(captures) = JUST_RECIPE.captures(line) {
            let recipe = captures[1].to_owned();
            if !["set", "alias", "export", "import", "mod"].contains(&recipe.as_str()) {
                recipes.push(recipe);
            }
        }
    }
    recipes
}

fn taskfile_tasks(text: &str) -> Vec<String> {
    let Ok(document) = repodna_dependencies::yaml::load(text) else {
        return Vec::new();
    };
    document["tasks"]
        .as_hash()
        .map(|tasks| {
            tasks
                .keys()
                .filter_map(|key| match key {
                    Yaml::String(name) => Some(name.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Derives command candidates from metadata.
pub fn command_candidates(context: ToolContext<'_>) -> Vec<CommandCandidate> {
    let index = Index::new(context);
    let mut out = Collector {
        commands: Vec::new(),
        seen: BTreeSet::new(),
    };
    let contents = context.contents;
    let exists = |name: &str| index.named(name).is_some();

    // JavaScript: every package.json with scripts, run with the detected package manager.
    let manager = node_package_manager(&index).map(|(manager, _, _)| manager);
    let runner = match manager {
        Some("pnpm") => "pnpm",
        Some("Yarn") => "yarn",
        Some("Bun") => "bun",
        _ => "npm",
    };
    let mut manifests: Vec<(&String, &String)> = contents
        .iter()
        .filter(|(path, _)| paths::file_name(path) == "package.json")
        .collect();
    manifests.sort_by_key(|(path, _)| (paths::depth(path), path.as_str()));
    for (path, text) in manifests {
        let dir = paths::parent(path);
        if dir.split('/').any(|part| part == "node_modules") {
            continue;
        }
        let install = if runner == "npm" && exists("package-lock.json") {
            "npm ci".to_owned()
        } else {
            format!("{runner} install")
        };
        if dir.is_empty() || paths::depth(path) <= 3 {
            out.add(
                install,
                CommandPurpose::Install,
                dir,
                format!("{path} (package manager)"),
                Evidence::file(path.as_str()),
            );
        }
        for script in package_json_scripts(text) {
            let Some(purpose) = purpose_of(&script) else {
                continue;
            };
            let command = if runner == "npm" && script == "test" {
                "npm test".to_owned()
            } else {
                format!("{runner} run {script}")
            };
            out.add(
                command,
                purpose,
                dir,
                format!("{path} scripts.{script}"),
                Evidence::file(path.as_str()).with_note(format!("scripts.{script}")),
            );
        }
    }

    // Conventional commands of build systems (root manifests first).
    let conventional: [(&str, &[(&str, CommandPurpose)]); 9] = [
        (
            "Cargo.toml",
            &[
                ("cargo build", CommandPurpose::Build),
                ("cargo test", CommandPurpose::Test),
                ("cargo clippy --all-targets", CommandPurpose::Lint),
                ("cargo fmt --check", CommandPurpose::Format),
            ],
        ),
        (
            "go.mod",
            &[
                ("go build ./...", CommandPurpose::Build),
                ("go test ./...", CommandPurpose::Test),
                ("go vet ./...", CommandPurpose::Lint),
            ],
        ),
        (
            "Package.swift",
            &[
                ("swift build", CommandPurpose::Build),
                ("swift test", CommandPurpose::Test),
            ],
        ),
        (
            "composer.json",
            &[("composer install", CommandPurpose::Install)],
        ),
        ("Gemfile", &[("bundle install", CommandPurpose::Install)]),
        (
            "meson.build",
            &[
                ("meson setup build", CommandPurpose::Build),
                ("meson test -C build", CommandPurpose::Test),
            ],
        ),
        (
            "CMakeLists.txt",
            &[(
                "cmake -S . -B build && cmake --build build",
                CommandPurpose::Build,
            )],
        ),
        ("Dockerfile", &[("docker build .", CommandPurpose::Build)]),
        ("uv.lock", &[("uv sync", CommandPurpose::Install)]),
    ];
    for (manifest, commands) in conventional {
        if let Some(path) = index.named(manifest) {
            let dir = paths::parent(path);
            for (command, purpose) in commands.iter().copied() {
                out.add(
                    command.to_owned(),
                    purpose,
                    dir,
                    path.to_owned(),
                    Evidence::file(path),
                );
            }
        }
    }
    if let Some(path) = index.named("Cargo.toml")
        && (exists("main.rs") || contents.get(path).is_some_and(|t| t.contains("[[bin]]")))
    {
        out.add(
            "cargo run".to_owned(),
            CommandPurpose::Run,
            paths::parent(path),
            path.to_owned(),
            Evidence::file(path),
        );
    }
    if let Some(path) = index.named("CMakeLists.txt")
        && contents
            .get(path)
            .is_some_and(|t| t.contains("enable_testing"))
    {
        out.add(
            "ctest --test-dir build".to_owned(),
            CommandPurpose::Test,
            paths::parent(path),
            path.to_owned(),
            Evidence::file(path).with_note("enable_testing()"),
        );
    }
    let wrapper = |name: &str, fallback: &str| {
        if exists(name) {
            format!("./{name}")
        } else {
            fallback.to_owned()
        }
    };
    if let Some(path) = index.named("pom.xml") {
        let mvn = wrapper("mvnw", "mvn");
        out.add(
            format!("{mvn} -B package"),
            CommandPurpose::Build,
            paths::parent(path),
            path.to_owned(),
            Evidence::file(path),
        );
        out.add(
            format!("{mvn} -B test"),
            CommandPurpose::Test,
            paths::parent(path),
            path.to_owned(),
            Evidence::file(path),
        );
    }
    if let Some(path) = [
        "build.gradle.kts",
        "build.gradle",
        "settings.gradle.kts",
        "settings.gradle",
    ]
    .iter()
    .find_map(|name| index.named(name))
    {
        let gradle = wrapper("gradlew", "gradle");
        out.add(
            format!("{gradle} build"),
            CommandPurpose::Build,
            paths::parent(path),
            path.to_owned(),
            Evidence::file(path),
        );
        out.add(
            format!("{gradle} test"),
            CommandPurpose::Test,
            paths::parent(path),
            path.to_owned(),
            Evidence::file(path),
        );
    }
    if let Some(path) = index.file(|file| {
        matches!(
            paths::extension(file.path).as_deref(),
            Some("sln" | "csproj" | "fsproj")
        )
    }) {
        let dir = paths::parent(path);
        out.add(
            "dotnet build".to_owned(),
            CommandPurpose::Build,
            dir,
            path.to_owned(),
            Evidence::file(path),
        );
        out.add(
            "dotnet test".to_owned(),
            CommandPurpose::Test,
            dir,
            path.to_owned(),
            Evidence::file(path),
        );
    }
    if let Some(path) = index.named("pubspec.yaml") {
        let flutter = contents.get(path).is_some_and(|t| t.contains("flutter"));
        let tool = if flutter { "flutter" } else { "dart" };
        let dir = paths::parent(path);
        out.add(
            format!("{tool} pub get"),
            CommandPurpose::Install,
            dir,
            path.to_owned(),
            Evidence::file(path),
        );
        out.add(
            format!("{tool} test"),
            CommandPurpose::Test,
            dir,
            path.to_owned(),
            Evidence::file(path),
        );
    }
    if let Some(path) = index.named("Gemfile") {
        let dir = paths::parent(path);
        let rspec = exists(".rspec") || contents.get(path).is_some_and(|t| t.contains("rspec"));
        let test = if rspec {
            "bundle exec rspec"
        } else {
            "bundle exec rake test"
        };
        out.add(
            test.to_owned(),
            CommandPurpose::Test,
            dir,
            path.to_owned(),
            Evidence::file(path),
        );
    }

    // Python.
    if let Some(path) = index
        .named("pyproject.toml")
        .or_else(|| index.named("setup.py"))
    {
        let dir = paths::parent(path);
        let text = contents.get(path).map(String::as_str).unwrap_or_default();
        let poetry = text.contains("[tool.poetry");
        let uv = exists("uv.lock");
        let prefix = if poetry {
            "poetry run "
        } else if uv {
            "uv run "
        } else {
            ""
        };
        if poetry {
            out.add(
                "poetry install".to_owned(),
                CommandPurpose::Install,
                dir,
                path.to_owned(),
                Evidence::file(path),
            );
        } else if !uv {
            out.add(
                "pip install -e .".to_owned(),
                CommandPurpose::Install,
                dir,
                path.to_owned(),
                Evidence::file(path),
            );
        }
        let pytest = text.contains("pytest") || exists("pytest.ini") || exists("conftest.py");
        let test = if pytest {
            format!("{prefix}pytest")
        } else {
            format!("{prefix}python -m unittest")
        };
        out.add(
            test,
            CommandPurpose::Test,
            dir,
            path.to_owned(),
            Evidence::file(path),
        );
        if text.contains("[tool.ruff") || text.contains("\"ruff") {
            out.add(
                format!("{prefix}ruff check ."),
                CommandPurpose::Lint,
                dir,
                path.to_owned(),
                Evidence::file(path).with_note("ruff"),
            );
        }
    } else if let Some(path) = index.named("requirements.txt") {
        out.add(
            "pip install -r requirements.txt".to_owned(),
            CommandPurpose::Install,
            paths::parent(path),
            path.to_owned(),
            Evidence::file(path),
        );
    }
    for (name, command) in [("tox.ini", "tox"), ("noxfile.py", "nox")] {
        if let Some(path) = index.named(name) {
            out.add(
                command.to_owned(),
                CommandPurpose::Test,
                paths::parent(path),
                path.to_owned(),
                Evidence::file(path),
            );
        }
    }

    // Task runners declared in the repository.
    for (name, runner, parse) in [
        ("Makefile", "make", make_targets as fn(&str) -> Vec<String>),
        ("GNUmakefile", "make", make_targets),
        ("makefile", "make", make_targets),
        ("justfile", "just", just_recipes),
        ("Justfile", "just", just_recipes),
        ("Taskfile.yml", "task", taskfile_tasks),
        ("Taskfile.yaml", "task", taskfile_tasks),
    ] {
        let Some(path) = index.named(name) else {
            continue;
        };
        let Some(text) = contents.get(path) else {
            continue;
        };
        for target in parse(text) {
            if let Some(purpose) = purpose_of(&target) {
                out.add(
                    format!("{runner} {target}"),
                    purpose,
                    paths::parent(path),
                    format!("{path} {target}"),
                    Evidence::file(path).with_note(format!("target {target}")),
                );
            }
        }
    }
    for (name, key, runner) in [
        ("deno.json", "tasks", "deno task"),
        ("deno.jsonc", "tasks", "deno task"),
        ("composer.json", "scripts", "composer run"),
    ] {
        let Some(path) = index.named(name) else {
            continue;
        };
        let Some(text) = contents.get(path) else {
            continue;
        };
        for task in json_object_keys(text, key) {
            if let Some(purpose) = purpose_of(&task) {
                out.add(
                    format!("{runner} {task}"),
                    purpose,
                    paths::parent(path),
                    format!("{path} {key}.{task}"),
                    Evidence::file(path).with_note(format!("{key}.{task}")),
                );
            }
        }
    }
    if let Some(path) = [
        "docker-compose.yml",
        "docker-compose.yaml",
        "compose.yml",
        "compose.yaml",
    ]
    .iter()
    .find_map(|name| index.named(name))
    {
        out.add(
            "docker compose up".to_owned(),
            CommandPurpose::Run,
            paths::parent(path),
            path.to_owned(),
            Evidence::file(path),
        );
    }

    let mut commands = out.commands;
    commands.sort_by(|a, b| {
        a.purpose
            .cmp(&b.purpose)
            .then_with(|| {
                paths::depth(&a.working_directory).cmp(&paths::depth(&b.working_directory))
            })
            .then_with(|| a.working_directory.cmp(&b.working_directory))
    });
    commands.truncate(MAX_COMMANDS);
    commands
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectFile;
    use repodna_core::model::structure::FileCategory;
    use std::collections::BTreeMap;

    fn file(path: &str) -> ProjectFile<'_> {
        ProjectFile {
            path,
            category: FileCategory::Other,
            language: None,
            code_lines: 0,
            total_lines: 0,
        }
    }

    fn commands(
        files: &[&str],
        contents: &[(&str, &str)],
    ) -> Vec<(String, CommandPurpose, String)> {
        let files: Vec<ProjectFile<'_>> = files.iter().map(|path| file(path)).collect();
        let contents: BTreeMap<String, String> = contents
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect();
        let context = ToolContext {
            files: &files,
            contents: &contents,
            dependencies: &[],
        };
        command_candidates(context)
            .into_iter()
            .map(|c| (c.command, c.purpose, c.working_directory))
            .collect()
    }

    #[test]
    fn maps_names_to_purposes() {
        assert_eq!(purpose_of("test:unit"), Some(CommandPurpose::Test));
        assert_eq!(purpose_of("build-docs"), Some(CommandPurpose::Build));
        assert_eq!(purpose_of("fmt"), Some(CommandPurpose::Format));
        assert_eq!(purpose_of("postinstall"), None);
        assert_eq!(purpose_of("deploy"), None);
    }

    #[test]
    fn derives_commands_from_scripts_and_build_systems() {
        let found = commands(
            &[
                "Cargo.toml",
                "src/main.rs",
                "web/package.json",
                "web/pnpm-lock.yaml",
                "Makefile",
            ],
            &[
                (
                    "web/package.json",
                    r#"{"scripts":{"dev":"vite","build":"vite build","test":"vitest","deploy":"x"}}"#,
                ),
                (
                    "Makefile",
                    "VERSION := 1\n.PHONY: test\nall: build\ntest:\n\tcargo test\nrelease: all\n%.o: %.c\n",
                ),
                ("Cargo.toml", "[package]\nname = \"x\"\n"),
            ],
        );
        let has = |command: &str, purpose: CommandPurpose, dir: &str| {
            found
                .iter()
                .any(|(c, p, d)| c == command && *p == purpose && d == dir)
        };
        assert!(has("pnpm install", CommandPurpose::Install, "web"));
        assert!(has("pnpm run dev", CommandPurpose::Dev, "web"));
        assert!(has("pnpm run build", CommandPurpose::Build, "web"));
        assert!(has("pnpm run test", CommandPurpose::Test, "web"));
        assert!(has("cargo test", CommandPurpose::Test, ""));
        assert!(has("cargo run", CommandPurpose::Run, ""));
        assert!(has("make test", CommandPurpose::Test, ""));
        assert!(has("make all", CommandPurpose::Build, ""));
        assert!(has("make release", CommandPurpose::Build, ""));
        assert!(
            !found
                .iter()
                .any(|(c, _, _)| c.contains("deploy") || c.contains("%"))
        );
        assert_eq!(found[0].1, CommandPurpose::Install);
    }

    #[test]
    fn derives_python_jvm_and_task_runner_commands() {
        let found = commands(
            &[
                "pyproject.toml",
                "uv.lock",
                "pom.xml",
                "mvnw",
                "justfile",
                "Taskfile.yml",
            ],
            &[
                (
                    "pyproject.toml",
                    "[project]\nname = \"x\"\n[tool.pytest.ini_options]\n[tool.ruff]\n",
                ),
                (
                    "justfile",
                    "set shell := [\"bash\"]\ndefault:\n  just --list\nlint *args:\n  ruff check\n",
                ),
                (
                    "Taskfile.yml",
                    "version: '3'\ntasks:\n  build:\n    cmds: [go build]\n  deploy:\n    cmds: [x]\n",
                ),
            ],
        );
        let names: Vec<&str> = found.iter().map(|(c, _, _)| c.as_str()).collect();
        for expected in [
            "uv sync",
            "uv run pytest",
            "uv run ruff check .",
            "./mvnw -B package",
            "./mvnw -B test",
            "just lint",
            "task build",
        ] {
            assert!(
                names.contains(&expected),
                "{expected} missing from {names:?}"
            );
        }
        assert!(!names.contains(&"task deploy"));
        assert_eq!(make_targets("a b: c\n"), Vec::<String>::new());
        assert_eq!(just_recipes("build:\n@quiet:\n"), vec!["build", "quiet"]);
    }
}
