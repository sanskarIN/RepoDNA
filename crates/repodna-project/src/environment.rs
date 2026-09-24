//! Environment requirements, container definitions, and configuration files.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;
use repodna_core::model::project::{ConfigFileInfo, EnvironmentRequirement, RequirementKind};
use repodna_core::paths;

use crate::{ProjectFile, is_ci_file};

/// Maximum configuration files listed.
pub const MAX_CONFIG_FILES: usize = 200;

static IMAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*(?:-\s*)?image:\s*["']?([^\s"'#]+)"#)
        .unwrap_or_else(|error| panic!("invalid image pattern: {error}"))
});

/// Well-known service images and their display names.
const SERVICES: &[(&str, &str)] = &[
    ("postgres", "PostgreSQL"),
    ("postgis", "PostgreSQL"),
    ("mysql", "MySQL"),
    ("mariadb", "MariaDB"),
    ("redis", "Redis"),
    ("valkey", "Valkey"),
    ("mongo", "MongoDB"),
    ("rabbitmq", "RabbitMQ"),
    ("elasticsearch", "Elasticsearch"),
    ("opensearch", "OpenSearch"),
    ("memcached", "Memcached"),
    ("cp-kafka", "Kafka"),
    ("kafka", "Kafka"),
    ("minio", "MinIO"),
    ("nats", "NATS"),
    ("localstack", "LocalStack"),
    ("clickhouse-server", "ClickHouse"),
    ("cassandra", "Cassandra"),
];

fn requirement(
    kind: RequirementKind,
    name: &str,
    version: Option<String>,
    source: &str,
) -> EnvironmentRequirement {
    EnvironmentRequirement {
        kind,
        name: name.to_owned(),
        version: version.filter(|v| !v.is_empty()),
        source: source.to_owned(),
    }
}

fn first_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.trim_start_matches('v').to_owned())
}

/// The service named by a container image such as `postgres:16-alpine`.
fn service(image: &str) -> Option<(&'static str, Option<String>)> {
    let (name, tag) = match image.rsplit_once(':') {
        Some((name, tag)) if !tag.contains('/') => (name, Some(tag.to_owned())),
        _ => (image, None),
    };
    let base = name.rsplit('/').next().unwrap_or(name);
    SERVICES
        .iter()
        .find(|(image, _)| base == *image)
        .map(|(_, display)| (*display, tag.filter(|tag| tag != "latest")))
}

fn tool_versions(text: &str, source: &str) -> Vec<EnvironmentRequirement> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let tool = parts.next().filter(|tool| !tool.starts_with('#'))?;
            let version = parts.next().map(str::to_owned);
            let (kind, name) = match tool {
                "nodejs" | "node" => (RequirementKind::Runtime, "Node.js"),
                "python" => (RequirementKind::Runtime, "Python"),
                "ruby" => (RequirementKind::Runtime, "Ruby"),
                "java" => (RequirementKind::Runtime, "Java"),
                "elixir" => (RequirementKind::Runtime, "Elixir"),
                "erlang" => (RequirementKind::Runtime, "Erlang/OTP"),
                "deno" => (RequirementKind::Runtime, "Deno"),
                "bun" => (RequirementKind::Runtime, "Bun"),
                "golang" | "go" => (RequirementKind::Toolchain, "Go"),
                "rust" => (RequirementKind::Toolchain, "Rust"),
                "pnpm" => (RequirementKind::PackageManager, "pnpm"),
                "yarn" => (RequirementKind::PackageManager, "Yarn"),
                other => {
                    return Some(requirement(
                        RequirementKind::Toolchain,
                        other,
                        version,
                        source,
                    ));
                }
            };
            Some(requirement(kind, name, version, source))
        })
        .collect()
}

/// Detects environment requirements from tool-version files, package.json engines, SDK
/// pins, container definitions, and service images, merged with `declared` requirements
/// from manifests.
pub fn environment_requirements(
    files: &[ProjectFile<'_>],
    contents: &BTreeMap<String, String>,
    declared: &[EnvironmentRequirement],
) -> Vec<EnvironmentRequirement> {
    let mut found: Vec<EnvironmentRequirement> = declared.to_vec();
    for (path, text) in contents {
        let name = paths::file_name(path);
        let single = |kind: RequirementKind, display: &str| {
            vec![requirement(kind, display, first_line(text), path)]
        };
        let items = match name {
            ".nvmrc" | ".node-version" => single(RequirementKind::Runtime, "Node.js"),
            ".python-version" => single(RequirementKind::Runtime, "Python"),
            ".ruby-version" => single(RequirementKind::Runtime, "Ruby"),
            ".java-version" => single(RequirementKind::Runtime, "Java"),
            ".go-version" => single(RequirementKind::Toolchain, "Go"),
            ".terraform-version" => single(RequirementKind::Toolchain, "Terraform"),
            "rust-toolchain" => single(RequirementKind::Toolchain, "Rust"),
            "rust-toolchain.toml" => {
                let channel = text.parse::<toml::Table>().ok().and_then(|table| {
                    table
                        .get("toolchain")?
                        .get("channel")?
                        .as_str()
                        .map(str::to_owned)
                });
                vec![requirement(
                    RequirementKind::Toolchain,
                    "Rust",
                    channel,
                    path,
                )]
            }
            ".tool-versions" => tool_versions(text, path),
            "global.json" => {
                let version = serde_json::from_str::<serde_json::Value>(text)
                    .ok()
                    .and_then(|json| json["sdk"]["version"].as_str().map(str::to_owned));
                vec![requirement(RequirementKind::Sdk, ".NET SDK", version, path)]
            }
            "runtime.txt" => first_line(text)
                .and_then(|line| {
                    let (runtime, version) = line.split_once('-')?;
                    let display = match runtime {
                        "python" => "Python",
                        "ruby" => "Ruby",
                        "node" | "nodejs" => "Node.js",
                        _ => return None,
                    };
                    Some(vec![requirement(
                        RequirementKind::Runtime,
                        display,
                        Some(version.to_owned()),
                        path,
                    )])
                })
                .unwrap_or_default(),
            "package.json" if paths::depth(path) <= 2 => {
                let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
                    continue;
                };
                let mut items = Vec::new();
                if let Some(engines) = json["engines"].as_object() {
                    for (engine, version) in engines {
                        let (kind, display) = match engine.as_str() {
                            "node" => (RequirementKind::Runtime, "Node.js"),
                            "npm" => (RequirementKind::PackageManager, "npm"),
                            "pnpm" => (RequirementKind::PackageManager, "pnpm"),
                            "yarn" => (RequirementKind::PackageManager, "Yarn"),
                            _ => continue,
                        };
                        items.push(requirement(
                            kind,
                            display,
                            version.as_str().map(str::to_owned),
                            path,
                        ));
                    }
                }
                if let Some((manager, version)) = json["packageManager"]
                    .as_str()
                    .and_then(|declared| declared.split_once('@'))
                {
                    let version = version.split('+').next().unwrap_or(version).to_owned();
                    items.push(requirement(
                        RequirementKind::PackageManager,
                        manager,
                        Some(version),
                        path,
                    ));
                }
                items
            }
            _ => Vec::new(),
        };
        found.extend(items);

        // Container images of services, in Compose files and CI configurations.
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("docker-compose") || lower.starts_with("compose.") || is_ci_file(path)
        {
            for line in text.lines() {
                if let Some(captures) = IMAGE.captures(line)
                    && let Some((display, version)) = service(&captures[1])
                {
                    found.push(requirement(
                        RequirementKind::Service,
                        display,
                        version,
                        path,
                    ));
                }
            }
        }
    }
    if let Some(dockerfile) = containers(files)
        .into_iter()
        .find(|path| !path.ends_with(".json"))
    {
        found.push(requirement(
            RequirementKind::Container,
            "Docker",
            None,
            &dockerfile,
        ));
    }

    let mut seen = BTreeSet::new();
    found.retain(|r| seen.insert((r.name.clone(), r.version.clone(), r.source.clone())));
    found.sort_by(|a, b| {
        (a.kind as u8)
            .cmp(&(b.kind as u8))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.source.cmp(&b.source))
    });
    found
}

/// Container definitions: Dockerfiles, Compose files, and dev container configurations.
pub fn containers(files: &[ProjectFile<'_>]) -> Vec<String> {
    let mut found: Vec<String> = files
        .iter()
        .filter(|file| {
            let lower = paths::file_name(file.path).to_ascii_lowercase();
            lower.starts_with("dockerfile")
                || lower.ends_with(".dockerfile")
                || lower == "containerfile"
                || ((lower.starts_with("docker-compose") || lower.starts_with("compose."))
                    && matches!(paths::extension(&lower).as_deref(), Some("yml" | "yaml")))
                || file.path.ends_with(".devcontainer/devcontainer.json")
                || file.path == ".devcontainer.json"
        })
        .map(|file| file.path.to_owned())
        .collect();
    found.sort_by_key(|path| (paths::depth(path), path.clone()));
    found
}

/// Known configuration files and what they configure.
pub fn configuration_files(files: &[ProjectFile<'_>]) -> Vec<ConfigFileInfo> {
    let purpose = |path: &str| -> Option<&'static str> {
        let name = paths::file_name(path);
        let lower = name.to_ascii_lowercase();
        let purpose = match lower.as_str() {
            ".editorconfig" => "Editor settings",
            ".gitignore" => "Files Git ignores",
            ".gitattributes" => "Git attributes (line endings, diff and merge settings)",
            ".dockerignore" => "Files excluded from Docker build contexts",
            "rustfmt.toml" | ".rustfmt.toml" => "Rust formatting",
            "clippy.toml" | ".clippy.toml" => "Rust linting (Clippy)",
            "deny.toml" => "Dependency policy (cargo-deny)",
            ".pre-commit-config.yaml" => "Pre-commit hooks",
            "ruff.toml" | ".ruff.toml" => "Python linting and formatting (Ruff)",
            "mypy.ini" | ".mypy.ini" => "Python type checking (mypy)",
            ".flake8" => "Python linting (flake8)",
            ".pylintrc" | "pylintrc" => "Python linting (Pylint)",
            ".golangci.yml" | ".golangci.yaml" => "Go linting (golangci-lint)",
            ".rubocop.yml" => "Ruby linting (RuboCop)",
            ".swiftlint.yml" => "Swift linting (SwiftLint)",
            ".clang-format" => "C and C++ formatting",
            ".clang-tidy" => "C and C++ linting",
            "phpstan.neon" | "phpstan.neon.dist" => "PHP static analysis (PHPStan)",
            "biome.json" | "biome.jsonc" => "Formatting and linting (Biome)",
            ".npmrc" => "npm settings",
            "renovate.json" | ".renovaterc" | ".renovaterc.json" => "Dependency updates (Renovate)",
            "dependabot.yml" | "dependabot.yaml" => "Dependency updates (Dependabot)",
            "codecov.yml" | ".codecov.yml" => "Coverage reporting (Codecov)",
            "sonar-project.properties" => "Static analysis (SonarQube)",
            "codeowners" => "Code ownership",
            ".env.example" | ".env.sample" | ".env.template" => "Environment variable template",
            _ if lower.starts_with("tsconfig") && lower.ends_with(".json") => {
                "TypeScript compiler options"
            }
            _ if lower.starts_with(".prettierrc") || lower.starts_with("prettier.config.") => {
                "Code formatting (Prettier)"
            }
            _ if lower.starts_with(".eslintrc") || lower.starts_with("eslint.config.") => {
                "JavaScript linting (ESLint)"
            }
            _ if lower.starts_with(".stylelintrc") || lower.starts_with("stylelint.config.") => {
                "CSS linting (Stylelint)"
            }
            _ if lower.starts_with("babel.config.") || lower == ".babelrc" => {
                "JavaScript transpiling (Babel)"
            }
            _ if lower.starts_with(".markdownlint") => "Markdown linting",
            _ => return None,
        };
        Some(purpose)
    };
    let mut found: Vec<ConfigFileInfo> = files
        .iter()
        .filter(|file| !file.path.split('/').any(|part| part == "node_modules"))
        .filter_map(|file| {
            purpose(file.path).map(|purpose| ConfigFileInfo {
                path: file.path.to_owned(),
                purpose: purpose.to_owned(),
            })
        })
        .collect();
    found.sort_by_key(|info| (paths::depth(&info.path), info.path.clone()));
    found.truncate(MAX_CONFIG_FILES);
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::structure::FileCategory;

    fn file(path: &str) -> ProjectFile<'_> {
        ProjectFile {
            path,
            category: FileCategory::Configuration,
            language: None,
            code_lines: 0,
            total_lines: 0,
        }
    }

    #[test]
    fn collects_requirements_from_many_sources() {
        let files = [
            file("Dockerfile"),
            file("docker-compose.yml"),
            file(".nvmrc"),
        ];
        let contents: BTreeMap<String, String> = [
            (".nvmrc", "v20.11.1\n"),
            ("rust-toolchain.toml", "[toolchain]\nchannel = \"1.98\"\n"),
            (".tool-versions", "python 3.12.2\nterraform 1.9.0\n"),
            ("global.json", r#"{"sdk":{"version":"8.0.100"}}"#),
            ("package.json", r#"{"engines":{"node":">=20"},"packageManager":"pnpm@9.1.0+sha256.abc"}"#),
            ("docker-compose.yml", "services:\n  db:\n    image: postgres:16-alpine\n  cache:\n    image: \"redis\"\n  app:\n    image: ghcr.io/acme/app:1.0\n"),
            (".github/workflows/ci.yml", "services:\n  mysql:\n    image: mysql:8.4\n"),
        ]
        .into_iter()
        .map(|(path, text)| (path.to_owned(), text.to_owned()))
        .collect();
        let declared = [requirement(
            RequirementKind::Toolchain,
            "Go",
            Some("1.22".into()),
            "go.mod",
        )];
        let found = environment_requirements(&files, &contents, &declared);
        let summary: Vec<(RequirementKind, &str, Option<&str>)> = found
            .iter()
            .map(|r| (r.kind, r.name.as_str(), r.version.as_deref()))
            .collect();
        assert_eq!(
            summary,
            vec![
                (RequirementKind::Runtime, "Node.js", Some("20.11.1")),
                (RequirementKind::Runtime, "Node.js", Some(">=20")),
                (RequirementKind::Runtime, "Python", Some("3.12.2")),
                (RequirementKind::Toolchain, "Go", Some("1.22")),
                (RequirementKind::Toolchain, "Rust", Some("1.98")),
                (RequirementKind::Toolchain, "terraform", Some("1.9.0")),
                (RequirementKind::PackageManager, "pnpm", Some("9.1.0")),
                (RequirementKind::Sdk, ".NET SDK", Some("8.0.100")),
                (RequirementKind::Container, "Docker", None),
                (RequirementKind::Service, "MySQL", Some("8.4")),
                (RequirementKind::Service, "PostgreSQL", Some("16-alpine")),
                (RequirementKind::Service, "Redis", None),
            ]
        );
    }

    #[test]
    fn lists_containers_and_configuration_files() {
        let files = [
            file("Dockerfile"),
            file("deploy/api.dockerfile"),
            file("compose.yaml"),
            file(".devcontainer/devcontainer.json"),
            file(".editorconfig"),
            file("web/tsconfig.app.json"),
            file("web/node_modules/x/.eslintrc.json"),
            file(".github/CODEOWNERS"),
            file("src/main.rs"),
        ];
        assert_eq!(
            containers(&files),
            vec![
                "Dockerfile",
                "compose.yaml",
                ".devcontainer/devcontainer.json",
                "deploy/api.dockerfile"
            ]
        );
        let configs = configuration_files(&files);
        let paths: Vec<&str> = configs.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                ".editorconfig",
                ".github/CODEOWNERS",
                "web/tsconfig.app.json"
            ]
        );
        assert_eq!(configs[0].purpose, "Editor settings");
    }
}
