# Changelog

All notable changes to RepoDNA are documented in this file. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and RepoDNA follows
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [1.1.0] - 2026-10-04

Optional AI explanations: prose about a repository, grounded in the analysis, checked
against its evidence, and off until you configure a provider.

### Explanations

- `repodna explain` explains a repository (`--about repository`, `architecture`,
  `history`, `dependencies`, or `onboarding`), one module (`--module`), one hotspot
  (`--hotspot`), or answers a question (`--ask`), as text, Markdown, or JSON with its
  provenance: provider, model, revision, cited evidence, and token use.
- The model receives a numbered selection of evidence from the analysis, never the
  repository, and its answer is checked against it: citations of evidence that was not
  sent are removed, statements without a valid citation are labeled as not supported, and
  statements the model marks as inferences are labeled as such.
- Three providers: a local program that reads the prompt on standard input (for example
  `ollama run <model>`), any OpenAI-compatible server (Ollama, llama.cpp, LM Studio, vLLM,
  or a hosted service), and the Anthropic API.
- `--dry-run` shows the exact prompt, the evidence, where it would be sent, and the
  highest cost from prices you set, without contacting anything.
- Answers are cached on this machine; `--fresh` asks again and `repodna cache clear`
  removes them. When a model declines, RepoDNA reports the reason the provider gives.
- `repodna doctor` reports the configured provider without contacting it.

### Privacy and safety

- AI is set only in your user configuration: a repository's own configuration cannot
  enable it or allow remote providers.
- A provider whose endpoint is not on this machine is refused unless you allow remote AI
  with `privacy.remote_ai = true` or, for one run, `--allow-remote-ai`.
- API keys are read from an environment variable you name and never stored. Without one,
  `ANTHROPIC_API_KEY` is sent only to Anthropic's own endpoint.
- No source code is sent unless you enable `ai.include_source_excerpts`: excerpts are a
  few lines per file, have likely secrets redacted, and are never read through a link
  that leads out of the checkout. Contributor names and email addresses are never sent.
- The model gets no tools: it cannot run commands, read files, or use the network through
  RepoDNA. Repository text is sent as data, and answers are cleaned of terminal control
  sequences and escaped in Markdown.
- RepoDNA uses the network only to clone a Git URL you give and to reach an AI provider
  you configure.

### Web interface and desktop app

- The start page, the settings, and the analysis details say that explanations are
  optional and need a provider you configure, and whether an analysis used remote AI.

## [1.0.0] - 2026-09-27

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
  `onboarding`, `ci`, `export`, `import`, `list` (alias `ls`), `init`, `config`, `plugins`,
  `cache`, `clean`, `doctor`, `version`, `serve`, `schema`, and `completions`.
- `repodna ci` with `--fail-on`, baselines, `--new-only`, GitHub Actions annotations, and
  step summaries; documented exit codes.
- `repodna doctor` checks the installation, and `--export` writes a diagnostics bundle for
  bug reports that contains no source code.

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
- A container image with the command line and Git, `ghcr.io/sanskarin/repodna`, for
  `linux/amd64` and `linux/arm64`.
- About & support, Privacy Policy, Terms of Use, and Licenses pages in the web interface
  and the desktop app; every download includes the licenses of the third-party software it
  contains (`THIRD-PARTY-NOTICES.txt`).

### Extensibility

- Plugins: declarative language definitions and analyzers in any language that exchange
  JSON with RepoDNA, enabled only by the user; two example plugins.
- Configuration in `repodna.toml` and a user configuration file, validated strictly; a
  repository's own configuration cannot enable plugins or command execution.

### Safety and privacy

- No telemetry, no AI, and no network use except cloning a Git URL you give.
- A hardened Git runner, restricted clone URLs, safe archive extraction with limits,
  linear-time regular expressions, and local storage in SQLite that can be checked,
  repaired, and cleaned.

[Unreleased]: https://github.com/sanskarIN/RepoDNA/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/sanskarIN/RepoDNA/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/sanskarIN/RepoDNA/releases/tag/v1.0.0
