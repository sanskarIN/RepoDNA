# Contributing

Contributions of every size are welcome: bug reports, documentation fixes, new languages,
new ecosystems, new analyzers, and ideas. The full guide, with exact commands, is
[CONTRIBUTING.md](../CONTRIBUTING.md). In short:

```sh
git clone https://github.com/sanskarIN/RepoDNA.git
cd RepoDNA
cargo xtask setup                  # check prerequisites, install, build, and write fixtures
cargo test --workspace             # run the Rust tests
npm test                           # run the web tests
cargo run -p repodna-cli -- serve  # try your change
```

## Good places to start

| Area | Where | Guide |
|---|---|---|
| A language RepoDNA does not know yet | `crates/repodna-parser/src/builtin.rs`, or a plugin | [Adding a language](development.md#adding-or-improving-a-language) |
| A dependency ecosystem | `crates/repodna-dependencies/src/providers/` | [Adding an ecosystem](development.md#adding-a-dependency-ecosystem) |
| A false positive or a missed signal | The analyzer crate, plus a fixture or test that shows it | [Adding an analyzer or a finding](development.md#adding-an-analyzer-or-a-finding) |
| A report section or a chart | `crates/repodna-report/`, `apps/web/src/` | [Adding a report section](development.md#adding-a-report-section) |
| An example plugin | `plugins/` | [Plugins](plugins.md) |
| Documentation | `docs/` | This site |

## Documentation contributions

A small guide fix is a useful contribution. When you add or revise a page:

1. Link it from [the documentation index](index.md) if readers should find
   it in the main navigation; use relative paths for other internal links.
2. Check commands, option names and examples against
   [the CLI reference](cli.md) or the corresponding implementation.
   Avoid presenting unreleased features as already available in a
   published stable version.
3. For changes to behavior, update the relevant
   [FAQ](faq.md), [troubleshooting](troubleshooting.md) and
   [getting-started guide](getting-started.md) where appropriate.
4. Build and validate the documentation site:

```sh
npm ci
node packaging/docs/build.mjs --out apps/web/dist/docs --check
```

The builder checks local file links and heading anchors. See
[the documentation-site development guide](development.md#the-documentation-site)
for local preview instructions.

## Principles for changes

- Every conclusion needs evidence, a method, and its limitations. Do not add signals that
  cannot say why they fired.
- No network access or code execution in analyzers; no telemetry anywhere.
- Keep results deterministic and add tests with realistic samples.
- Keep the user interface accessible: keyboard operable, readable without color, with a
  table view for every chart.

Please read the [Code of Conduct](../CODE_OF_CONDUCT.md) before participating, and report
security issues privately as described in [SECURITY.md](../SECURITY.md).
