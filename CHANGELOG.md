# Changelog

All notable changes to RepoDNA are documented in this file. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and RepoDNA follows
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [1.0.0] - 2026-09-26

The first stable release: local-first repository intelligence and code archaeology, with
every conclusion backed by evidence.

### Analysis

- One analysis of a directory, a Git URL (cloned into a temporary directory), or an archive
  (`.zip`, `.tar`, `.tar.gz`, `.tgz`), with three profiles: `quick`, `standard`, and `deep`.
- Structure and languages: 65 built-in languages, 25 of them with lexical analysis of
  imports, symbols, and complexity; file classification that honors `.gitignore`,
  `.gitattributes` (`linguist-generated`, `linguist-vendored`), and your own rules; entry
  points and the directory tree.
- Architecture: modules from package manifests and directories, import resolution, the
  file and module dependency graphs, cycles, layers, centrality, and the inferred style
  with a confidence level.
- Dependencies: manifests and lockfiles for Cargo, npm, PyPI, Go, Maven and Gradle, NuGet,
  RubyGems, Composer, pub, and Swift Package Manager, read offline.
- History and code archaeology: commits, contributors with `.mailmap`, ownership, releases,
  quiet periods, file history across renames, recent changes, and ranked change hotspots.
- The Codebase Time Machine: snapshots rebuilt from Git without a checkout, epochs, events,
  the architecture at each snapshot (deep profile), and the project's story, with facts
  and interpretations kept apart.
- Quality signals: cyclomatic complexity, long and deeply nested functions, large files,
  duplicated and similar code, markers such as TODO and FIXME, and dead-code candidates.
- Security signals: 20 secret rules (values are never stored), 18 risky-pattern rules for
  code, configuration, CI workflows, and containers, and file permission checks.
- Tests, build, and documentation: test frameworks and files, build systems and commands,
  CI providers, documentation checks, and a getting-started guide assembled from the
  repository. Build and test commands run only when execution is enabled in the user
  configuration.
- Findings with evidence, method, limitations, and next steps, a severity (critical,
  warning, attention, info) and a confidence (high, medium, low), and suppressions that
  require a reason.
- The DNA fingerprint: eight descriptive dimensions and a DNA hash for each analyzed
  snapshot.
- The RepositoryDNA artifact, a versioned JSON document (schema 1.0) with published JSON
  Schemas, and reproducible output with `--reproducible` and `SOURCE_DATE_EPOCH`.

### Command line

- `repodna` with `analyze` (alias `scan`), `report`, `architecture`, `dependencies`,
  `history`, `hotspots`, `timeline`, `findings`, `show`, `compare`, `card`, `badge`,
  `onboarding`, `explain`, `ci`, `export`, `import`, `list` (alias `ls`), `init`, `config`,
  `plugins`, `cache`, `clean`, `doctor`, `version`, `serve`, `schema`, and `completions`.
- `repodna ci` with `--fail-on`, baselines, `--new-only`, GitHub Actions annotations, and
  step summaries; documented exit codes.

### Reports and sharing

- Self-contained HTML reports in four themes, Markdown, JSON, and CSV tables, written one
  at a time or as a bundle.
- Project DNA cards (SVG and PNG, light and dark), README badges, onboarding guides, and
  side-by-side comparisons that describe differences without ranking.
- Portable `.repodna` exports and imports, and the privacy presets `local`, `share`, and
  `public`.

### Web interface and desktop app

- `repodna serve`: a local web interface on 127.0.0.1 with a session token, covering every
  part of the analysis, with search, a command palette, keyboard shortcuts, light and dark
  themes, a table view for every chart, and a bundled demo that works offline.
- A desktop app for Linux, macOS, and Windows, built with Tauri on the same Rust core.

### Extensibility

- Optional AI explanations built from the analysis evidence, through a local command, an
  OpenAI-compatible server, or the Anthropic API, off by default, with a dry run, explicit
  consent for remote endpoints, and recorded provenance.
- Plugins: declarative language definitions and analyzers in any language that exchange
  JSON with RepoDNA, enabled only by the user; two example plugins.
- Configuration in `repodna.toml` and a user configuration file, validated strictly; a
  repository's own configuration cannot enable plugins, AI, or command execution.

### Safety and privacy

- No telemetry and no network use except cloning a URL you give and an AI provider you
  configure.
- A hardened Git runner, restricted clone URLs, safe archive extraction with limits,
  linear-time regular expressions, and local storage in SQLite that can be checked,
  repaired, and cleaned.

[Unreleased]: https://github.com/sanskarIN/RepoDNA/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/sanskarIN/RepoDNA/releases/tag/v1.0.0
