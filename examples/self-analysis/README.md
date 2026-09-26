# RepoDNA analyzing itself

RepoDNA's analysis of its own repository at revision `35af2a9`, made with the deep profile
from a clean clone. It is the same analysis that the web interface opens as its demo
([`apps/web/public/demo/repodna.json`](../../apps/web/public/demo/repodna.json)).

| File | What it is |
|---|---|
| [`report.md`](report.md) | The full report in Markdown: every section, with findings and their evidence |
| [`dna-card.svg`](dna-card.svg), [`dna-card-dark.svg`](dna-card-dark.svg) | The Project DNA card, light and dark |
| [`badges/`](badges) | README badges: languages, architecture, activity, tests, and DNA |

The files were written from the stored analysis with these commands:

```sh
A=apps/web/public/demo/repodna.json
repodna report $A --format markdown -o examples/self-analysis/report.md
repodna card $A -o examples/self-analysis/dna-card.svg
repodna card $A --dark -o examples/self-analysis/dna-card-dark.svg
repodna badge $A -o examples/self-analysis/badges
```

To analyze the current state of the repository yourself, run `repodna analyze . --profile
deep` in a clone, then `repodna report` or `repodna serve`. The HTML report
(`repodna report --format html -o report.html`) is a single self-contained page with
charts; it is not included here because GitHub shows HTML files as source.
