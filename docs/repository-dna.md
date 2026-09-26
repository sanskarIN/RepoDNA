# The RepositoryDNA model

Every analysis produces one **RepositoryDNA artifact**: a versioned JSON document that
describes a repository snapshot. Reports, cards, badges, the web interface, and
comparisons are all generated from the artifact alone, so they can be regenerated later, on
another machine, and without the repository.

```sh
repodna analyze . --format json > repodna.json    # the artifact
repodna export . -o project.repodna               # the same format, for sharing (privacy preset "share")
repodna schema artifact                           # its JSON Schema
```

A `.repodna` file is an artifact with a privacy preset applied; both are JSON. The schema
is published at [`schemas/repodna-artifact.schema.json`](../schemas/repodna-artifact.schema.json),
and TypeScript types generated from it are in [`packages/schema`](../packages/schema).

## Sections

| Field | Contents |
|---|---|
| `schemaVersion`, `tool` | The artifact schema version and the RepoDNA version that wrote it. |
| `identity` | Name, description, remotes, branches, revision, primary languages, and size class. |
| `structure` | Every file with its category, language, size, lines, and flags (generated, vendored, binary), directories, symbols, entry points, and totals. |
| `languages` | Languages with their files, lines, and share of code; primary languages; unrecognized files. |
| `architecture` | Modules, packages, module and file dependency edges, layers, cycles, centrality, external imports, and the inferred style. |
| `git` | Commits, contributors, ownership, activity, releases, dormant periods, file history, and change hotspots. |
| `codeQuality` | Complexity, functions, nesting, duplication, markers, dead-code candidates, and hotspots. |
| `dependencies` | Ecosystems, manifests, lockfiles, dependencies, duplicates, and requirements. |
| `tests` | Test files, frameworks, directories, commands, and (when enabled) test runs. |
| `builds` | Build systems, commands, CI, containers, environment requirements, and (when enabled) build runs. |
| `docs` | README, license, documentation files, and documentation checks. |
| `security` | Possible secrets (fingerprints, never values), risky patterns, and permissions. |
| `evolution` | Epochs, events, Time Machine snapshots, module ages, and the project story. |
| `similarity` | Pairs of similar files (deep profile). |
| `metrics` | Every raw metric with its definition and unit. |
| `findings` | Findings with evidence (see below). |
| `fingerprint` | The DNA fingerprint and the DNA hash. |
| `insights` | Answers to first questions, reading order, onboarding steps, and recent changes. |
| `plugins` | Plugin runs and what they contributed. |
| `generatedReports` | Reports generated from this artifact, when recorded. |
| `analysisMetadata` | Analysis identifier, time, duration, RepoDNA version, profile, configuration hash, thresholds, stage outcomes, data sources, and privacy. |

Sections that did not run carry a `status` (`analyzed`, `partial`, `skipped`, or
`unavailable`) and notes explaining why; see
[the analysis engine](analysis-engine.md#when-a-stage-cannot-run).

## Findings

A finding is a conclusion with everything needed to check it:

```json
{
  "id": "quality.long-function:4f1c2a9b0d3e",
  "rule": "quality.long-function",
  "category": "complexity",
  "severity": "attention",
  "confidence": "medium",
  "title": "run_analysis is 142 lines long",
  "summary": "...",
  "rationale": "Why this matters.",
  "method": "How it was measured.",
  "evidence": [{ "kind": "file", "path": "crates/app/src/analysis.rs", "line": 120, "endLine": 261 }],
  "limitations": ["What the measurement cannot see."],
  "nextSteps": ["What to do about it."],
  "paths": ["crates/app/src/analysis.rs"]
}
```

`id` is stable across runs for the same subject, so suppressions and CI baselines can
refer to it. Evidence items have a `kind`: `file`, `directory`, `symbol`, `commit`,
`dependency-edge`, `package`, `metric`, or `observation`. Severity and confidence levels
are defined in [the analysis engine](analysis-engine.md#evidence-confidence-and-severity).
Suppressed findings keep their place and carry the suppression's reason.

## Rules

| Rule | Default severity | Reported when |
|---|---|---|
| `activity.hotspot` | attention | A file ranks high for both change frequency and complexity. |
| `activity.high-churn-area` | info | A directory takes a large share of recent changes (`high_churn_share`). |
| `architecture.cycle` | attention or warning | Modules depend on each other in a cycle. |
| `architecture.file-cycle` | attention | Files import each other in a cycle. |
| `architecture.concentration` | attention | One module holds a large share of the code. |
| `architecture.centrality` | info | A module connects many parts of the system. |
| `architecture.isolated-module` | info | No resolved import connects a module with any other. |
| `architecture.unresolved` | info | A large share of imports could not be resolved. |
| `build.no-ci` | info | No CI configuration was detected. |
| `contributors.concentration` | info | Most commits come from one contributor identity. |
| `dependencies.lock-mismatch` | warning | A lockfile does not lock every dependency its manifest declares. |
| `dependencies.no-lockfile` | info | Manifests declare dependencies but no lockfile is committed. |
| `dependencies.duplicate-versions` | info | A package is locked at more than one version. |
| `dependencies.stale-manifest` | info | A manifest has not changed for `stale_manifest_days`. |
| `dependencies.concentration` | info | Many files import one external package. |
| `docs.readme-missing` | attention | There is no README. |
| `docs.readme-short` | info | The README is very short or not at the root. |
| `docs.setup-instructions` | info | The README has no installation or usage section. |
| `docs.license-missing` | attention | No license file or declaration was found. |
| `docs.license-file-missing` | info | A license is declared in a manifest but there is no license file. |
| `docs.optional-missing` | info | A contributing guide, code of conduct, security policy, or changelog was not detected. |
| `evolution.dormant-period` | info | Development paused for at least `dormant_days`. |
| `evolution.abandoned-area` | info | A directory has not changed in a long time while the rest of the repository did. |
| `evolution.new-module` | info | A module appeared recently. |
| `quality.large-file` | attention | A file exceeds `large_file_lines`. |
| `quality.long-function` | attention | A function exceeds `large_function_lines`. |
| `quality.complex-function` | attention | A function exceeds `high_complexity`. |
| `quality.deep-nesting` | attention | A function nests deeper than `deep_nesting`. |
| `quality.duplicate-block` | attention | A block of at least `duplicate_min_tokens` tokens appears in several places. |
| `quality.duplication-ratio` | attention | A large share of the analyzed code is duplicated. |
| `quality.similar-files` | info | Files are similar above the `similarity` threshold. |
| `quality.markers` | info | Work markers such as `TODO` and `FIXME` were found. |
| `quality.dead-code` | info | Nothing in the repository appears to use a file. |
| `security.secret` | attention, warning, or critical | A possible credential was found (see [security](security.md)). |
| `security.<pattern>` | info, attention, or warning | A risky construct was found (see [security](security.md#risky-patterns-securityrule)). |
| `security.permission` | attention | A file is world-writable or has the setuid or setgid bit. |
| `structure.large-binary` | info | A committed binary exceeds `large_binary_bytes`. |
| `tests.none` | attention | No automated tests were detected. |
| `tests.low-ratio` | info | There is little test code relative to source code. |
| `tests.not-in-ci` | info | CI configuration does not appear to run the tests. |
| `tests.coverage-committed` | info | Coverage reports are committed. |
| `plugin.<name>.<rule>` | set by the plugin | Reported by an enabled [plugin](plugins.md). |

Thresholds in `code format` are [configurable](configuration.md#thresholds-when-signals-are-reported).

## The DNA fingerprint

The fingerprint places a repository on eight dimensions, each scored from 0 to 1 with a
documented formula and the raw value behind it:

| Dimension | Formula |
|---|---|
| Language diversity | 1 − Σ share² over each language's share of first-party code (0 = one language). |
| Modularity | log2(modules) ÷ 6, capped at 1 (64 or more modules). |
| Dependency centrality | Modules that depend on the most depended-upon module, divided by the number of other modules. |
| Change concentration | Share of file changes that touch the most-changed 10% of current files, rescaled so that evenly spread changes give 0. |
| Contributor spread | 1 − Σ share² over each contributor identity's share of commits. |
| Age | ln(1 + years) ÷ ln(21), capped at 1 (20 years or more). |
| Test presence | Share of first-party code files that are test files or contain inline tests. |
| Documentation | Share of documentation checks that are met (partial matches count half). |

The fingerprint is descriptive, not a score: a high or low value is neither good nor bad
on its own. It appears on the [DNA card](dna-cards.md) and in the Overview.

## The DNA hash

`fingerprint.dnaHash` (for example `rdna1-3f9a...`) identifies the exact content of a
snapshot: it is computed from every file's path and content hash, so two analyses of the
same files have the same hash, whatever the time, machine, or configuration.

## Versioning

`schemaVersion` is `major.minor`. Readers must ignore unknown fields:

- A newer **minor** version only adds optional fields, so artifacts remain readable by
  older RepoDNA versions with the same major version.
- An older **major** version is upgraded on read.
- A newer **major** version is rejected with an error that names both versions.

The Settings page of the web interface shows the versions as, for example,
"RepoDNA v1.0.0" and "Analysis schema v1".

Artifacts are written atomically (to a temporary file that is then renamed), so an
interrupted export never leaves a truncated file.
