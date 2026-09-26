//! Path-based file classification.
//!
//! Classification is deterministic and explainable: every category comes from a documented
//! rule on the path (directory names, file names, extensions) plus the detected language.
//! Content-based adjustments (binary detection, generated-code headers) are applied after
//! the file is read; see [`crate::content`].
//!
//! Precedence, from strongest to weakest: user overrides, vendored code, build output, IDE
//! metadata, generated code, lockfiles, manifests, CI/CD, containers, infrastructure, build
//! files, license, tests, documentation, configuration, assets, data, binary, source, other.

use repodna_core::glob::GlobSet;
use repodna_core::model::languages::LanguageKind;
use repodna_core::model::structure::FileCategory;
use repodna_core::paths;

/// Result of classifying one path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Classification {
    /// Primary category.
    pub category: FileCategory,
    /// The file lives in a vendored third-party directory.
    pub vendored: bool,
    /// The file looks machine-generated.
    pub generated: bool,
}

/// User-configured classification overrides (glob patterns).
#[derive(Debug, Clone, Default)]
pub struct ClassificationOverrides {
    /// Paths to treat as generated.
    pub generated: GlobSet,
    /// Paths to treat as vendored.
    pub vendor: GlobSet,
    /// Paths to treat as tests.
    pub tests: GlobSet,
    /// Paths to treat as documentation.
    pub docs: GlobSet,
    /// Paths to treat as first-party source even if they look generated or vendored.
    pub source: GlobSet,
}

const VENDOR_DIRS: &[&str] = &[
    "vendor",
    "vendors",
    "third_party",
    "third-party",
    "thirdparty",
    "3rdparty",
    "node_modules",
    "bower_components",
    "Pods",
    "Carthage",
    "site-packages",
    "jspm_packages",
    ".bundle",
];

const BUILD_OUTPUT_DIRS: &[&str] = &[
    "target",
    "dist",
    "out",
    "obj",
    ".next",
    ".nuxt",
    ".svelte-kit",
    "__pycache__",
    "coverage",
    ".gradle",
    ".pytest_cache",
    ".mypy_cache",
    ".tox",
    "htmlcov",
    ".parcel-cache",
    ".turbo",
    ".angular",
    "DerivedData",
    ".dart_tool",
    ".output",
];

const IDE_DIRS: &[&str] = &[
    ".idea",
    ".vscode",
    ".vs",
    ".fleet",
    ".settings",
    ".eclipse",
    ".zed",
];
const IDE_FILES: &[&str] = &[".project", ".classpath", ".factorypath", "Session.vim"];
const IDE_EXTENSIONS: &[&str] = &[
    "iml",
    "suo",
    "user",
    "sublime-project",
    "sublime-workspace",
    "code-workspace",
];

const GENERATED_DIRS: &[&str] = &["generated", "__generated__", "gen-src", "autogen"];
const GENERATED_SUFFIXES: &[&str] = &[
    ".pb.go",
    ".pb.cc",
    ".pb.h",
    "_pb2.py",
    "_pb2_grpc.py",
    "_pb2.pyi",
    ".pb.ts",
    "_pb.js",
    "_pb.d.ts",
    ".g.dart",
    ".freezed.dart",
    ".gr.dart",
    ".config.dart",
    ".gen.go",
    "_generated.go",
    ".designer.cs",
    ".g.cs",
    ".g.i.cs",
    ".min.js",
    ".min.css",
    ".min.mjs",
    ".bundle.js",
    ".chunk.js",
    ".js.map",
    ".css.map",
    ".d.ts.map",
    ".generated.ts",
    ".generated.js",
    ".generated.rs",
    "_generated.rs",
    ".pyc",
];

const LOCKFILES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "npm-shrinkwrap.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "bun.lockb",
    "bun.lock",
    "deno.lock",
    "poetry.lock",
    "Pipfile.lock",
    "uv.lock",
    "pdm.lock",
    "composer.lock",
    "Gemfile.lock",
    "go.sum",
    "go.work.sum",
    "mix.lock",
    "pubspec.lock",
    "Podfile.lock",
    "packages.lock.json",
    "gradle.lockfile",
    "flake.lock",
    "Package.resolved",
    "conan.lock",
    "Cartfile.resolved",
    "shard.lock",
];

const MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "Pipfile",
    "requirements.txt",
    "requirements-dev.txt",
    "requirements_dev.txt",
    "dev-requirements.txt",
    "go.mod",
    "go.work",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
    "packages.config",
    "Directory.Packages.props",
    "composer.json",
    "Gemfile",
    "pubspec.yaml",
    "Package.swift",
    "Podfile",
    "Cartfile",
    "mix.exs",
    "deno.json",
    "deno.jsonc",
    "conanfile.txt",
    "conanfile.py",
    "vcpkg.json",
    "environment.yml",
    "environment.yaml",
    "pnpm-workspace.yaml",
    "lerna.json",
    "nx.json",
    "turbo.json",
    "rush.json",
    "shard.yml",
    "stack.yaml",
    "cabal.project",
    "dub.json",
    "Project.toml",
];
const MANIFEST_EXTENSIONS: &[&str] = &["csproj", "fsproj", "vbproj", "gemspec", "cabal", "nimble"];

const CI_DIRS: &[&str] = &[".circleci", ".buildkite", ".woodpecker"];
const CI_FILES: &[&str] = &[
    ".gitlab-ci.yml",
    "Jenkinsfile",
    "azure-pipelines.yml",
    ".travis.yml",
    "bitbucket-pipelines.yml",
    ".drone.yml",
    "appveyor.yml",
    ".appveyor.yml",
    "cloudbuild.yaml",
    "cloudbuild.yml",
    "codemagic.yaml",
    ".woodpecker.yml",
    "buildspec.yml",
    ".cirrus.yml",
    "wercker.yml",
    "netlify.toml",
    "vercel.json",
    "render.yaml",
    "fly.toml",
];

const CONTAINER_FILES: &[&str] = &[
    "Dockerfile",
    "Containerfile",
    "docker-compose.yml",
    "docker-compose.yaml",
    "compose.yml",
    "compose.yaml",
    ".dockerignore",
    "devcontainer.json",
];

const INFRA_DIRS: &[&str] = &[
    "terraform",
    "k8s",
    "kubernetes",
    "helm",
    "charts",
    "ansible",
    "pulumi",
    "manifests",
    "deploy",
    "deployment",
    "deployments",
    "infra",
    "infrastructure",
    "cloudformation",
];
const INFRA_FILES: &[&str] = &[
    "Chart.yaml",
    "kustomization.yaml",
    "kustomization.yml",
    "serverless.yml",
    "serverless.yaml",
    "cdk.json",
    "Pulumi.yaml",
    "skaffold.yaml",
    "Tiltfile",
    "app.yaml",
    "template.yaml",
];
const INFRA_EXTENSIONS: &[&str] = &["tf", "tfvars", "hcl", "bicep", "nomad"];

const BUILD_FILES: &[&str] = &[
    "Makefile",
    "makefile",
    "GNUmakefile",
    "CMakeLists.txt",
    "meson.build",
    "meson_options.txt",
    "BUILD",
    "BUILD.bazel",
    "WORKSPACE",
    "WORKSPACE.bazel",
    "MODULE.bazel",
    "SConstruct",
    "SConscript",
    "build.xml",
    "Rakefile",
    "justfile",
    "Justfile",
    "Taskfile.yml",
    "Taskfile.yaml",
    "gradlew",
    "gradlew.bat",
    "mvnw",
    "mvnw.cmd",
    "build.sbt",
    "Directory.Build.props",
    "Directory.Build.targets",
    "configure.ac",
    "Makefile.am",
    "build.zig",
    "Earthfile",
];
const BUILD_PREFIXES: &[&str] = &[
    "webpack.config.",
    "vite.config.",
    "vitest.config.",
    "rollup.config.",
    "gulpfile.",
    "Gruntfile.",
    "esbuild.config.",
    "tsup.config.",
    "babel.config.",
    "svelte.config.",
    "next.config.",
    "nuxt.config.",
    "astro.config.",
    "remix.config.",
    "metro.config.",
    "snowpack.config.",
    "craco.config.",
    "rspack.config.",
    "turbo.",
];
const BUILD_EXTENSIONS: &[&str] = &[
    "cmake", "bzl", "mk", "mak", "sln", "vcxproj", "pbxproj", "xcconfig",
];

const LICENSE_PREFIXES: &[&str] = &[
    "LICENSE",
    "LICENCE",
    "COPYING",
    "UNLICENSE",
    "NOTICE",
    "COPYRIGHT",
];

const TEST_DIRS: &[&str] = &[
    "test",
    "tests",
    "__tests__",
    "__test__",
    "spec",
    "specs",
    "testing",
    "e2e",
    "cypress",
    "playwright",
    "integration-tests",
    "integration_tests",
    "testdata",
    "test-data",
    "test_data",
    "fixtures",
    "__fixtures__",
    "__mocks__",
    "mocks",
    "benches",
    "benchmarks",
    "androidTest",
    "testFixtures",
    "unittests",
    "UnitTests",
    "Tests",
];

const DOC_DIRS: &[&str] = &[
    "docs",
    "doc",
    "documentation",
    "man",
    "wiki",
    "guides",
    "website",
    "site",
];
const DOC_PREFIXES: &[&str] = &[
    "README",
    "CHANGELOG",
    "CHANGES",
    "HISTORY",
    "NEWS",
    "CONTRIBUTING",
    "CODE_OF_CONDUCT",
    "SECURITY",
    "SUPPORT",
    "GOVERNANCE",
    "AUTHORS",
    "MAINTAINERS",
    "CONTRIBUTORS",
    "ROADMAP",
    "RELEASE",
    "UPGRADING",
    "MIGRATION",
    "FAQ",
    "ARCHITECTURE",
    "DESIGN",
    "CITATION",
];
const DOC_EXTENSIONS: &[&str] = &[
    "md", "markdown", "mdx", "rst", "adoc", "asciidoc", "txt", "pdf", "tex", "org", "pod",
];

const CONFIG_FILES: &[&str] = &[
    ".editorconfig",
    ".gitignore",
    ".gitattributes",
    ".gitmodules",
    ".mailmap",
    ".npmrc",
    ".nvmrc",
    ".node-version",
    ".python-version",
    ".ruby-version",
    ".tool-versions",
    ".yarnrc",
    ".yarnrc.yml",
    ".prettierrc",
    ".prettierignore",
    ".eslintignore",
    ".eslintrc",
    ".stylelintrc",
    ".babelrc",
    ".browserslistrc",
    ".env",
    ".env.example",
    ".flake8",
    ".pylintrc",
    ".rubocop.yml",
    ".golangci.yml",
    ".golangci.yaml",
    ".clang-format",
    ".clang-tidy",
    ".pre-commit-config.yaml",
    ".markdownlint.json",
    ".swiftlint.yml",
    "CODEOWNERS",
    "rustfmt.toml",
    ".rustfmt.toml",
    "clippy.toml",
    "deny.toml",
    "rust-toolchain",
    "rust-toolchain.toml",
    "tsconfig.json",
    "jsconfig.json",
    "tox.ini",
    "pytest.ini",
    "mypy.ini",
    "renovate.json",
    ".renovaterc",
    "biome.json",
    ".swcrc",
    "repodna.toml",
    ".repodna.toml",
    "codecov.yml",
    ".codecov.yml",
    "dependabot.yml",
    "Procfile",
    ".htaccess",
    "robots.txt",
    "tauri.conf.json",
];
const CONFIG_PREFIXES: &[&str] = &[
    ".eslintrc.",
    "eslint.config.",
    ".prettierrc.",
    "prettier.config.",
    "tsconfig.",
    ".stylelintrc.",
    "stylelint.config.",
    "jest.config.",
    "karma.conf.",
    "playwright.config.",
    "cypress.config.",
    "tailwind.config.",
    "postcss.config.",
    ".env.",
    "commitlint.config.",
    "lint-staged.config.",
    ".lintstagedrc",
    ".releaserc",
    "release.config.",
];
const CONFIG_DIRS: &[&str] = &["config", "configs", "conf", ".config", "settings"];

const ASSET_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "icns", "webp", "svg", "tif", "tiff", "avif",
    "heic", "ttf", "otf", "woff", "woff2", "eot", "mp3", "wav", "ogg", "flac", "m4a", "aac", "mp4",
    "webm", "mov", "avi", "mkv", "obj", "fbx", "glb", "gltf", "blend", "psd", "ai", "sketch",
    "fig", "xd", "cur", "ani", "lottie", "riv",
];
const DATA_EXTENSIONS: &[&str] = &[
    "csv",
    "tsv",
    "psv",
    "parquet",
    "sqlite",
    "sqlite3",
    "db",
    "npy",
    "npz",
    "h5",
    "hdf5",
    "pkl",
    "pickle",
    "feather",
    "arrow",
    "avro",
    "orc",
    "jsonl",
    "ndjson",
    "geojson",
    "xlsx",
    "xls",
    "ods",
    "dat",
    "tfrecord",
    "safetensors",
    "onnx",
    "pt",
    "ckpt",
];
const DATA_DIRS: &[&str] = &["data", "datasets", "dataset", "samples", "sample-data"];
const BINARY_EXTENSIONS: &[&str] = &[
    "exe", "dll", "so", "dylib", "a", "lib", "o", "class", "jar", "war", "ear", "pyc", "pyo",
    "wasm", "zip", "tar", "gz", "tgz", "bz2", "xz", "7z", "rar", "zst", "iso", "dmg", "msi", "deb",
    "rpm", "apk", "aab", "ipa", "bin", "img", "pdb", "nupkg", "whl", "gem", "crx",
];

fn has_dir(path: &str, names: &[&str]) -> bool {
    let parent = paths::parent(path);
    !parent.is_empty() && parent.split('/').any(|segment| names.contains(&segment))
}

fn name_has_prefix(name: &str, prefixes: &[&str]) -> bool {
    let upper = name.to_ascii_uppercase();
    prefixes.iter().any(|prefix| {
        upper == prefix.to_ascii_uppercase()
            || upper.starts_with(&format!("{}.", prefix.to_ascii_uppercase()))
            || upper.starts_with(&format!("{}-", prefix.to_ascii_uppercase()))
            || upper.starts_with(&format!("{}_", prefix.to_ascii_uppercase()))
    })
}

/// Returns `true` if the path matches a generated-code naming convention.
pub fn is_generated_path(path: &str) -> bool {
    let lower = paths::file_name(path).to_ascii_lowercase();
    has_dir(path, GENERATED_DIRS)
        || GENERATED_SUFFIXES
            .iter()
            .any(|suffix| lower.ends_with(suffix))
        || lower.starts_with("zz_generated")
}

/// Returns `true` if the path lives in a vendored third-party directory.
pub fn is_vendored_path(path: &str) -> bool {
    has_dir(path, VENDOR_DIRS)
}

/// Returns `true` if the path looks like a test file or lives in a test directory.
pub fn is_test_path(path: &str) -> bool {
    if has_dir(path, TEST_DIRS) {
        return true;
    }
    let name = paths::file_name(path);
    let lower = name.to_ascii_lowercase();
    let stem = paths::file_stem(path);
    lower.starts_with("test_")
        || lower == "conftest.py"
        || stem.ends_with("_test")
        || stem.ends_with("_tests")
        || stem.ends_with("_spec")
        || lower.contains(".test.")
        || lower.contains(".spec.")
        || lower.contains(".e2e.")
        || lower.contains(".cy.")
        || (stem.len() > 4
            && (stem.ends_with("Test") || stem.ends_with("Tests") || stem.ends_with("Spec")))
}

/// Classifies a repository path.
///
/// `language_kind` is the kind of the detected language, if any.
pub fn classify(
    path: &str,
    language_kind: Option<LanguageKind>,
    overrides: &ClassificationOverrides,
) -> Classification {
    let mut result = classify_default(path, language_kind);
    if overrides.vendor.matches(path) {
        result.vendored = true;
        result.category = FileCategory::Vendor;
    }
    if overrides.generated.matches(path) {
        result.generated = true;
        result.category = FileCategory::Generated;
    }
    if overrides.tests.matches(path) {
        result.category = FileCategory::Test;
    }
    if overrides.docs.matches(path) {
        result.category = FileCategory::Documentation;
    }
    if overrides.source.matches(path) {
        result.generated = false;
        result.vendored = false;
        result.category = if is_test_path(path) {
            FileCategory::Test
        } else {
            FileCategory::Source
        };
    }
    result
}

fn classify_default(path: &str, language_kind: Option<LanguageKind>) -> Classification {
    let name = paths::file_name(path);
    let extension = paths::extension(path).unwrap_or_default();
    let ext = extension.as_str();
    let vendored = is_vendored_path(path);
    let generated = is_generated_path(path);
    let category = |category| Classification {
        category,
        vendored,
        generated,
    };

    if vendored {
        return category(FileCategory::Vendor);
    }
    if has_dir(path, BUILD_OUTPUT_DIRS)
        || name.ends_with(".egg-info")
        || path.contains(".egg-info/")
    {
        return category(FileCategory::BuildOutput);
    }
    if has_dir(path, IDE_DIRS) || IDE_FILES.contains(&name) || IDE_EXTENSIONS.contains(&ext) {
        return category(FileCategory::IdeMetadata);
    }
    if generated {
        return category(FileCategory::Generated);
    }
    if LOCKFILES.contains(&name) {
        return category(FileCategory::Lockfile);
    }
    if MANIFESTS.contains(&name)
        || MANIFEST_EXTENSIONS.contains(&ext)
        || (name.starts_with("requirements") && ext == "txt")
    {
        return category(FileCategory::Manifest);
    }
    let in_workflows = path.starts_with(".github/workflows/")
        || path.starts_with(".gitea/workflows/")
        || path.starts_with(".forgejo/workflows/");
    if in_workflows || CI_FILES.contains(&name) || has_dir(path, CI_DIRS) {
        return category(FileCategory::CiCd);
    }
    if CONTAINER_FILES.contains(&name)
        || name.starts_with("Dockerfile.")
        || name.starts_with("docker-compose.")
        || ext == "dockerfile"
        || path.starts_with(".devcontainer/")
    {
        return category(FileCategory::Container);
    }
    if INFRA_EXTENSIONS.contains(&ext)
        || INFRA_FILES.contains(&name)
        || (has_dir(path, INFRA_DIRS) && matches!(ext, "yml" | "yaml" | "json" | "tpl" | "j2"))
    {
        return category(FileCategory::Infrastructure);
    }
    if BUILD_FILES.contains(&name)
        || BUILD_EXTENSIONS.contains(&ext)
        || BUILD_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
    {
        return category(FileCategory::Build);
    }
    // Conventional names such as LICENSE, HISTORY, or SECURITY mark documents, not program
    // source that happens to share the name (license.rs, History.tsx, security.py).
    let is_program = language_kind == Some(LanguageKind::Programming);
    if !is_program && paths::depth(path) <= 2 && name_has_prefix(name, LICENSE_PREFIXES) {
        return category(FileCategory::License);
    }
    let is_code = matches!(
        language_kind,
        Some(LanguageKind::Programming | LanguageKind::Markup | LanguageKind::Stylesheet)
    );
    if is_test_path(path) && !ASSET_EXTENSIONS.contains(&ext) && !BINARY_EXTENSIONS.contains(&ext) {
        return category(FileCategory::Test);
    }
    // SECURITY.md or HISTORY, but not security_concern.yml.
    let document_format =
        ext.is_empty() || DOC_EXTENSIONS.contains(&ext) || matches!(ext, "html" | "htm" | "cff");
    if (!is_program && document_format && name_has_prefix(name, DOC_PREFIXES))
        || (has_dir(path, DOC_DIRS)
            && (DOC_EXTENSIONS.contains(&ext) || language_kind == Some(LanguageKind::Prose)))
        || (DOC_EXTENSIONS.contains(&ext) && ext != "txt")
    {
        return category(FileCategory::Documentation);
    }
    if CONFIG_FILES.contains(&name)
        || CONFIG_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
        || matches!(ext, "ini" | "cfg" | "conf" | "properties" | "env")
        || (matches!(language_kind, Some(LanguageKind::Data))
            && matches!(
                ext,
                "yml" | "yaml" | "toml" | "json" | "jsonc" | "json5" | "xml" | "plist"
            )
            && (paths::depth(path) <= 2 || has_dir(path, CONFIG_DIRS)))
    {
        return category(FileCategory::Configuration);
    }
    if ASSET_EXTENSIONS.contains(&ext) {
        return category(FileCategory::Asset);
    }
    if DATA_EXTENSIONS.contains(&ext) || (has_dir(path, DATA_DIRS) && !is_code) {
        return category(FileCategory::Data);
    }
    if BINARY_EXTENSIONS.contains(&ext) {
        return category(FileCategory::Binary);
    }
    if is_code {
        return category(FileCategory::Source);
    }
    match language_kind {
        Some(LanguageKind::Data) => category(FileCategory::Data),
        Some(LanguageKind::Prose) => category(FileCategory::Documentation),
        Some(LanguageKind::Build) => category(FileCategory::Build),
        _ => category(FileCategory::Other),
    }
}

/// Returns `true` for paths that are examples or samples (useful for documentation signals
/// and for down-weighting secret candidates).
pub fn is_example_path(path: &str) -> bool {
    has_dir(
        path,
        &[
            "example",
            "examples",
            "sample",
            "samples",
            "demo",
            "demos",
            "tutorial",
            "tutorials",
            "playground",
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::languages::LanguageKind::{Data, Markup, Programming, Prose};

    fn category(path: &str, kind: Option<LanguageKind>) -> FileCategory {
        classify(path, kind, &ClassificationOverrides::default()).category
    }

    #[test]
    fn classifies_source_and_tests() {
        assert_eq!(
            category("src/main.rs", Some(Programming)),
            FileCategory::Source
        );
        assert_eq!(
            category("tests/cli.rs", Some(Programming)),
            FileCategory::Test
        );
        assert_eq!(
            category("pkg/server_test.go", Some(Programming)),
            FileCategory::Test
        );
        assert_eq!(
            category("app/test_models.py", Some(Programming)),
            FileCategory::Test
        );
        assert_eq!(
            category("web/Button.test.tsx", Some(Programming)),
            FileCategory::Test
        );
        assert_eq!(
            category("src/UserServiceTest.java", Some(Programming)),
            FileCategory::Test
        );
        assert_eq!(
            category("spec/models/user_spec.rb", Some(Programming)),
            FileCategory::Test
        );
        assert_eq!(
            category("src/latest.rs", Some(Programming)),
            FileCategory::Source
        );
        assert_eq!(
            category("src/contest.py", Some(Programming)),
            FileCategory::Source
        );
    }

    #[test]
    fn classifies_project_metadata() {
        assert_eq!(category("Cargo.toml", Some(Data)), FileCategory::Manifest);
        assert_eq!(category("Cargo.lock", Some(Data)), FileCategory::Lockfile);
        assert_eq!(
            category("web/package-lock.json", Some(Data)),
            FileCategory::Lockfile
        );
        assert_eq!(
            category("requirements-test.txt", Some(Prose)),
            FileCategory::Manifest
        );
        assert_eq!(
            category("src/App.csproj", Some(Data)),
            FileCategory::Manifest
        );
        assert_eq!(
            category(".github/workflows/ci.yml", Some(Data)),
            FileCategory::CiCd
        );
        assert_eq!(category(".gitlab-ci.yml", Some(Data)), FileCategory::CiCd);
        assert_eq!(category("Dockerfile", None), FileCategory::Container);
        assert_eq!(
            category("deploy/docker-compose.prod.yml", Some(Data)),
            FileCategory::Container
        );
        assert_eq!(
            category("infra/main.tf", Some(Data)),
            FileCategory::Infrastructure
        );
        assert_eq!(
            category("k8s/deployment.yaml", Some(Data)),
            FileCategory::Infrastructure
        );
        assert_eq!(category("CMakeLists.txt", None), FileCategory::Build);
        assert_eq!(
            category("vite.config.ts", Some(Programming)),
            FileCategory::Build
        );
        assert_eq!(category("LICENSE", None), FileCategory::License);
        assert_eq!(category("LICENSE-MIT", None), FileCategory::License);
    }

    #[test]
    fn classifies_documentation_and_configuration() {
        assert_eq!(
            category("README.md", Some(Prose)),
            FileCategory::Documentation
        );
        assert_eq!(
            category("docs/guide/intro.md", Some(Prose)),
            FileCategory::Documentation
        );
        assert_eq!(category("CONTRIBUTING", None), FileCategory::Documentation);
        assert_eq!(
            category("HISTORY.rst", Some(Prose)),
            FileCategory::Documentation
        );
        assert_eq!(
            category("SECURITY.md", Some(Prose)),
            FileCategory::Documentation
        );
        // Source files named like conventional documents are still source.
        assert_eq!(
            category("src/views/History.tsx", Some(Programming)),
            FileCategory::Source
        );
        assert_eq!(
            category("crates/core/src/security.rs", Some(Programming)),
            FileCategory::Source
        );
        assert_eq!(
            category("db/migration_001.sql", Some(Programming)),
            FileCategory::Source
        );
        assert_eq!(
            category("src/license.rs", Some(Programming)),
            FileCategory::Source
        );
        assert_ne!(
            category(".github/ISSUE_TEMPLATE/security_concern.yml", Some(Data)),
            FileCategory::Documentation
        );
        assert_eq!(
            category("CITATION.cff", Some(Data)),
            FileCategory::Documentation
        );
        assert_eq!(category(".editorconfig", None), FileCategory::Configuration);
        assert_eq!(
            category("tsconfig.base.json", Some(Data)),
            FileCategory::Configuration
        );
        assert_eq!(
            category("config/settings.yaml", Some(Data)),
            FileCategory::Configuration
        );
        assert_eq!(
            category("deep/nested/path/values.yaml", Some(Data)),
            FileCategory::Data
        );
    }

    #[test]
    fn classifies_vendored_generated_and_output() {
        let vendored = classify(
            "vendor/lib/x.go",
            Some(Programming),
            &ClassificationOverrides::default(),
        );
        assert_eq!(vendored.category, FileCategory::Vendor);
        assert!(vendored.vendored);
        let generated = classify(
            "api/service.pb.go",
            Some(Programming),
            &ClassificationOverrides::default(),
        );
        assert_eq!(generated.category, FileCategory::Generated);
        assert!(generated.generated);
        assert_eq!(
            category("assets/app.min.js", Some(Programming)),
            FileCategory::Generated
        );
        assert_eq!(
            category("target/debug/build.log", None),
            FileCategory::BuildOutput
        );
        assert_eq!(
            category("dist/index.html", Some(Markup)),
            FileCategory::BuildOutput
        );
        assert_eq!(
            category(".idea/workspace.xml", Some(Data)),
            FileCategory::IdeMetadata
        );
        assert_eq!(category("app.iml", None), FileCategory::IdeMetadata);
    }

    #[test]
    fn classifies_assets_data_and_binaries() {
        assert_eq!(category("assets/logo.png", None), FileCategory::Asset);
        assert_eq!(category("icons/app.svg", Some(Data)), FileCategory::Asset);
        assert_eq!(category("data/sales.csv", Some(Data)), FileCategory::Data);
        assert_eq!(category("bin/tool.exe", None), FileCategory::Binary);
        assert_eq!(
            category("tests/fixtures/logo.png", None),
            FileCategory::Asset
        );
        assert_eq!(category("notes.unknown", None), FileCategory::Other);
    }

    #[test]
    fn overrides_take_precedence() {
        let overrides = ClassificationOverrides {
            generated: GlobSet::new(&["src/schema/**"]).unwrap(),
            source: GlobSet::new(&["vendor/ours/**"]).unwrap(),
            ..ClassificationOverrides::default()
        };
        let generated = classify("src/schema/types.rs", Some(Programming), &overrides);
        assert_eq!(generated.category, FileCategory::Generated);
        let ours = classify("vendor/ours/lib.rs", Some(Programming), &overrides);
        assert_eq!(ours.category, FileCategory::Source);
        assert!(!ours.vendored);
    }

    #[test]
    fn detects_example_paths() {
        assert!(is_example_path("examples/basic/main.rs"));
        assert!(!is_example_path("src/example.rs"));
    }
}
