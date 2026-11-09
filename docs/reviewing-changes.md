# Reviewing repository changes with RepoDNA

RepoDNA has two complementary review workflows: **baseline impact in the CLI**
and **filtered findings exports in the web interface**. Neither workflow is a
security audit, proof of causality, or a substitute for reading the actual diff.

> **Stable release (2026-11-06):** RepoDNA **[v1.5.0](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.5.0)**
> is published on GitHub as a stable release. Its binaries, installers and
> versioned GitHub Action include the CI `fileOwners` field and local
> Findings-export review introduced by PRs
> [#198](https://github.com/sanskarIN/RepoDNA/pull/198) and
> [#195](https://github.com/sanskarIN/RepoDNA/pull/195).
> The hosted web application may still deploy later changes from `main`
> separately from tagged desktop/CLI releases.

## 1. Build a comparable baseline

Analyze the target repository on the base revision and save a **complete JSON
analysis artifact**, outside the repository checkout:

```sh
repodna analyze /path/to/base-checkout --format json > /path/to/baseline.json
```

Use the **same profile**, configuration, enabled analyzers and comparable
repository scope/history depth for the head revision. Otherwise a changed
signal may reflect different input coverage rather than a code change.
Keep the output files outside the scanned directories.

```sh
repodna ci /path/to/head-checkout --baseline /path/to/baseline.json --format json > /path/to/impact.json
repodna ci /path/to/head-checkout --baseline /path/to/baseline.json --format markdown > /path/to/impact.md
```

The baseline must be a full RepoDNA analysis, **not** a findings-only JSON
export from the web interface. An omitted baseline retains the ordinary CI
summary and does not include `changes`.

## 2. Read the baseline impact

The JSON object preserves `summary`, `failed` and `failing`, and adds
`changes` when `--baseline` is supplied.

| Field | Interpretation |
| --- | --- |
| `changes.files.added` / `removed` | Paths present only in the head or base analysis |
| `changes.files.modified` | Same path with **different known hashes** in both artifacts |
| `changes.findings.introduced` / `resolved` | Deduplicated active finding IDs entering or leaving the result |
| `changes.architecture.touchedModules` | Modules owning changed files in either snapshot |
| `changes.architecture.fileOwners` | For each changed path, its closest mapped baseline and current module IDs (**available in v1.5.0 and later**) |
| `changes.architecture.directDependents` | Modules with a direct dependency on touched modules in either snapshot |
| `changes.architecture.moduleEdges.added` / `removed` | Observed module-to-module relationships that appeared or disappeared |

An illustrative `fileOwners` entry is:

```json
{
  "path": "src/parser/lexer.rs",
  "baselineModule": "src",
  "currentModule": "src/parser"
}
```

For an added file, `baselineModule` is `null`; for a removed file,
`currentModule` is `null`. Either can also be `null` when that snapshot
has no mapped owner. The root module only owns files at the repository root.
Items are sorted by repository-relative path.

**Important limits:**

- A rename is represented as an added path and a removed path; this CI delta
  does not pair renames by identity or hash.
- Missing hashes do **not** establish a modification. A file that did change
  but lacks a comparable hash may be absent from `modified`.
- `changes.architecture` is absent if architecture was not analyzed in
  both artifacts; an unmapped file is not automatically attributed to the root.
- Direct dependents are **one hop**, not transitive blast-radius predictions.
  Changed edge weights alone do not appear under added/removed relationships.
- Finding IDs, not source-line changes or semantic matching, determine
  introduced/resolved records. Suppressed findings are excluded; a previously
  suppressed finding becoming active is introduced.
- Paths, titles, evidence and module names may reveal private repository
  context. Inspect outputs before posting them to an issue or uploading them.

The Markdown CI summary lists representative entries and counts. Introduced
and resolved findings show the first associated path (when available) and a
count of additional paths, rather than claiming an exact evidence line.
Detail lists are bounded at **10 entries**, with per-entry text clipped to
**180 characters** and escaped. The JSON retains full paths and records.

## 3. Add a CI failure policy

```sh
repodna ci /path/to/head-checkout --baseline /path/to/baseline.json --new-only --fail-on warning --format github
```

This fails with exit code **5** only for introduced active findings at
`warning` or `critical` severity. Previously suppressed findings that are
now active count as introduced. With `--new-only`, GitHub annotations target
the introduced findings; their limit is 50 after filtering.

The published [GitHub Action](github-action.md) provides an easier route for
automated CI. **Pin `sanskarIN/RepoDNA@v1.5.0` for the published stable
behavior**; the action does not automatically gain changes merged to `main`
after the release. See
[the practical CI workflow](workflows.md#review-new-findings-in-ci).

## 4. Review and save selected findings in the web interface

In the Findings view, apply search, severity, category, confidence and
**Show suppressed** filters, then choose **Export filtered CSV**, **JSON** or
**Markdown**. In v1.5.0 and the hosted web version, RepoDNA opens a **local
review** showing the selection count, unique paths, evidence count,
suppressed count and a sample of up to **three findings / 4,000 characters**.
It then requires **Save full export** or **Cancel export**. Changing filters
closes a previous review. The final file includes **all** matching findings,
not just the visible page or the preview.

This sample is **not** redaction or secret detection. It cannot demonstrate
that unseen entries are safe. The full export may reveal paths, titles, evidence
and context. It stays local unless you choose to share it.

**On `main` after v1.5.0 (not in the stable v1.5.0 downloads):** the
Findings view also offers **Export counts-only CSV**. It aggregates the current
filtered selection by severity and active/suppressed status, includes totals,
and uses fixed labels and integers instead of individual paths, titles, rules,
summaries, or evidence. Unknown imported severity values are grouped as
`Unknown`, not copied verbatim. Review the entire aggregate in the same
local confirmation panel before saving. Totals can still disclose sensitive
information about a project; this is a lower-disclosure export, not guaranteed
anonymization or a secret scanner. It is also not a full analysis artifact.

A findings-only JSON file is an array of findings, **not** a complete analysis
artifact. Use **Reports & export** (or `repodna analyze --format json`) to
create a complete artifact; those are separate export paths.

For exporting and sharing see [web](web.md), [reports](reports.md), and
[privacy](privacy.md).

## 5. Release and research status

**v1.5.0 is published as stable.** See its [GitHub release](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.5.0),
the [release qualification record](../packaging/release/v1.5.0-checklist.md),
the [roadmap](../ROADMAP.md) and [release history](../CHANGELOG.md).
Local native explanations are extractive; the optional trained ranker is not a
generative language model and benchmark quality claims require independently
reviewed evidence. See [native-model](native-model.md).
