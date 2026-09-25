# Getting started

This walk-through takes about five minutes: install RepoDNA, analyze a repository, explore
it in the terminal and the browser, and write a report and a DNA card.

## 1. Install

Download a binary from the [releases page](https://github.com/sanskarIN/RepoDNA/releases)
or build from source; see [installation](installation.md). Check it:

```sh
repodna --version
repodna doctor
```

## 2. Analyze a repository

Point RepoDNA at a directory, an archive, or a Git URL:

```sh
cd ~/src/some-project
repodna analyze
```

Progress is shown per stage, followed by a summary. For a small service it looks like this:

```text
polyglot · revision 540e92db6965 · standard profile

  Tracks parcels from pickup to delivery.

  Files         17 (8 source, 2 test)
  Code lines    146
  Languages     Go 51%, TypeScript 30%, Python 10%, SQL 6%
  History       3 commits by 2 contributors, 2024-01-08 → 2024-03-20
  Architecture  Monorepo (high confidence), 4 modules
  Dependencies  4 declared directly
  Tests         2 test files
  Findings      0 critical · 0 warnings · 2 attention · 9 info

Top findings
  [attention] example.invalid/parcels holds 50.6% of the code
  [attention] No license was found
  [informational] parcel-dashboard has no detected links to other modules
  ...
Stored as analysis 4f72dd572c00. `repodna report .` writes the full report.
```

(This is the `polyglot` [fixture repository](../fixtures/README.md); you can generate it
and the others with `cargo xtask fixtures` in a clone of RepoDNA.)

The analysis is stored locally, so the commands below reuse it instead of analyzing again.
Nothing in the repository was modified, nothing was executed, and nothing left your
machine.

Other inputs:

```sh
repodna analyze https://github.com/sanskarIN/RepoDNA    # cloned to a temporary directory
repodna analyze project.zip                             # extracted to a temporary directory
repodna analyze . --profile deep                        # also duplication, similarity, historical architecture
repodna analyze . --profile quick                       # files only, no history
```

## 3. Explore in the terminal

```sh
repodna findings                          # every finding with its evidence
repodna findings --severity warning       # warnings and critical findings only
repodna architecture                      # modules, dependencies, layers, cycles
repodna history                           # commits, contributors, releases, activity
repodna hotspots                          # files that change often and are complex
repodna timeline                          # Time Machine snapshots and the project story
repodna show --section summary,tests,build
```

Every finding says what was measured, why it matters, how it was measured, what the
measurement cannot see, and what to do next.

## 4. Explore in the browser

```sh
repodna serve
```

Open the link it prints (it contains a session token). The web interface shows the same
analysis with an architecture map, history charts, the Time Machine, file and symbol
browsers, and every finding with its evidence. Press `Ctrl+K` (or `Cmd+K`) to search
views, files, modules, packages, and findings. See [the web interface](web.md), or use the
[desktop app](desktop.md) instead.

## 5. Write a report and a card

```sh
repodna report                                  # ./repodna-report/index.html, report.md, repodna.json, ...
repodna report --format html -o report.html     # one self-contained page
repodna card                                    # dna-card.svg
repodna badge                                   # README badges
repodna onboarding -o docs/onboarding           # an onboarding guide for new developers
```

Before sharing, add `--privacy public` to replace contributor names with pseudonyms and
remove remote URLs and symbol names. See [reports](reports.md), [DNA cards](dna-cards.md),
and [privacy](privacy.md).

## 6. Use it in CI

```sh
repodna ci --fail-on warning --format github
```

fails the job (exit code 5) when a finding at warning severity or above exists, and prints
GitHub Actions annotations. Accept known findings with a reason in `repodna.toml`:

```toml
[[suppress]]
rule = "security.secret"
path = "tests/fixtures/**"
reason = "Intentional fake credentials used by tests"
```

See the [example workflow](../examples/ci/repodna.yml).

## Next steps

- [Configure](configuration.md) profiles, thresholds, and ignore patterns (`repodna init`
  writes a commented `repodna.toml`).
- Learn what each part of the analysis does, starting with
  [the analysis engine](analysis-engine.md).
- Add languages or analyzers with [plugins](plugins.md).
- Optionally, [ask an AI model](ai.md) to explain the analysis in prose.
