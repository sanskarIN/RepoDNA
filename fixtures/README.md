# Fixture repositories

Small, deterministic repositories that each demonstrate a situation RepoDNA must handle. They
are generated from code in [`crates/repodna-testkit/src/fixtures.rs`](../crates/repodna-testkit/src/fixtures.rs),
so the repository never contains the files themselves: no generated noise in the history, and
no credential lookalikes in the source (the fake secrets are assembled when the fixture is
written). Authors are fictional and every commit date is fixed, so each build is identical.

```sh
cargo xtask fixtures          # writes every fixture to fixtures/generated/
repodna analyze fixtures/generated/history
```

`fixtures/generated/` is ignored by Git. `cargo xtask fixtures` only replaces that directory if
it created it. `--out DIR` writes somewhere else, and `--large-files N` and `--large-commits N`
change the size of the `large` fixture (default 5,000 files and 120 commits).

| Fixture | What it demonstrates |
|---|---|
| `tiny` | A tiny single-language project with a README, a license, and tests. |
| `polyglot` | A service in Go, TypeScript, Python, SQL, and shell with containers and CI. |
| `monorepo` | An npm workspace of packages that depend on each other. |
| `history` | Two years of history: contributors, releases, a rename, a restructuring, and a quiet period. |
| `no-git` | A Rust crate without Git history. |
| `duplicated` | Handlers that copy the same validation code. |
| `cyclic` | Modules and files that import each other in loops. |
| `generated` | Generated protocol code, a minified bundle, and linguist attributes next to handwritten code. |
| `undocumented` | Source code without a README, license, or comments. |
| `secrets` | Fake credentials in code and configuration, in test fixtures, and as placeholders. |
| `build-failure` | A project whose build command fails (seen only when execution is enabled). |
| `test-failure` | A project whose test suite fails (seen only when execution is enabled). |
| `unsupported` | Code in languages RepoDNA does not recognize without a plugin (Zig and COBOL). |
| `archived` | A project that stopped changing years ago, with deprecated tooling. |
| `large` | A generated repository with thousands of files across several languages. |

## How they are used

- [`crates/repodna-app/tests/fixtures.rs`](../crates/repodna-app/tests/fixtures.rs) builds each
  fixture in a temporary directory, analyzes it, and checks what RepoDNA finds: the languages
  of `polyglot`, the workspace packages of `monorepo`, the releases and quiet period of
  `history`, the cycle in `cyclic`, the generated files in `generated`, each kind of secret in
  `secrets` (and none from the placeholders), the failing build and tests when execution is
  enabled, and so on. Fixtures that need Git are skipped when Git is not installed.
- [`cargo xtask bench`](../benchmarks/README.md) times the analysis of every fixture.
- They are handy for trying RepoDNA's views and reports on something small and known.

The secrets are fake: they have the shape of real credentials so that the scanner finds them,
but they belong to no account.
