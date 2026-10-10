# GitHub Action

RepoDNA's GitHub Action analyzes your repository in a workflow. It shows the summary on the
run's page and each finding as an annotation on the file it is about, keeps the analysis as
a `.repodna` file and, if you ask, the full report, and fails the step when findings reach
the severity you choose. It runs on the runner: the code is never uploaded. It downloads
the RepoDNA release it runs from this repository's releases, and checks it against the
release's `SHA256SUMS.txt`.

```yaml
name: RepoDNA

on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: read

jobs:
  repodna:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          # The whole history, for the history, hotspot, and evolution analyses.
          fetch-depth: 0
      - uses: sanskarIN/RepoDNA@v1.3.1
        with:
          fail-on: warning
```

It runs on Linux, macOS, and Windows runners, on x64 and Arm. Accept known findings with a
reason in your `repodna.toml` (`[[suppress]]`, see [configuration](configuration.md)).

## Inputs

| Input | Default | What it does |
| --- | --- | --- |
| `path` | `.` | The directory to analyze. |
| `version` | the release of the action's tag | The RepoDNA release to run, such as `1.3.1`, or `latest`. Used at a branch or a commit, the action runs the latest release. |
| `profile` | the repository's `repodna.toml`, or `standard` | The analysis profile: `quick`, `standard`, `deep`, `history-only`, `architecture-only`, or `dependencies-only`. |
| `fail-on` | `never` | Fail the step when findings at or above this severity exist: `never`, `info`, `attention`, `warning`, or `critical`. |
| `baseline` | none | An earlier analysis (`.repodna` or JSON) to compare with, such as the base branch's. |
| `new-only` | `false` | With `baseline`, fail only on findings the baseline does not have. |
| `annotate` | `true` | Show findings as annotations on the files they are about. |
| `report` | none | A directory to write the full report to: HTML, Markdown, JSON, CSV tables, and the Project DNA card. |

## Outputs

| Output | What it is |
| --- | --- |
| `analysis` | The analysis as a `.repodna` file. It opens in the [web version](web.md), `repodna serve`, and the [desktop app](desktop.md). |
| `report` | The report directory, when `report` was given. |

## Keep the report

```yaml
      - id: repodna
        uses: sanskarIN/RepoDNA@v1.3.1
        with:
          report: ${{ runner.temp }}/repodna-report
      - uses: actions/upload-artifact@v7
        with:
          name: repodna
          path: |
            ${{ steps.repodna.outputs.report }}
            ${{ steps.repodna.outputs.analysis }}
```

## Fail only on new findings in pull requests

Analyze the base branch too, and compare the pull request with it. Each branch is checked
out in a folder of its own, so that neither analysis includes the other:

```yaml
    steps:
      - uses: actions/checkout@v7
        with:
          path: head
          fetch-depth: 0
      - uses: actions/checkout@v7
        with:
          ref: ${{ github.base_ref }}
          path: base
          fetch-depth: 0
      - id: base
        uses: sanskarIN/RepoDNA@v1.3.1
        with:
          path: base
          annotate: false
      - uses: sanskarIN/RepoDNA@v1.3.1
        with:
          path: head
          baseline: ${{ steps.base.outputs.analysis }}
          new-only: true
          fail-on: warning
```

Each use of the action keeps its analysis apart, and adds its summary to the run's page.

The action runs `repodna export`, `repodna report`, and `repodna ci`; see the
[command line](cli.md) for what each does. For a workflow without the action, see
[`examples/ci/repodna.yml`](../examples/ci/repodna.yml).
