# Contributing to RepoDNA

Thank you for helping. Contributions of every size are welcome, from a typo fix to a new
analyzer. This guide takes you from a fresh clone to an open pull request with the exact
commands to run.

Everyone who participates agrees to the [Code of Conduct](CODE_OF_CONDUCT.md). Please
report security vulnerabilities privately, as described in [SECURITY.md](SECURITY.md),
never in a public issue.

## Ways to contribute

- **Report a bug** or a wrong result, with the output of `repodna version --json` and
  `repodna doctor --json`.
- **Teach RepoDNA a language**: lexical analysis for a language it only counts lines for, or
  a language it does not know yet (built in, or as a plugin).
- **Add a dependency ecosystem** or improve one.
- **Improve an analyzer**: fix a false positive, catch a missed signal, or add a finding.
- **Add a fixture** that captures a situation RepoDNA should handle.
- **Improve the web interface or the reports**: views, charts, accessibility.
- **Make it faster**, with a benchmark that shows it.
- **Write a plugin** or improve the documentation.

For anything larger than a small fix, please open an issue first, so that we can agree on
the approach before you spend time on it.

## 1. Clone

```sh
git clone https://github.com/sanskarIN/RepoDNA.git
cd RepoDNA
```

If you plan to open a pull request, fork the repository on GitHub and clone your fork
instead.

## 2. Install the prerequisites

- [Git](https://git-scm.com/)
- A current stable [Rust](https://www.rust-lang.org/tools/install) toolchain, installed with
  `rustup`, with the `rustfmt` and `clippy` components
- [Node.js](https://nodejs.org/) 20.19 or newer, with npm
- Only for the desktop app: Tauri's system libraries. On Debian and Ubuntu:
  `sudo apt-get install libwebkit2gtk-4.1-dev`. For other systems, see
  [Tauri's prerequisites](https://v2.tauri.app/start/prerequisites/).

## 3. Set up

```sh
cargo xtask setup
```

This checks the prerequisites, installs the web dependencies (`npm ci`), builds the web
interface and the `repodna` command line, and writes the fixture repositories to
`fixtures/generated/`. Run it again whenever you want a clean start.

## 4. Run the tests

```sh
cargo test --workspace        # Rust unit and integration tests
npm test                      # web interface and package tests
```

Both should pass on a fresh clone. If they do not, please
[open an issue](https://github.com/sanskarIN/RepoDNA/issues/new/choose).

## 5. Try it

```sh
cargo run -p repodna-cli -- serve
```

Open the printed link and choose **Try the demo**, or analyze a fixture or your own
repository:

```sh
cargo run -p repodna-cli -- analyze fixtures/generated/history
cargo run -p repodna-cli -- findings fixtures/generated/secrets
```

For work on the web interface, `npm run dev -w @repodna/web` serves it with live reloading
at `http://localhost:5173`; for the desktop app, run `npm run dev -w @repodna/desktop`.

## 6. Make your change

Create a branch, then change the code. [The development guide](docs/development.md) shows
how the repository is organized and where to make common changes: a language, an
ecosystem, a finding, a metric, a chart, or a report section.

Keep these principles in mind:

- **Evidence first.** Every conclusion needs evidence, a method, and its limitations. Do not
  add signals that cannot say why they fired.
- **Safe and private.** Analyzers never use the network or run code from the repository,
  and nothing sends telemetry.
- **Deterministic.** The same input gives the same output. Sort before you emit.
- **Tested with realistic samples.** Prove that the signal is found, and that its common
  false positives are not.
- **Accessible.** The interface works with a keyboard, never depends on color alone, and
  every chart has a table view.
- **Documented.** Update the pages in `docs/` that describe what you changed, and add a line
  to the "Unreleased" section of [CHANGELOG.md](CHANGELOG.md).

If you change the analysis model in `repodna-core`, regenerate the schemas and the
TypeScript types:

```sh
cargo run -p repodna-cli -- schema artifact > schemas/repodna-artifact.schema.json
cargo run -p repodna-cli -- schema config > schemas/repodna-config.schema.json
npm run generate -w @repodna/schema
```

## 7. Run the checks

These are the checks CI runs. Run them before you push:

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

`cargo fmt --all` and `npm run format` fix formatting. If you changed the desktop app, also
run:

```sh
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --features custom-protocol --locked -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --features custom-protocol --locked
```

## 8. Open a pull request

Push your branch and open a pull request against `main`. The pull request template asks
what changed, why, and how you tested it.

- Keep each pull request focused on one change; several small ones are easier to review
  than one large one.
- Write commit messages in the style of the history: a type and an optional scope, then a
  short summary in the imperative, such as `fix(report): name modules consistently` or
  `feat(parser): add lexical analysis for Zig`. Common types are `feat`, `fix`, `docs`,
  `test`, `perf`, `refactor`, `ci`, and `chore`.
- CI must pass. If a check fails and you are not sure why, say so in the pull request, and
  we will help.

A maintainer will review your pull request as soon as possible. Reviews are about the
change, never about the person.

## License

By contributing, you agree that your contributions are licensed under the
[Apache License 2.0](LICENSE), the license of the project.
