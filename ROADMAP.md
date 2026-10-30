# Roadmap

What we plan to work on, grouped by how soon. It is a statement of direction, not a set of
promises or dates: priorities change with what users need and who contributes. Ideas and
help are welcome in [issues](https://github.com/sanskarIN/RepoDNA/issues/new/choose).

## Now

Making 1.x solid and easy to reach. Since 1.3.1, the web version analyzes repositories in
the browser, and an official GitHub Action runs RepoDNA in workflows.

- Fix bugs, false positives, and missed signals reported by users, each with a fixture or
  test that keeps it fixed.
- Keep the documentation accurate and complete as the code changes.
- Measure and improve performance on large repositories and long histories, and publish the
  results with the [benchmarks](benchmarks/README.md).

## After v1.4.5 — v1.5.0 development

RepoDNA v1.4.5 is the current published stable release. The next milestone is
**v1.5.0 development**, not an immediate version change or a release promise.
Work should be reviewed in separately tested PRs with compatibility, privacy,
offline behavior, and performance checks.

The first incremental CI impact improvement is a typed machine-readable
baseline delta: introduced and resolved finding details, stable IDs and
repository-relative changed paths, while retaining existing summary fields.
It is only one part of a future review-ready pull-request impact experience.
Follow-on work should cover affected modules and dependency edges, evidence
links, an optional share-safe output, and real-world validation before claiming
a complete impact analysis or releasing v1.5.0.

## Candidate feature milestones

These are prioritized candidates, not features claimed to be complete or a promised
release schedule. Ship each behind tests and independent PR review:

1. **Review-ready findings exports.** Keep the interactive CSV workflow and add a
   filtered JSON export retaining structured finding evidence; then explore an
   optional privacy-preview step before sharing sensitive paths or history.
2. **Pull request impact summaries.** Build on `repodna ci --baseline` to show
   introduced and resolved findings plus affected modules and dependency edges,
   with a stable machine-readable output for CI comments.
3. **Incremental local analysis.** Cache stages by revision, profile, and relevant
   configuration hash; invalidate safely when ignore patterns or analyzer versions
   change. Measure correctness against cold full analyses.
4. **Large-repository performance.** Add deterministic memory budgets and progress
   reporting for browser imports, then benchmark monorepos and histories with
   repeatable fixtures.
5. **Navigable onboarding views.** Let readers jump from a finding or module to
   the exact evidence and export a filtered, privacy-reviewed onboarding brief
   without uploading code.

## Next

- **Lexical analysis for more languages.** Imports, symbols, and complexity for languages
  that RepoDNA only counts lines for today, starting with Haskell, Erlang, Clojure, Julia,
  F#, and OCaml.
- **Deeper import resolution** for more languages and build setups, so that the dependency
  graph misses fewer edges.
- **Pull request analysis.** Compare a change with its base branch in CI and summarize what
  it changes in structure, dependencies, and findings, building on
  `repodna ci --baseline`.
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
