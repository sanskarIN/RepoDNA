# Project DNA cards

A Project DNA card summarizes a repository in one image: its name and description, a
double helix colored by its languages, key numbers, its architecture style, activity, and
tests, and its DNA hash. It is made for README files, slides, and posts.

![RepoDNA's own Project DNA card](../examples/self-analysis/dna-card.svg)

```sh
repodna card                                   # dna-card.svg for the current repository
repodna card --format png -o dna-card.png      # a PNG at twice the size (2400 × 1260)
repodna card --dark -o dna-card-dark.svg
repodna card ~/src/project --privacy public --no-branding
```

The same card is part of every report bundle (`dna-card.svg` and `dna-card.png`), is
shown in the web interface's Reports view, and is served by `repodna serve` at
`/api/repositories/{id}/card.svg`.

## What it shows

| Element | Source |
|---|---|
| Name, owner, and description | The repository's identity and the description found in its README or manifests |
| Helix | Each rung is colored by a language, in proportion to its share of first-party code |
| Legend | The four largest languages with their shares |
| Files, code lines | The structure section |
| Commits, contributors, age | The Git history (shown as — when there is no history) |
| Dependencies | Declared direct dependencies |
| Architecture, activity, tests | The inferred style, the activity level, and whether tests were detected |
| DNA hash | The content hash of the snapshot (see [the RepositoryDNA model](repository-dna.md#the-dna-hash)) |
| Footer | The remote, the revision, the analysis date, and the "RepoDNA · Made by the Sanskar" credit (`--no-branding` removes the credit) |

Every value is measured; nothing on a card is estimated or invented. Long names and
descriptions are shortened with an ellipsis so the layout never breaks.

## Formats

- **SVG** is small, sharp at any size, and includes a text description for screen readers.
  Text uses the bundled DejaVu Sans metrics for layout and your system's fonts for display.
- **PNG** is rendered with the bundled DejaVu Sans fonts, so it looks the same on every
  machine, at 2400 × 1260 pixels.

## In a README

```markdown
![Project DNA](docs/dna-card.svg)
```

Regenerate the card when the repository changes, for example in CI:

```sh
repodna card --privacy public -o docs/dna-card.svg --force
```

For small signals in a README header, use [badges](reports.md#badges).

## Privacy

Cards show the repository's name, owner, remote, and aggregate numbers. `--privacy public`
removes the remote URL; contributor names never appear on a card.
