# Development

How the repository is organized, how to build and test it, and where to make common
changes. For the contribution process, see [CONTRIBUTING.md](../CONTRIBUTING.md).

## Setup

Prerequisites: Git, a current stable Rust toolchain (`rustup`), and Node.js 20.19 or newer
with npm. For the desktop app, also Tauri's system libraries (on Debian and Ubuntu,
`sudo apt-get install libwebkit2gtk-4.1-dev`).

```sh
git clone https://github.com/sanskarIN/RepoDNA.git
cd RepoDNA
npm ci                              # web, schema, visualization, and desktop tooling
cargo build                         # the Rust workspace
cargo xtask fixtures                # fixture repositories in fixtures/generated/
```

Run it:

```sh
cargo run -p repodna-cli -- analyze fixtures/generated/history
cargo run -p repodna-cli -- serve
```

## Repository layout

```text
crates/
  repodna-core          domain model, configuration, evidence, findings, artifact I/O, schemas
  repodna-discovery     file walking, classification, safe reading, archive extraction
  repodna-parser        language registry and lexical analyzers
  repodna-git           hardened Git runner, history, refs, trees, URL safety
  repodna-dependencies  manifests and lockfiles per ecosystem
  repodna-architecture  modules, import resolution, graphs, cycles, layers
  repodna-quality       complexity, duplication, similarity, markers, dead code, hotspots
  repodna-security      secrets, risky patterns, permissions
  repodna-project       tests, build systems, commands, CI, documentation
  repodna-evolution     epochs, events, snapshots, story, module ages
  repodna-engine        the pipeline: input preparation, stages, insights, fingerprint
  repodna-store         SQLite index, stored artifacts, per-file cache
  repodna-report        Markdown, HTML, cards, badges, CSV, CI summaries, comparisons
  repodna-ai            optional AI explanations
  repodna-plugin        plugin discovery, manifests, and the analyzer protocol
  repodna-app           services shared by the CLI, the server, and the desktop app
  repodna-server        the local server behind `repodna serve`
  repodna-cli           the `repodna` binary
  repodna-testkit       deterministic Git repositories and fixtures for tests
apps/
  web                   React + TypeScript web interface (Vite)
  desktop               Tauri desktop app (its own Cargo workspace in src-tauri/)
packages/
  schema                TypeScript types generated from the artifact schema
  visualization         layout, color, and formatting helpers for charts
plugins/                example plugins
schemas/                published JSON Schemas
fixtures/               fixture documentation (repositories are generated)
benchmarks/             benchmark results and method
xtask/                  development tasks: fixtures and benchmarks
docs/                   this documentation
```

See [architecture](architecture.md) for how the pieces fit together.

## Checks

These are the checks CI runs. Run them before opening a pull request:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

npm run format:check
npm run check-generated -w @repodna/schema
npm run typecheck
npm test
npm run build -w @repodna/web
```

The desktop app is checked separately (it needs the web build first):

```sh
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --features custom-protocol --locked -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --features custom-protocol --locked
```

Dependencies are checked with [cargo-deny](https://github.com/EmbarkStudios/cargo-deny):
`cargo deny check` and `cargo deny --manifest-path apps/desktop/src-tauri/Cargo.toml check`.

`npm run format` formats the TypeScript code; `cargo fmt --all` formats the Rust code.

## Tests

- **Unit tests** live next to the code in each crate (`#[cfg(test)]` modules) and in
  `*.test.ts(x)` files for the web packages.
- **Integration tests**: `crates/repodna-app/tests/fixtures.rs` analyzes every
  [fixture repository](../fixtures/README.md) and checks what RepoDNA finds;
  `crates/repodna-cli/tests/cli.rs` runs the binary; `crates/repodna-server/tests/server.rs`
  exercises the API and its security checks.
- **Web view tests** render every view against the bundled demo artifact
  (`apps/web/public/demo/repodna.json`).
- `repodna-testkit` builds Git repositories with fixed authors and dates, so commit hashes
  and results are deterministic. Tests that need Git skip themselves when it is missing.

## The web interface

```sh
npm run dev -w @repodna/web          # http://localhost:5173
```

In development, API requests are proxied to a running `repodna serve` on port 7878. Start
the server in another terminal and open `http://localhost:5173/?token=TOKEN` with the token
it printed. Without a server, the interface still opens the demo and analysis files.

`packages/schema/src/generated.ts` is generated from `schemas/repodna-artifact.schema.json`;
after changing the Rust model, regenerate both:

```sh
cargo run -p repodna-cli -- schema artifact > schemas/repodna-artifact.schema.json
cargo run -p repodna-cli -- schema config > schemas/repodna-config.schema.json
npm run generate -w @repodna/schema
```

A test fails when the published schemas are out of date.

## The desktop app

```sh
npm run dev -w @repodna/desktop       # the app with live reloading
npm run bundle -w @repodna/desktop    # installers in apps/desktop/src-tauri/target/release/bundle/
```

See [apps/desktop/README.md](../apps/desktop/README.md).

## Common changes

### Adding or improving a language

Built-in languages are data in `crates/repodna-parser/src/builtin.rs`: file names, comment
and string syntax, import and symbol patterns, body style, and complexity keywords. Add a
`LanguageSpec` there, then a test with a small sample in the same file or in
`symbols.rs`/`imports.rs`. The test `every_builtin_pattern_compiles_and_ids_are_unique`
compiles every pattern. If imports should resolve to files, extend
`crates/repodna-architecture/src/resolve.rs`. Languages can also ship as
[plugins](plugins.md#language-definitions) without changing RepoDNA.

### Adding a dependency ecosystem

Implement `EcosystemProvider` in `crates/repodna-dependencies/src/providers/`, register it
in `builtin_providers`, and add tests with real-world manifest and lockfile samples.
Providers only read file contents; they must never run a package manager or use the
network.

### Adding an analyzer or a finding

1. Put the analysis in the crate it belongs to (or a new crate), producing types from
   `repodna-core`'s model. Add fields to the model with `#[serde(default)]` so older
   artifacts stay readable.
2. Wire it into the pipeline in `crates/repodna-engine/src/pipeline.rs`. A new stage also
   needs an entry in `Stage` and the profiles in `crates/repodna-core/src/config/profile.rs`.
3. Findings use `Finding::new(rule, subject, category, severity, confidence, title)` with a
   summary, rationale, method, evidence, limitations, and next steps. Rule identifiers are
   `area.kebab-case-name`; document new rules in
   [the rules reference](repository-dna.md#rules).
4. Regenerate the schemas, show the result in a report section and a web view, and add a
   fixture or test that proves the signal is found and that its false positives are not.

### Adding a metric

Metrics are defined in `crates/repodna-engine/src/fingerprint.rs` with an identifier,
label, value, unit, definition, method, and confidence. They appear in the metrics appendix,
the CSV `metrics` table, and the web interface.

### Adding a visualization

Charts in the web interface are SVG components in `apps/web/src/charts`, built on the
helpers in `packages/visualization` (layered graph layout, treemap, heatmap, scales, color
palettes, number formatting). Every chart needs a table view and must not rely on color
alone. Charts in HTML reports are rendered in Rust in `crates/repodna-report/src/charts.rs`.

### Adding a report section

Sections are listed in `crates/repodna-report/src/sections.rs`, and their content is built
in `crates/repodna-report/src/content/` as format-independent blocks, which the Markdown,
HTML, and terminal renderers turn into output.

### Adding an AI provider

Implement the `AiProvider` trait in `crates/repodna-ai/src/provider/`, add the provider
kind to `AiProviderKind` in `crates/repodna-core/src/config/mod.rs`, and build it in
`build_provider`. Report `remote()` honestly: remote providers need the user's consent.

### Database migrations

The storage schema is a list of migrations in `crates/repodna-store/src/schema.rs`; the
schema version is the number of migrations. Append a migration, never edit an existing
one. Migrations run in a transaction, so a failed upgrade leaves the previous data intact.

## Fixtures and benchmarks

```sh
cargo xtask fixtures                    # write every fixture to fixtures/generated/
cargo build --release -p repodna-cli
cargo xtask bench --runs 5              # the table in benchmarks/README.md
```

## Releasing

1. Update the version everywhere it appears: `Cargo.toml` (`[workspace.package]` and the
   internal dependencies), `apps/desktop/src-tauri/Cargo.toml`,
   `apps/desktop/src-tauri/tauri.conf.json`, and the `package.json` files (root,
   `apps/web`, `apps/desktop`, `packages/schema`, `packages/visualization`). Run
   `cargo build` and `npm install` to update the lockfiles.
2. Add a section for the version to `CHANGELOG.md`.
3. Regenerate the schemas and the bundled demo analysis if the model changed.
4. Commit, then create and push an annotated tag: `git tag -a v1.1.0 -m "RepoDNA 1.1.0"` and
   `git push origin v1.1.0`.

The [release workflow](../.github/workflows/release.yml) checks that the tag matches the
workspace version and the changelog, builds the command line for Linux, macOS, and Windows
and the desktop installers, and publishes them with checksums.
