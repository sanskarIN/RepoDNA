# RepoDNA 1.0.0

Released on 2026-09-27: [GitHub release](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.0.0) · [source code at v1.0.0](https://github.com/sanskarIN/RepoDNA/tree/v1.0.0).

The first stable release: local-first repository intelligence and code archaeology, with
every conclusion backed by evidence.

## What it includes

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

## Downloads

The files are attached to the [GitHub release](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.0.0).

| File | What it is | Size |
|---|---|---|
| [`repodna-1.0.0-x86_64-unknown-linux-musl.tar.gz`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.0.0/repodna-1.0.0-x86_64-unknown-linux-musl.tar.gz) | Command line for Linux, x86_64 (static) | 10.2 MB |
| [`repodna-1.0.0-aarch64-unknown-linux-musl.tar.gz`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.0.0/repodna-1.0.0-aarch64-unknown-linux-musl.tar.gz) | Command line for Linux, ARM64 (static) | 9.4 MB |
| [`repodna-1.0.0-aarch64-apple-darwin.tar.gz`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.0.0/repodna-1.0.0-aarch64-apple-darwin.tar.gz) | Command line for macOS, Apple silicon | 9.2 MB |
| [`repodna-1.0.0-x86_64-apple-darwin.tar.gz`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.0.0/repodna-1.0.0-x86_64-apple-darwin.tar.gz) | Command line for macOS, Intel | 9.8 MB |
| [`repodna-1.0.0-x86_64-pc-windows-msvc.zip`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.0.0/repodna-1.0.0-x86_64-pc-windows-msvc.zip) | Command line for Windows, x86_64 | 9.8 MB |
| [`RepoDNA_1.0.0_amd64.deb`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.0.0/RepoDNA_1.0.0_amd64.deb) | Desktop app for Debian and Ubuntu | 11.9 MB |
| [`RepoDNA-1.0.0-1.x86_64.rpm`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.0.0/RepoDNA-1.0.0-1.x86_64.rpm) | Desktop app for Fedora and openSUSE | 11.9 MB |
| [`RepoDNA_1.0.0_universal.dmg`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.0.0/RepoDNA_1.0.0_universal.dmg) | Desktop app for macOS (Apple silicon and Intel) | 21.0 MB |
| [`RepoDNA_1.0.0_x64_en-US.msi`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.0.0/RepoDNA_1.0.0_x64_en-US.msi) | Desktop app for Windows (MSI installer) | 11.4 MB |
| [`RepoDNA_1.0.0_x64-setup.exe`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.0.0/RepoDNA_1.0.0_x64-setup.exe) | Desktop app for Windows (setup program) | 7.5 MB |
| [`SHA256SUMS.txt`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.0.0/SHA256SUMS.txt) | SHA-256 checksums of every file | 1.0 kB |

Also published for this version:

- `ghcr.io/sanskarin/repodna:1.0.0`: the command line with Git, for `linux/amd64` and `linux/arm64`.

## Install this version

```sh
# Linux, x86_64
curl -LO https://github.com/sanskarIN/RepoDNA/releases/download/v1.0.0/repodna-1.0.0-x86_64-unknown-linux-musl.tar.gz
tar -xzf repodna-1.0.0-x86_64-unknown-linux-musl.tar.gz
sudo install -m 0755 repodna-1.0.0-x86_64-unknown-linux-musl/repodna /usr/local/bin/repodna

# From source, with a Rust toolchain
cargo install --git https://github.com/sanskarIN/RepoDNA --tag v1.0.0 --locked repodna-cli

# In a container
docker run --rm -v "$PWD:/work" ghcr.io/sanskarin/repodna:1.0.0 analyze .
```

The [installation guide at v1.0.0](https://github.com/sanskarIN/RepoDNA/blob/v1.0.0/docs/installation.md) covers every
platform, checking the downloads against `SHA256SUMS.txt`, and opening the unsigned
binaries on macOS and Windows.

## Screenshots

<a href="../../images/v1.0.0/desktop/overview-light.png"><img src="../../images/v1.0.0/desktop/overview-light.png" alt="The overview in RepoDNA 1.0.0" width="400"></a> <a href="../../images/v1.0.0/desktop/architecture-dark.png"><img src="../../images/v1.0.0/desktop/architecture-dark.png" alt="The module map in RepoDNA 1.0.0" width="400"></a>

Twelve desktop views and the phone screens of this version, showing its own analysis of
this repository at `v1.0.0`, are in [`docs/images/v1.0.0`](../../images/README.md#100).

[All releases](../README.md)
