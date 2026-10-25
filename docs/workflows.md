# Practical RepoDNA workflows

Choose a workflow based on **where the repository is available**, whether you need
Git history, and what you want to share. All analysis is local-first. No cloud
service or AI model is required.

| Goal | Starting point | Important limitation |
|---|---|---|
| Explore a folder without installing software | [Web version](web.md#the-web-version) | Browser-only analysis has no Git history |
| Investigate a repository with its Git history | [CLI or desktop app](getting-started.md) | Git must be available for history analysis |
| Review only new CI findings | [CI baseline](#review-new-findings-in-ci) | Baseline and current artifacts must be comparable |
| Share an evidence-backed review | [Reports and findings exports](#share-a-review-without-sharing-source-code) | Reports still contain metadata that needs review |

For supported versions and verified downloads, use the
[installation guide](installation.md). Start with
[troubleshooting](troubleshooting.md) when the result is unexpected.

## Explore a local repository in a browser

1. Open the [web version](https://sanskarin.github.io/RepoDNA/).
2. Choose **Analyze a repository in this browser** and select one project folder
   or a supported archive.
3. Choose the analysis profile, then explore **Overview**, **Files**,
   **Architecture**, **Security signals**, and **Findings**.
4. In **Findings**, narrow the results by search, severity and category, and
   optionally include suppressed findings.
5. Use **Export filtered CSV** for spreadsheets, **JSON** for structured
   evidence, or **Markdown** for a human-readable review.

The static web version reads selected files in your browser rather than
uploading them to a server. It can work offline after the application has been
loaded and cached successfully. Browser analysis does **not** include Git
commits, contributor history, or time-based analysis derived from Git.

If you select a folder, keep the selection to a **single repository root**.
RepoDNA refuses unsafe paths, duplicates, and mixed roots instead of silently
combining unrelated inputs. For incomplete or large analyses, try a smaller
project folder or the native CLI.

See [the web interface](web.md), including its privacy and storage options.

## Investigate a repository with full Git history

After installing the CLI, run these commands **inside the repository**:

```sh
repodna doctor
repodna analyze . --profile standard
repodna findings .
repodna architecture .
repodna history .
repodna hotspots .
repodna serve
```

Open the one-time local URL printed by `repodna serve`. The URL contains a
session token: **do not publish or forward it**. The CLI and local server can
use Git history when Git is installed and the target is a Git working tree.

To change the amount of work:

```sh
repodna analyze . --profile quick
repodna analyze . --profile deep
```

`quick` focuses on fast structural analysis without Git history. `deep`
includes more expensive signals such as duplication and historical
architecture. The `standard` profile is a useful default.

If history is absent, confirm the selected path is the repository's root and
that Git is installed. See [troubleshooting](troubleshooting.md#analysis) and
[the analysis engine](analysis-engine.md#profiles).

## Review new findings in CI

RepoDNA can compare a current scan against an **existing JSON analysis
artifact** made from your baseline revision. For example, while checked out
at the baseline commit or branch, save an artifact:

```sh
repodna analyze . --format json > ../baseline.json
```

In your CI job, make that artifact available as a file and check out the
revision you want to evaluate. Use an absolute path or another path outside
the checkout to avoid accidentally analyzing the baseline file itself:

```sh
repodna ci . --baseline /path/to/baseline.json --new-only --fail-on warning --format github
```

`--new-only` applies the failure threshold to findings absent from the
baseline. `--fail-on warning` includes warning and critical severities.
The CI command returns exit code **5** when the configured gate fails.
Suppressed findings do not fail the gate.

A meaningful comparison requires equivalent analysis profiles and consistent
configuration. Use comparable repository scope and Git history depth; otherwise
a difference may reflect the inputs rather than a code change. For a ready
workflow, see [GitHub Action](github-action.md) and
[command-line options](cli.md#repodna-ci).

## Share a review without sharing source code

Create a report with the strongest built-in sharing privacy preset:

```sh
repodna report . --privacy public -o repodna-review
```

Or use **Reports & export** in the web or desktop interface. The report bundle
contains summaries, evidence and metadata, not a full copy of your source
files. The `public` preset removes additional identifying data, including
contributor names, remote URLs and symbol names. It is **not a guarantee**
that the entire report is non-sensitive: filenames, finding evidence, and
context may still reveal details.

For a smaller review, open **Findings**, apply filters and export:

- **CSV:** spreadsheet-safe tabular findings.
- **JSON:** a machine-readable array retaining full finding evidence.
- **Markdown:** a readable review with method, evidence, limitations and
  suggested next steps.

These exports include all matching findings, not only the currently visible
page. A findings-only JSON array is **not** a full RepositoryDNA artifact; use
**Reports & export** or `repodna analyze --format json` for the full artifact.

Review outputs before distributing them and never attach session tokens,
API keys, or private source files to a public issue. See
[privacy](privacy.md), [reports](reports.md), and
[security](security.md).

## Troubleshoot and report reproducible problems

Start with `repodna doctor` and the
[troubleshooting guide](troubleshooting.md). When opening an issue,
include the RepoDNA version, platform, mode (browser, CLI or desktop),
the command or steps, and the expected versus actual behavior.

```sh
repodna version --json
repodna doctor --json
```

Review the output before sharing. For security-sensitive reports, follow
[responsible disclosure](../SECURITY.md) rather than posting private
details publicly.
