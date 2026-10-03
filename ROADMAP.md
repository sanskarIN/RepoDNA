# Roadmap

What we plan to work on, grouped by how soon. It is a statement of direction, not a set of
promises or dates: priorities change with what users need and who contributes. Ideas and
help are welcome in [issues](https://github.com/sanskarIN/RepoDNA/issues/new/choose).

## Now

Making 1.0 solid.

- Fix bugs, false positives, and missed signals reported by users, each with a fixture or
  test that keeps it fixed.
- Keep the documentation accurate and complete as the code changes.
- Measure and improve performance on large repositories and long histories, and publish the
  results with the [benchmarks](benchmarks/README.md).

## Next

- **Lexical analysis for more languages.** Imports, symbols, and complexity for languages
  that RepoDNA only counts lines for today, starting with Haskell, Erlang, Clojure, Julia,
  F#, and OCaml.
- **Deeper import resolution** for more languages and build setups, so that the dependency
  graph misses fewer edges.
- **Pull request analysis.** Compare a change with its base branch in CI and summarize what
  it changes in structure, dependencies, and findings, building on
  `repodna ci --baseline`.
- **An official GitHub Action** that installs RepoDNA and runs `repodna ci`, replacing the
  [example workflow](examples/ci/repodna.yml).
- **Easier installation** through package managers such as Homebrew, Scoop, and crates.io.

## Later

- **Editor integrations** for VS Code and JetBrains IDEs that show findings, hotspots, and
  module information next to the code, read from the RepositoryDNA artifact.
- **Syntax-tree analysis** for languages where lexical analysis is not precise enough,
  offered as an additional depth next to the lexical analyzers.
- **Several repositories at once:** a map of an organization's repositories and the
  dependencies between them.
- **Translations** of the web interface and the reports.

## Exploration

Ideas we find interesting but have not committed to:

- Replaying a repository's history as an animation of the Time Machine
- A repository lineage graph and a similarity explorer across projects
- An opt-in, self-hostable way to publish reports for a team
- An educational mode that walks through a well-known codebase
- New visualizations of the codebase, such as a three-dimensional map

## Not planned

- **Telemetry** of any kind.
- **Scores or rankings of people.** Contributor data describes history, never anyone's
  value.
- **Uploading code** to a service as a requirement for any feature. The local analysis
  stays complete and free.
