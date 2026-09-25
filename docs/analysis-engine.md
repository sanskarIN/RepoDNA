# The analysis engine

The engine turns a repository into a [RepositoryDNA artifact](repository-dna.md). It is a
pipeline of stages; each stage reads the repository (or the results of earlier stages)
and adds a section to the artifact. Nothing in the repository is modified, and nothing is
executed unless you enable command execution.

## Inputs

| Input | How it is prepared |
|---|---|
| A local directory | Analyzed in place. If it is a Git working tree, its history is read with `git`. |
| A Git URL | Cloned into a temporary directory (see [safe cloning](security.md#how-repodna-protects-you)), then removed. `--clone-depth N` limits the history fetched. |
| A `.zip`, `.tar`, `.tar.gz`, or `.tgz` archive | Extracted with safety limits into a temporary directory, then removed. Archives have no history. |

When a directory is inside a larger Git repository (a subdirectory rather than the top of
the working tree), it is analyzed as files without history, and the Git section says so.

## Stages

| Stage | Label in progress output | What it does |
|---|---|---|
| `discovery` | Files discovered | Walks the tree, respecting `.gitignore`, `.ignore`, and Git exclude files; classifies every file; detects languages; reads text files up to `max_file_bytes`. |
| `parsing` | Source files parsed | Scans each source file for lines of code, comments, imports, symbols, functions, and approximate complexity. Runs during discovery, in parallel. |
| `git` | Git history analyzed | Reads commits, file changes, contributors, tags and releases, and computes activity, ownership, and change hotspots. |
| `dependencies` | Dependencies analyzed | Reads manifests and lockfiles across ecosystems. |
| `architecture` | Architecture inferred | Infers modules, resolves imports, builds dependency graphs, and finds cycles, layers, entry points, and central modules. |
| `quality` | Quality signals computed | Complexity, size, nesting, markers, dead-code candidates, and hotspots. |
| `duplication` | Duplication detected | Finds duplicated blocks on normalized token streams. |
| `similarity` | File similarity estimated | Finds pairs of files with similar content. |
| `project` | Tests, build, and docs detected | Tests, build systems, commands, CI, environment requirements, and documentation. |
| `security` | Security signals scanned | Possible secrets, risky patterns, and file permissions. |
| `evolution` | Evolution reconstructed | Epochs, events, Time Machine snapshots, and the project story. |
| `historical-architecture` | Historical architecture sampled | Modules and dependencies at each Time Machine snapshot. |
| `plugins` | Plugins executed | Enabled analyzer plugins. |
| `insights` | Insights generated | Answers to first questions, reading order, onboarding steps, the fingerprint, and findings. |

Each stage is documented in more detail:
[Git history](git-analysis.md), [architecture](architecture-analysis.md),
[dependencies](dependency-analysis.md), [complexity and quality](complexity.md),
[tests and build](tests-and-build.md), [security](security.md), and
[the Time Machine](time-machine.md).

## Profiles

A profile chooses which stages run. Choose one with `--profile` or `analysis.profile`.

| Profile | Stages |
|---|---|
| `quick` | Discovery, parsing, tests/build/docs, and insights. No Git history. |
| `standard` (default) | Adds Git history, dependencies, architecture, quality, security, evolution, and enabled plugins. |
| `deep` | Adds duplication, file similarity, and historical architecture at every snapshot. |
| `history-only` | Discovery, Git history, and evolution. |
| `architecture-only` | Discovery, parsing, dependencies, and architecture. |
| `dependencies-only` | Discovery and dependencies. |
| `security-only` | Discovery and security. |

The `include_*` switches in [`[analysis]`](configuration.md#analysis-what-to-analyze) turn
stages off within a profile. Timings for each profile are in the
[benchmarks](../benchmarks/README.md).

## When a stage cannot run

A stage that cannot run does not fail the analysis. Each artifact section has a status:

| Status | Meaning |
|---|---|
| `analyzed` | The stage ran to completion. |
| `partial` | The stage ran, but some inputs could not be processed; the section's notes say which. |
| `skipped` | The stage was not enabled by the profile or configuration. |
| `unavailable` | The stage could not run, for example because Git is not installed or the input has no history. |

The artifact also records every stage's outcome (`completed`, `partial`, `skipped`,
`failed`, or `cancelled`) and duration in `analysisMetadata.analyzers`, and reports and
views say when a section is missing and why, instead of showing empty results as if
nothing was found.

## Evidence, confidence, and severity

Everything RepoDNA concludes is recorded with its evidence: files and lines, commits,
dependency edges, packages, metrics, or observations. Findings also state the method used,
the rationale, the limitations, and next steps. See
[the RepositoryDNA model](repository-dna.md#findings).

Confidence says how directly the evidence supports a conclusion:

| Confidence | Meaning |
|---|---|
| High | Directly supported by explicit, verifiable evidence in the repository. |
| Medium | Supported by consistent structural evidence, but inferred rather than declared. |
| Low | A weak heuristic signal; a pointer for manual investigation. |
| Unavailable | The analysis could not be performed, so no conclusion can be drawn. |

Severity says how urgent a finding is:

| Severity | Meaning |
|---|---|
| Info | A neutral observation that describes the repository. |
| Attention | A structural signal worth reviewing that does not indicate a problem by itself. |
| Warning | A likely problem, supported by at least medium-confidence evidence. |
| Critical | High-confidence evidence of an immediate risk, such as private key material. |

## Performance

- Files are read and parsed in parallel (`--threads` or `performance.parallelism`).
- **The cache.** Per-file results are stored by content hash, language definition, and
  RepoDNA version, so analyzing a repository again only parses files that changed. Use
  `--no-cache` to ignore it and `repodna cache clear` to empty it.
- Git history is read in a small number of streaming `git` commands; `--max-commits` and
  `analysis.max_commits` (50,000 by default) cap it.
- The Time Machine reuses the analysis of files that did not change between snapshots.
- Language patterns are compiled on first use, so languages a repository does not contain
  cost nothing.
- Large outputs are capped (`max_symbols`, `max_file_edges`, `artifact_commits`), and the
  artifact records where results were cut (for example `git.historyTruncated` and
  `architecture.fileEdgesTruncated`).

## Cancellation

Press Ctrl+C (or cancel from the web interface) to stop an analysis. Running Git commands
and plugins are stopped, nothing partial is stored, and the command exits with code 130.

## Reproducibility

Given the same repository revision, configuration, and RepoDNA version, the analysis
produces the same results. Two things vary between runs: the analysis time and the stage
durations. To produce byte-identical artifacts, fix both:

```sh
SOURCE_DATE_EPOCH=1790000000 repodna analyze . --reproducible --no-store --format json > repodna.json
```

The artifact records the RepoDNA version, schema version, analyzers, configuration hash,
and privacy preset in `analysisMetadata`, and the DNA fingerprint and hash in
`fingerprint`.
