//! Build systems and test frameworks detected from files, manifests, and dependencies.

use std::collections::{BTreeMap, HashMap};

use repodna_core::confidence::Confidence;
use repodna_core::evidence::Evidence;
use repodna_core::model::project::DetectedTool;
use repodna_core::model::structure::FileCategory;
use repodna_core::paths;

use crate::ProjectFile;

/// What tool detection can see.
#[derive(Debug, Clone, Copy)]
pub struct ToolContext<'a> {
    /// Repository files.
    pub files: &'a [ProjectFile<'a>],
    /// Contents of metadata files, keyed by path.
    pub contents: &'a BTreeMap<String, String>,
    /// Declared dependencies as `(ecosystem, name)`.
    pub dependencies: &'a [(String, String)],
}

/// Indexes used by the detectors.
pub(crate) struct Index<'a> {
    by_name: HashMap<String, Vec<&'a str>>,
    context: ToolContext<'a>,
}

impl<'a> Index<'a> {
    pub(crate) fn new(context: ToolContext<'a>) -> Self {
        let mut by_name: HashMap<String, Vec<&'a str>> = HashMap::new();
        for file in context.files {
            by_name
                .entry(paths::file_name(file.path).to_owned())
                .or_default()
                .push(file.path);
        }
        for paths in by_name.values_mut() {
            paths.sort_by_key(|path| (paths::depth(path), *path));
        }
        Self { by_name, context }
    }

    /// Shallowest file with exactly this name.
    pub(crate) fn named(&self, name: &str) -> Option<&'a str> {
        self.by_name
            .get(name)
            .and_then(|paths| paths.first().copied())
    }

    /// Shallowest file whose name starts with `prefix` (e.g. `vite.config.`).
    pub(crate) fn prefixed(&self, prefix: &str) -> Option<&'a str> {
        let mut found: Vec<&'a str> = self
            .by_name
            .iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .flat_map(|(_, paths)| paths.iter().copied())
            .collect();
        found.sort_by_key(|path| (paths::depth(path), *path));
        found.first().copied()
    }

    /// First file (by depth) with this name whose content contains `needle`.
    pub(crate) fn content_contains(&self, name: &str, needle: &str) -> Option<&'a str> {
        self.by_name.get(name)?.iter().copied().find(|path| {
            self.context
                .contents
                .get(*path)
                .is_some_and(|text| text.contains(needle))
        })
    }

    /// First dependency (in any ecosystem) whose name satisfies `matches`.
    pub(crate) fn dependency(&self, matches: impl Fn(&str) -> bool) -> Option<(&'a str, &'a str)> {
        self.context
            .dependencies
            .iter()
            .find(|(_, name)| matches(&name.to_ascii_lowercase()))
            .map(|(ecosystem, name)| (ecosystem.as_str(), name.as_str()))
    }

    /// First file for which `matches` holds.
    pub(crate) fn file(&self, matches: impl Fn(&ProjectFile<'a>) -> bool) -> Option<&'a str> {
        self.context
            .files
            .iter()
            .filter(|file| matches(file))
            .map(|file| file.path)
            .min_by_key(|path| (paths::depth(path), *path))
    }
}

fn tool(
    name: &str,
    ecosystem: Option<&str>,
    confidence: Confidence,
    evidence: Evidence,
) -> DetectedTool {
    DetectedTool {
        name: name.to_owned(),
        ecosystem: ecosystem.map(str::to_owned),
        confidence,
        evidence: vec![evidence],
    }
}

fn package_evidence(ecosystem: &str, name: &str) -> Evidence {
    Evidence::Package {
        ecosystem: ecosystem.to_owned(),
        name: name.to_owned(),
        version: None,
        manifest: None,
    }
}

/// Which JavaScript package manager the repository uses.
pub(crate) fn node_package_manager(
    index: &Index<'_>,
) -> Option<(&'static str, Confidence, Evidence)> {
    let manifest = index.named("package.json")?;
    if let Some(text) = index.context.contents.get(manifest)
        && let Ok(json) = serde_json::from_str::<serde_json::Value>(text)
        && let Some(declared) = json.get("packageManager").and_then(|v| v.as_str())
    {
        let name = declared.split('@').next().unwrap_or_default();
        let manager = match name {
            "pnpm" => "pnpm",
            "yarn" => "Yarn",
            "bun" => "Bun",
            _ => "npm",
        };
        return Some((
            manager,
            Confidence::High,
            Evidence::file(manifest).with_note(format!("packageManager: {declared}")),
        ));
    }
    for (lockfile, manager) in [
        ("pnpm-lock.yaml", "pnpm"),
        ("yarn.lock", "Yarn"),
        ("bun.lockb", "Bun"),
        ("bun.lock", "Bun"),
        ("package-lock.json", "npm"),
        ("npm-shrinkwrap.json", "npm"),
    ] {
        if let Some(path) = index.named(lockfile) {
            return Some((manager, Confidence::High, Evidence::file(path)));
        }
    }
    Some((
        "npm",
        Confidence::Medium,
        Evidence::file(manifest).with_note("no lockfile; npm assumed"),
    ))
}

/// Detects build systems.
pub fn build_systems(context: ToolContext<'_>) -> Vec<DetectedTool> {
    let index = Index::new(context);
    let mut tools = Vec::new();
    let high = Confidence::High;
    let mut by_file = |name: &str, ecosystem: Option<&str>, files: &[&str]| {
        if let Some(path) = files.iter().find_map(|file| index.named(file)) {
            tools.push(tool(name, ecosystem, high, Evidence::file(path)));
        }
    };
    by_file("Cargo", Some("cargo"), &["Cargo.toml"]);
    by_file("Go modules", Some("go"), &["go.mod"]);
    by_file("Maven", Some("maven"), &["pom.xml"]);
    by_file(
        "Gradle",
        Some("gradle"),
        &[
            "build.gradle.kts",
            "build.gradle",
            "settings.gradle.kts",
            "settings.gradle",
        ],
    );
    by_file("Composer", Some("composer"), &["composer.json"]);
    by_file("Bundler", Some("rubygems"), &["Gemfile"]);
    by_file("Rake", Some("rubygems"), &["Rakefile"]);
    by_file("Swift Package Manager", Some("swiftpm"), &["Package.swift"]);
    by_file("CMake", None, &["CMakeLists.txt"]);
    by_file("Meson", None, &["meson.build"]);
    by_file("Make", None, &["Makefile", "GNUmakefile", "makefile"]);
    by_file(
        "Bazel",
        None,
        &[
            "MODULE.bazel",
            "WORKSPACE",
            "WORKSPACE.bazel",
            "BUILD.bazel",
        ],
    );
    by_file("Autotools", None, &["configure.ac"]);
    by_file("SCons", None, &["SConstruct"]);
    by_file("just", None, &["justfile", "Justfile"]);
    by_file("Task", None, &["Taskfile.yml", "Taskfile.yaml"]);
    by_file("Deno", Some("deno"), &["deno.json", "deno.jsonc"]);
    by_file("Nix", None, &["flake.nix", "default.nix", "shell.nix"]);
    by_file("Turborepo", Some("npm"), &["turbo.json"]);
    by_file("Nx", Some("npm"), &["nx.json"]);
    by_file("Lerna", Some("npm"), &["lerna.json"]);
    by_file("Angular CLI", Some("npm"), &["angular.json"]);
    by_file("Tauri", None, &["tauri.conf.json"]);
    by_file("uv", Some("pypi"), &["uv.lock"]);
    by_file("Pipenv", Some("pypi"), &["Pipfile"]);

    if let Some((manager, confidence, evidence)) = node_package_manager(&index) {
        tools.push(tool(manager, Some("npm"), confidence, evidence));
    }
    for (prefix, name) in [
        ("vite.config.", "Vite"),
        ("webpack.config.", "webpack"),
        ("next.config.", "Next.js"),
        ("rollup.config.", "Rollup"),
        ("svelte.config.", "SvelteKit"),
        ("astro.config.", "Astro"),
        ("nuxt.config.", "Nuxt"),
    ] {
        if let Some(path) = index.prefixed(prefix) {
            tools.push(tool(name, Some("npm"), high, Evidence::file(path)));
        }
    }
    for (needle, name) in [
        ("[tool.poetry", "Poetry"),
        ("hatchling", "Hatch"),
        ("pdm-backend", "PDM"),
        ("flit_core", "Flit"),
        ("maturin", "Maturin"),
        ("setuptools", "setuptools"),
    ] {
        if let Some(path) = index.content_contains("pyproject.toml", needle) {
            tools.push(tool(name, Some("pypi"), high, Evidence::file(path)));
        }
    }
    if !tools.iter().any(|t| t.name == "setuptools")
        && let Some(path) = index.named("setup.py").or_else(|| index.named("setup.cfg"))
    {
        tools.push(tool("setuptools", Some("pypi"), high, Evidence::file(path)));
    }
    if let Some(path) = index.file(|file| {
        let name = paths::file_name(file.path);
        name.starts_with("requirements") && name.ends_with(".txt")
    }) {
        tools.push(tool(
            "pip",
            Some("pypi"),
            Confidence::Medium,
            Evidence::file(path),
        ));
    }
    if let Some(path) = index.file(|file| {
        matches!(
            paths::extension(file.path).as_deref(),
            Some("csproj" | "fsproj" | "vbproj" | "sln")
        )
    }) {
        tools.push(tool(".NET SDK", Some("nuget"), high, Evidence::file(path)));
    }
    if let Some(path) = index.named("pubspec.yaml") {
        let flutter = index.content_contains("pubspec.yaml", "flutter").is_some();
        let name = if flutter { "Flutter" } else { "Dart pub" };
        tools.push(tool(name, Some("pub"), high, Evidence::file(path)));
    }
    if let Some(path) = index.file(|file| file.path.contains(".xcodeproj/")) {
        let project = path.split(".xcodeproj/").next().unwrap_or(path);
        tools.push(tool(
            "Xcode",
            None,
            high,
            Evidence::directory(format!("{project}.xcodeproj")),
        ));
    }
    if let Some((ecosystem, name)) = index.dependency(|name| name == "electron") {
        tools.push(tool(
            "Electron",
            Some(ecosystem),
            high,
            package_evidence(ecosystem, name),
        ));
    }
    tools
}

/// Detects test frameworks.
pub fn test_frameworks(context: ToolContext<'_>) -> Vec<DetectedTool> {
    let index = Index::new(context);
    let mut tools = Vec::new();
    let test_file = |language: &'static str| {
        move |file: &ProjectFile<'_>| {
            file.category == FileCategory::Test && file.language == Some(language)
        }
    };

    if let Some(path) = index.file(test_file("rust")) {
        tools.push(tool(
            "Rust test harness",
            Some("cargo"),
            Confidence::High,
            Evidence::file(path),
        ));
    } else if let Some(path) = index.named("Cargo.toml") {
        tools.push(tool(
            "Rust test harness",
            Some("cargo"),
            Confidence::Low,
            Evidence::file(path).with_note("tests may be inline #[test] functions"),
        ));
    }
    if let Some(path) = index.file(|file| file.path.ends_with("_test.go")) {
        tools.push(tool(
            "Go testing",
            Some("go"),
            Confidence::High,
            Evidence::file(path),
        ));
    }

    // Frameworks recognized by a dependency or a configuration file.
    let frameworks: [(&str, &str, &[&str], &[&str]); 21] = [
        ("Jest", "npm", &["jest"], &["jest.config."]),
        (
            "Vitest",
            "npm",
            &["vitest"],
            &["vitest.config.", "vitest.workspace."],
        ),
        ("Mocha", "npm", &["mocha"], &[".mocharc"]),
        ("Jasmine", "npm", &["jasmine", "jasmine-core"], &[]),
        ("AVA", "npm", &["ava"], &[]),
        (
            "Playwright",
            "npm",
            &["@playwright/test"],
            &["playwright.config."],
        ),
        (
            "Cypress",
            "npm",
            &["cypress"],
            &["cypress.config.", "cypress.json"],
        ),
        ("Karma", "npm", &["karma"], &["karma.conf."]),
        (
            "Testing Library",
            "npm",
            &[
                "@testing-library/react",
                "@testing-library/vue",
                "@testing-library/dom",
                "@testing-library/svelte",
            ],
            &[],
        ),
        (
            "pytest",
            "pypi",
            &["pytest"],
            &["pytest.ini", "conftest.py"],
        ),
        ("Hypothesis", "pypi", &["hypothesis"], &[]),
        ("tox", "pypi", &["tox"], &["tox.ini"]),
        ("nox", "pypi", &["nox"], &["noxfile.py"]),
        (
            "RSpec",
            "rubygems",
            &["rspec", "rspec-rails", "rspec-core"],
            &[".rspec"],
        ),
        ("Minitest", "rubygems", &["minitest"], &[]),
        (
            "PHPUnit",
            "composer",
            &["phpunit/phpunit"],
            &["phpunit.xml", "phpunit.xml.dist"],
        ),
        ("Pest", "composer", &["pestphp/pest"], &[]),
        ("xUnit.net", "nuget", &["xunit"], &[]),
        ("NUnit", "nuget", &["nunit"], &[]),
        ("MSTest", "nuget", &["mstest.testframework"], &[]),
        ("Flutter test", "pub", &["flutter_test"], &[]),
    ];
    for (name, ecosystem, packages, configs) in frameworks {
        if let Some((found_ecosystem, package)) =
            index.dependency(|dependency| packages.contains(&dependency))
        {
            tools.push(tool(
                name,
                Some(ecosystem),
                Confidence::High,
                package_evidence(found_ecosystem, package),
            ));
        } else if let Some(path) = configs
            .iter()
            .find_map(|config| index.named(config).or_else(|| index.prefixed(config)))
        {
            tools.push(tool(
                name,
                Some(ecosystem),
                Confidence::High,
                Evidence::file(path),
            ));
        }
    }
    if !tools.iter().any(|t| t.name == "pytest")
        && let Some(path) = index
            .content_contains("pyproject.toml", "[tool.pytest")
            .or_else(|| index.content_contains("setup.cfg", "[tool:pytest]"))
    {
        tools.push(tool(
            "pytest",
            Some("pypi"),
            Confidence::High,
            Evidence::file(path),
        ));
    }
    if !tools.iter().any(|t| t.name == "pytest")
        && let Some(path) = index.file(test_file("python"))
    {
        tools.push(tool(
            "unittest",
            Some("pypi"),
            Confidence::Low,
            Evidence::file(path).with_note("Python tests without a detected pytest configuration"),
        ));
    }
    for (needle, name) in [
        ("junit", "JUnit"),
        ("testng", "TestNG"),
        ("kotest", "Kotest"),
        ("spock", "Spock"),
    ] {
        let jvm_dependency = context.dependencies.iter().find(|(ecosystem, package)| {
            matches!(ecosystem.as_str(), "maven" | "gradle")
                && package.to_ascii_lowercase().contains(needle)
        });
        if let Some((ecosystem, package)) = jvm_dependency {
            tools.push(tool(
                name,
                Some(ecosystem),
                Confidence::High,
                package_evidence(ecosystem, package),
            ));
        }
    }
    if let Some(path) =
        index.file(|file| file.language == Some("swift") && file.path.contains("Tests/"))
    {
        tools.push(tool(
            "XCTest",
            Some("swiftpm"),
            Confidence::Medium,
            Evidence::file(path),
        ));
    }
    for (needle, name) in [
        ("GTest", "GoogleTest"),
        ("gtest", "GoogleTest"),
        ("Catch2", "Catch2"),
        ("enable_testing", "CTest"),
    ] {
        if !tools.iter().any(|t| t.name == name)
            && let Some(path) = index.content_contains("CMakeLists.txt", needle)
        {
            tools.push(tool(name, None, Confidence::Medium, Evidence::file(path)));
        }
    }
    if let Some(path) =
        index.file(|file| file.language == Some("elixir") && file.path.ends_with("_test.exs"))
    {
        tools.push(tool(
            "ExUnit",
            Some("hex"),
            Confidence::High,
            Evidence::file(path),
        ));
    }
    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file<'a>(
        path: &'a str,
        category: FileCategory,
        language: Option<&'static str>,
    ) -> ProjectFile<'a> {
        ProjectFile {
            path,
            category,
            language,
            code_lines: 10,
            total_lines: 12,
        }
    }

    fn names(tools: &[DetectedTool]) -> Vec<&str> {
        tools.iter().map(|t| t.name.as_str()).collect()
    }

    #[test]
    fn detects_build_systems_from_files_and_content() {
        let files = [
            file("Cargo.toml", FileCategory::Manifest, Some("toml")),
            file("web/package.json", FileCategory::Manifest, Some("json")),
            file("web/pnpm-lock.yaml", FileCategory::Lockfile, Some("yaml")),
            file(
                "web/vite.config.ts",
                FileCategory::Configuration,
                Some("typescript"),
            ),
            file("py/pyproject.toml", FileCategory::Manifest, Some("toml")),
            file("Makefile", FileCategory::Build, Some("makefile")),
            file("App/App.csproj", FileCategory::Manifest, Some("xml")),
        ];
        let mut contents = BTreeMap::new();
        contents.insert(
            "py/pyproject.toml".to_owned(),
            "[build-system]\nrequires = [\"hatchling\"]\n".to_owned(),
        );
        let context = ToolContext {
            files: &files,
            contents: &contents,
            dependencies: &[],
        };
        assert_eq!(
            names(&build_systems(context)),
            vec!["Cargo", "Make", "pnpm", "Vite", "Hatch", ".NET SDK"]
        );
    }

    #[test]
    fn detects_test_frameworks_from_dependencies_configs_and_files() {
        let files = [
            file("src/lib.rs", FileCategory::Source, Some("rust")),
            file("tests/cli.rs", FileCategory::Test, Some("rust")),
            file("pkg/api_test.go", FileCategory::Test, Some("go")),
            file(
                "web/playwright.config.ts",
                FileCategory::Configuration,
                Some("typescript"),
            ),
            file("py/tests/test_api.py", FileCategory::Test, Some("python")),
            file("pom.xml", FileCategory::Manifest, Some("xml")),
        ];
        let contents = BTreeMap::new();
        let dependencies = vec![
            ("npm".to_owned(), "vitest".to_owned()),
            (
                "maven".to_owned(),
                "org.junit.jupiter:junit-jupiter".to_owned(),
            ),
        ];
        let context = ToolContext {
            files: &files,
            contents: &contents,
            dependencies: &dependencies,
        };
        let tools = test_frameworks(context);
        assert_eq!(
            names(&tools),
            vec![
                "Rust test harness",
                "Go testing",
                "Vitest",
                "Playwright",
                "unittest",
                "JUnit"
            ]
        );
        let unittest = tools.iter().find(|t| t.name == "unittest").unwrap();
        assert_eq!(unittest.confidence, Confidence::Low);
    }
}
