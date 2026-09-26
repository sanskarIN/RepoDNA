//! Python: `pyproject.toml` (PEP 621, Poetry, dependency groups), `requirements*.txt`,
//! `Pipfile`, `setup.py`, and Poetry, uv, PDM, and Pipenv lockfiles.

use std::sync::LazyLock;

use regex::Regex;
use repodna_core::model::dependencies::{DependencyScope, ManifestKind};
use serde_json::Value;

use crate::model::{
    DeclaredDependency, EcosystemProvider, FileMatch, ParsedLockfile, ParsedManifest, file_name,
};

/// Python provider.
pub struct Python;

/// Normalizes a distribution name per PEP 503 (lowercase, runs of `-_.` become `-`).
pub fn normalize(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut separator = false;
    for c in name.trim().chars() {
        if matches!(c, '-' | '_' | '.') {
            separator = true;
        } else {
            if separator && !out.is_empty() {
                out.push('-');
            }
            separator = false;
            out.push(c.to_ascii_lowercase());
        }
    }
    out
}

static PEP508_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*([A-Za-z0-9][A-Za-z0-9._-]*)\s*(?:\[[^\]]*\])?\s*(.*)$")
        .unwrap_or_else(|error| panic!("invalid PEP 508 pattern: {error}"))
});

/// Parses a PEP 508 requirement such as `requests[security]>=2.0; python_version<"3.8"`.
pub fn parse_requirement(text: &str, scope: DependencyScope) -> Option<DeclaredDependency> {
    let text = text.split(';').next().unwrap_or(text).trim();
    let captures = PEP508_NAME.captures(text)?;
    let name = normalize(&captures[1]);
    let rest = captures[2].trim();
    let (requirement, source) = if let Some(url) = rest.strip_prefix('@') {
        (
            Some(url.trim().to_owned()),
            if url.contains("git") { "git" } else { "path" },
        )
    } else {
        ((!rest.is_empty()).then(|| rest.to_owned()), "registry")
    };
    Some(DeclaredDependency::registry(name, requirement, scope).with_source(source))
}

fn toml_str(value: Option<&toml::Value>) -> Option<String> {
    value.and_then(toml::Value::as_str).map(str::to_owned)
}

fn poetry_dependency(
    name: &str,
    value: &toml::Value,
    scope: DependencyScope,
) -> DeclaredDependency {
    let name = normalize(name);
    match value {
        toml::Value::String(requirement) => {
            DeclaredDependency::registry(name, Some(requirement.clone()), scope)
        }
        toml::Value::Table(table) => {
            let source = if table.contains_key("path") {
                "path"
            } else if table.contains_key("git") {
                "git"
            } else {
                "registry"
            };
            let optional = table.get("optional").and_then(toml::Value::as_bool) == Some(true);
            let scope = if optional {
                DependencyScope::Optional
            } else {
                scope
            };
            DeclaredDependency::registry(name, toml_str(table.get("version")), scope)
                .with_source(source)
        }
        _ => DeclaredDependency::registry(name, None, scope),
    }
}

fn requirement_list(
    value: Option<&toml::Value>,
    scope: DependencyScope,
    manifest: &mut ParsedManifest,
) {
    if let Some(items) = value.and_then(toml::Value::as_array) {
        for item in items {
            match item
                .as_str()
                .and_then(|text| parse_requirement(text, scope))
            {
                Some(dependency) => manifest.dependencies.push(dependency),
                // `{ include-group = "…" }` entries of PEP 735 groups are references, not packages.
                None if item.is_table() => {}
                None => manifest.partial = true,
            }
        }
    }
}

impl Python {
    fn parse_pyproject(content: &str) -> Result<ParsedManifest, String> {
        let table: toml::Table = content
            .parse()
            .map_err(|error: toml::de::Error| error.to_string())?;
        let mut manifest = ParsedManifest::default();
        if let Some(project) = table.get("project").and_then(toml::Value::as_table) {
            manifest.package_name = toml_str(project.get("name"));
            manifest.package_version = toml_str(project.get("version"));
            manifest.description = toml_str(project.get("description"));
            if let Some(python) = toml_str(project.get("requires-python")) {
                manifest.requirements.push(("Python".to_owned(), python));
            }
            requirement_list(
                project.get("dependencies"),
                DependencyScope::Runtime,
                &mut manifest,
            );
            if let Some(extras) = project
                .get("optional-dependencies")
                .and_then(toml::Value::as_table)
            {
                for group in extras.values() {
                    requirement_list(Some(group), DependencyScope::Optional, &mut manifest);
                }
            }
        }
        if let Some(groups) = table
            .get("dependency-groups")
            .and_then(toml::Value::as_table)
        {
            for group in groups.values() {
                requirement_list(Some(group), DependencyScope::Development, &mut manifest);
            }
        }
        let poetry = table
            .get("tool")
            .and_then(|tool| tool.get("poetry"))
            .and_then(toml::Value::as_table);
        if let Some(poetry) = poetry {
            manifest.package_name = manifest
                .package_name
                .or_else(|| toml_str(poetry.get("name")));
            manifest.package_version = manifest
                .package_version
                .or_else(|| toml_str(poetry.get("version")));
            manifest.description = manifest
                .description
                .or_else(|| toml_str(poetry.get("description")));
            let mut add = |entries: Option<&toml::Value>, scope: DependencyScope| {
                if let Some(entries) = entries.and_then(toml::Value::as_table) {
                    for (name, value) in entries {
                        if name == "python" {
                            if let Some(version) = value.as_str() {
                                manifest
                                    .requirements
                                    .push(("Python".to_owned(), version.to_owned()));
                            }
                            continue;
                        }
                        manifest
                            .dependencies
                            .push(poetry_dependency(name, value, scope));
                    }
                }
            };
            add(poetry.get("dependencies"), DependencyScope::Runtime);
            add(poetry.get("dev-dependencies"), DependencyScope::Development);
            if let Some(groups) = poetry.get("group").and_then(toml::Value::as_table) {
                for group in groups.values() {
                    add(group.get("dependencies"), DependencyScope::Development);
                }
            }
        }
        if let Some(members) = table
            .get("tool")
            .and_then(|tool| tool.get("uv"))
            .and_then(|uv| uv.get("workspace"))
            .and_then(|workspace| workspace.get("members"))
            .and_then(toml::Value::as_array)
        {
            manifest.workspace_tool = Some("uv-workspace".to_owned());
            manifest.workspace_members = members
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_owned)
                .collect();
        }
        Ok(manifest)
    }

    fn parse_requirements_txt(path: &str, content: &str) -> ParsedManifest {
        let lower = file_name(path).to_ascii_lowercase();
        let scope = if lower.contains("dev")
            || lower.contains("test")
            || lower.contains("lint")
            || lower.contains("doc")
        {
            DependencyScope::Development
        } else {
            DependencyScope::Runtime
        };
        let mut manifest = ParsedManifest::default();
        for line in content.lines() {
            let line = match line.find(" #") {
                Some(position) => &line[..position],
                None => line,
            };
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
                continue;
            }
            match parse_requirement(line, scope) {
                Some(dependency) => manifest.dependencies.push(dependency),
                None => manifest.partial = true,
            }
        }
        manifest
    }

    fn parse_pipfile(content: &str) -> Result<ParsedManifest, String> {
        let table: toml::Table = content
            .parse()
            .map_err(|error: toml::de::Error| error.to_string())?;
        let mut manifest = ParsedManifest::default();
        for (section, scope) in [
            ("packages", DependencyScope::Runtime),
            ("dev-packages", DependencyScope::Development),
        ] {
            if let Some(entries) = table.get(section).and_then(toml::Value::as_table) {
                for (name, value) in entries {
                    let dependency = match value {
                        toml::Value::String(requirement) if requirement == "*" => {
                            DeclaredDependency::registry(normalize(name), None, scope)
                        }
                        _ => poetry_dependency(name, value, scope),
                    };
                    manifest.dependencies.push(dependency);
                }
            }
        }
        if let Some(version) = table
            .get("requires")
            .and_then(|requires| requires.get("python_version"))
            .and_then(toml::Value::as_str)
        {
            manifest
                .requirements
                .push(("Python".to_owned(), version.to_owned()));
        }
        Ok(manifest)
    }

    fn parse_setup_py(content: &str) -> ParsedManifest {
        static INSTALL_REQUIRES: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"(?s)install_requires\s*=\s*\[(.*?)\]")
                .unwrap_or_else(|error| panic!("invalid setup.py pattern: {error}"))
        });
        static STRING: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r#"["']([^"']+)["']"#)
                .unwrap_or_else(|error| panic!("invalid string pattern: {error}"))
        });
        static NAME: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r#"\bname\s*=\s*["']([^"']+)["']"#)
                .unwrap_or_else(|error| panic!("invalid name pattern: {error}"))
        });
        // setup.py is executable code; only literal values are read, so results are partial.
        let mut manifest = ParsedManifest {
            partial: true,
            package_name: NAME.captures(content).map(|c| c[1].to_owned()),
            ..ParsedManifest::default()
        };
        if let Some(block) = INSTALL_REQUIRES.captures(content) {
            for literal in STRING.captures_iter(&block[1]) {
                if let Some(dependency) = parse_requirement(&literal[1], DependencyScope::Runtime) {
                    manifest.dependencies.push(dependency);
                }
            }
        }
        manifest
    }
}

impl EcosystemProvider for Python {
    fn ecosystem(&self) -> &'static str {
        "pypi"
    }

    fn matches(&self, path: &str) -> Option<FileMatch> {
        let name = file_name(path);
        let lower = name.to_ascii_lowercase();
        let kind = if matches!(name, "pyproject.toml" | "setup.py" | "Pipfile")
            || (lower.starts_with("requirements")
                && (lower.ends_with(".txt") || lower.ends_with(".in")))
        {
            ManifestKind::Manifest
        } else if matches!(
            name,
            "poetry.lock" | "Pipfile.lock" | "uv.lock" | "pdm.lock"
        ) {
            ManifestKind::Lockfile
        } else {
            return None;
        };
        Some(FileMatch { kind })
    }

    fn parse_manifest(&self, path: &str, content: &str) -> Result<ParsedManifest, String> {
        match file_name(path) {
            "pyproject.toml" => Self::parse_pyproject(content),
            "Pipfile" => Self::parse_pipfile(content),
            "setup.py" => Ok(Self::parse_setup_py(content)),
            _ => Ok(Self::parse_requirements_txt(path, content)),
        }
    }

    fn parse_lockfile(&self, path: &str, content: &str) -> Result<ParsedLockfile, String> {
        if file_name(path) == "Pipfile.lock" {
            let json: Value = serde_json::from_str(content).map_err(|error| error.to_string())?;
            let mut packages = Vec::new();
            for section in ["default", "develop"] {
                if let Some(entries) = json.get(section).and_then(Value::as_object) {
                    for (name, value) in entries {
                        if let Some(version) = value.get("version").and_then(Value::as_str) {
                            packages.push((
                                normalize(name),
                                version.trim_start_matches("==").to_owned(),
                            ));
                        }
                    }
                }
            }
            return Ok(ParsedLockfile { packages });
        }
        let table: toml::Table = content
            .parse()
            .map_err(|error: toml::de::Error| error.to_string())?;
        let packages = table
            .get("package")
            .and_then(toml::Value::as_array)
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| {
                        Some((
                            normalize(entry.get("name")?.as_str()?),
                            entry.get("version")?.as_str()?.to_owned(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(ParsedLockfile { packages })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_names_and_parses_requirements() {
        assert_eq!(normalize("Django_REST.framework"), "django-rest-framework");
        let requirement = parse_requirement(
            "requests[security]>=2.31; python_version<'3.8'",
            DependencyScope::Runtime,
        )
        .unwrap();
        assert_eq!(requirement.name, "requests");
        assert_eq!(requirement.requirement.as_deref(), Some(">=2.31"));
        let url = parse_requirement(
            "pkg @ git+https://example.invalid/pkg.git",
            DependencyScope::Runtime,
        )
        .unwrap();
        assert_eq!(url.source, "git");
        assert!(parse_requirement("", DependencyScope::Runtime).is_none());
    }

    #[test]
    fn parses_pyproject_variants() {
        let manifest = Python
            .parse_manifest(
                "pyproject.toml",
                r#"
[project]
name = "demo"
version = "1.0"
requires-python = ">=3.10"
dependencies = ["fastapi>=0.110", "Pydantic"]
[project.optional-dependencies]
docs = ["mkdocs"]
[dependency-groups]
dev = ["pytest", { include-group = "docs" }]
[tool.poetry.group.lint.dependencies]
ruff = "^0.5"
"#,
            )
            .unwrap();
        assert_eq!(manifest.package_name.as_deref(), Some("demo"));
        assert!(!manifest.partial);
        let names: Vec<_> = manifest
            .dependencies
            .iter()
            .map(|d| (d.name.as_str(), d.scope))
            .collect();
        assert!(names.contains(&("fastapi", DependencyScope::Runtime)));
        assert!(names.contains(&("pydantic", DependencyScope::Runtime)));
        assert!(names.contains(&("mkdocs", DependencyScope::Optional)));
        assert!(names.contains(&("pytest", DependencyScope::Development)));
        assert!(names.contains(&("ruff", DependencyScope::Development)));
        assert_eq!(
            manifest.requirements,
            vec![("Python".into(), ">=3.10".into())]
        );
    }

    #[test]
    fn parses_poetry_projects() {
        let manifest = Python
            .parse_manifest(
                "pyproject.toml",
                "[tool.poetry]\nname = \"svc\"\n[tool.poetry.dependencies]\npython = \"^3.11\"\nflask = \"^3.0\"\nlocal = { path = \"../local\" }\n[tool.poetry.dev-dependencies]\nblack = \"*\"\n",
            )
            .unwrap();
        assert_eq!(manifest.package_name.as_deref(), Some("svc"));
        assert_eq!(manifest.dependencies.len(), 3);
        assert_eq!(manifest.requirements[0].1, "^3.11");
    }

    #[test]
    fn parses_requirements_pipfile_and_setup_py() {
        let requirements = Python
            .parse_manifest(
                "requirements-dev.txt",
                "# tools\n-r requirements.txt\npytest==8.0  # tests\nblack\n\n",
            )
            .unwrap();
        assert_eq!(requirements.dependencies.len(), 2);
        assert!(
            requirements
                .dependencies
                .iter()
                .all(|d| d.scope == DependencyScope::Development)
        );
        let pipfile = Python
            .parse_manifest("Pipfile", "[packages]\nrequests = \"*\"\n[dev-packages]\npytest = \">=7\"\n[requires]\npython_version = \"3.12\"\n")
            .unwrap();
        assert_eq!(pipfile.dependencies.len(), 2);
        let setup = Python
            .parse_manifest(
                "setup.py",
                "setup(name='legacy', install_requires=[\n 'six>=1.0',\n \"attrs\"\n])\n",
            )
            .unwrap();
        assert!(setup.partial);
        assert_eq!(setup.package_name.as_deref(), Some("legacy"));
        assert_eq!(setup.dependencies.len(), 2);
    }

    #[test]
    fn parses_lockfiles() {
        let poetry = Python
            .parse_lockfile(
                "poetry.lock",
                "[[package]]\nname = \"Flask\"\nversion = \"3.0.0\"\n",
            )
            .unwrap();
        assert_eq!(poetry.packages, vec![("flask".into(), "3.0.0".into())]);
        let pipenv = Python
            .parse_lockfile(
                "Pipfile.lock",
                r#"{"default":{"requests":{"version":"==2.31.0"}},"develop":{}}"#,
            )
            .unwrap();
        assert_eq!(pipenv.packages, vec![("requests".into(), "2.31.0".into())]);
    }
}
