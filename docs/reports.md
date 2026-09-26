# Reports

Every report is generated from an analysis artifact alone, so you can regenerate it later,
on another machine, with another theme or privacy preset, and without the repository.

```sh
repodna report                                  # a bundle in ./repodna-report/
repodna report ~/src/project --format html -o report.html
repodna report project.repodna --format markdown -o REPORT.md
```

## Formats

| Format | Command | What you get |
|---|---|---|
| Bundle | `repodna report` | A directory with `index.html`, `report.md`, `repodna.json`, `dna-card.svg`, `dna-card.png`, and `data/*.csv` |
| HTML | `--format html` | One self-contained page |
| Markdown | `--format markdown` | GitHub-flavored Markdown |
| JSON | `--format json` | The artifact, with the privacy preset applied |
| CSV | `--format csv --table NAME` | One table: `files`, `findings`, `metrics`, `hotspots`, `dependencies`, `contributors`, or `languages` |

The **HTML report** is a single file with no external resources: styles and charts
(inline SVG, each with a text description) are embedded and it uses your system fonts, so
it opens offline and can be attached to an e-mail or an issue. It has a table of contents,
tables with the numbers behind the analysis, and a severity filter for findings; it is
fully readable without JavaScript. The report's sections are the same as in
`repodna show --list`.

The **Markdown report** renders on GitHub and GitLab and in most documentation tools.

Both carry a small signature with the analysis identifier, DNA hash, revision, and
RepoDNA and schema versions, so a report can always be traced to the analysis it came from.

## Themes

`--theme` (or `report.theme`) changes the HTML report's look:

| Theme | Look |
|---|---|
| `professional` (default) | Balanced light theme |
| `minimal` | Reduced decoration |
| `technical` | Denser, with more numbers |
| `dark` | Dark background |

## Choosing sections

```sh
repodna report --format markdown --sections summary,architecture,hotspots,findings
```

Section identifiers are listed in [the CLI reference](cli.md#repodna-show). `report.sections`
in `repodna.toml` sets a default.

## Privacy

Apply a [privacy preset](privacy.md#privacy-presets) before sharing:

```sh
repodna report --privacy share -o team-report      # no commit messages, marker text, or command output
repodna report --privacy public -o public-report   # also pseudonymous contributors, no remote URLs or symbol names
```

`--no-branding` omits the "RepoDNA · Made by the Sanskar" credit line.

## Onboarding guide

```sh
repodna onboarding -o docs/onboarding
```

Writes a Markdown guide for new developers: a `README.md` and one page each for the
overview, setup, architecture, important files, testing, dependencies, and recent changes.
Every statement comes from the analysis, and the guide notes that detected commands have not
been run.

## Comparisons

```sh
repodna compare ~/src/service-a ~/src/service-b
repodna compare v1.repodna v2.repodna --format html -o compare.html
```

Puts two or more repositories or analyses side by side, as text, Markdown, JSON, or HTML:
main languages, files, code lines, architecture, direct dependencies, commits, history,
activity, contributor identities, files with tests, documentation checks met, average
complexity, active findings, and the DNA fingerprint dimensions. Comparing two analyses of the same repository shows how it
changed.

## CI summaries

`repodna ci` prints a summary for continuous integration: text, Markdown for job summaries,
JSON, or GitHub Actions annotations. See [the CLI reference](cli.md#repodna-ci) and the
[example workflow](../examples/ci/repodna.yml).

## Badges

```sh
repodna badge -o docs/badges
```

Writes SVG badges for the DNA hash, languages, architecture, activity, and tests, and
prints the Markdown to paste into a README. Badges show only values RepoDNA measured.

## DNA cards

See [DNA cards](dna-cards.md).

## Overwriting safely

Report directories are marked with a `.repodna-output` file. RepoDNA writes into an existing
directory only if it created it or it is empty, replaces a single file only if RepoDNA
generated it, and never writes reports into the root of a Git working tree. `--force`
overrides the first two checks.
