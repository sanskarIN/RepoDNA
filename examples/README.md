# Examples

| Path | What it shows |
|---|---|
| [`self-analysis/`](self-analysis) | RepoDNA's analysis of its own repository: the full report, Project DNA cards, and README badges |
| [`ci/repodna.yml`](ci/repodna.yml) | A GitHub Actions workflow that installs a RepoDNA release and runs `repodna ci` |
| [`configs/`](configs) | Configuration files: a repository's `repodna.toml`, a stricter CI configuration, and a user configuration |

More to try:

- The fixture repositories (`cargo xtask fixtures`, see [fixtures](../fixtures/README.md))
  show how RepoDNA handles polyglot services, monorepos, long histories, cycles,
  duplication, generated code, fake secrets, and more.
- The example plugins in [`plugins/`](../plugins) add a language and an analyzer.
- A comparison of two fixtures with different architectures:

  ```sh
  cargo xtask fixtures
  repodna compare fixtures/generated/monorepo fixtures/generated/cyclic
  ```
