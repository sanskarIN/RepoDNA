# RepoDNA v1.3.3 — screenshots

These images were captured from the **real v1.3.3 web interface and documentation
site**, using headless Chromium against a locally built copy of the v1.3.3 source.
The web views use a new `repodna analyze . --profile standard --format json` artifact
produced from the same checkout; no older screenshot has been renamed or reused.

| Screenshot | Pixels | Notes |
|---|---|---|
| [Overview (light)](desktop/overview-light.png) | 2880 × 1620 | Real current analysis, light palette |
| [Overview (dark)](desktop/overview-dark.png) | 2880 × 1620 | Same analysis in dark palette |
| [Architecture (dark)](desktop/architecture-dark.png) | 2880 × 1620 | Architecture view with actual graph |
| [Phone overview (light)](phone/overview-light.png) | 1170 × 2532 | Mobile layout of the same analysis |
| [Docs index (light)](docs-site/index-light.png) | 2880 × 1620 | Real generated documentation site |
| [CLI guide (dark)](docs-site/cli-dark.png) | 2880 × 1620 | Command-line guide in dark theme |
| [Phone docs (dark)](docs-site/phone-dark.png) | 1170 × 2532 | Configuration guide on a phone |

The screenshots are intentionally a **curated subset** of the v1.3.2 gallery.
The unreproduced older screenshots remain correctly attributed to v1.3.2.
Desktop-app screenshots are not presented as new because this capture runs
Chromium, not a packaged Tauri window. Later platform-native captures should
be added here only after running the actual desktop app.

## Reproducibility

Run the 1.3.3 code in a clean checkout, build the web interface and documentation
site as described in [the development guide](https://github.com/sanskarIN/RepoDNA/blob/main/docs/development.md),
load the artifact generated from the same revision, and use a Chromium viewport
of 1440 × 810 with device scale factor 2, or 390 × 844 with scale factor 3 for phone.

All captured images are PNG screenshots, not artistic mockups.
