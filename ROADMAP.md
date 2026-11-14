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

## After v1.5.0 — v1.5.1 maintenance

RepoDNA v1.5.0 was published November 6, 2026. A focused **v1.5.1 patch**
is being qualified from that exact stable tag. It addresses saved-analysis
recovery/deletion correctness, malformed artifact validation and Git remote
security without adding the newer features already on `main`.

The `main` branch continues broader post-v1.5.0 feature development
independently. No future minor-version scope is promised by this patch. See
the [v1.5.1 qualification record](packaging/release/v1.5.1-checklist.md).
The release remains unpublished until full trial and explicit maintainer approval.

## Candidate feature milestones

These are prioritized candidates, not features claimed to be complete or a promised
release schedule. Ship each behind tests and independent PR review:

1. **Review-ready findings exports.** Filtered CSV, JSON and Markdown exports
   and a local review-before-save step are implemented on `main`. Next: a
   clearly defined privacy or redaction policy that covers the full export,
   not just the bounded preview.
2. **Pull request impact summaries.** Typed finding deltas, changed paths,
   one-hop architecture impact and per-file module ownership exist on `main`.
   Next: actionable evidence references, real-world evaluation, and explicit
   rename/unknown semantics before claiming comprehensive impact analysis.
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
- **Deeper pull request analysis.** Expand the existing `repodna ci --baseline`
  workflow with validated evidence links, explicit uncertainty and realistic
  change comparisons; one-hop graph neighbors are not causal predictions.
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
