# Releases

Every release of RepoDNA, newest first. Each folder holds that release's notes, its
downloads with their sizes, and the commands that install it. The same notes are on the
[GitHub releases page](https://github.com/sanskarIN/RepoDNA/releases), and the
[changelog](../../CHANGELOG.md) has all of them on one page.

| Version | Released | Highlights |
|---|---|---|
| [1.3.0](v1.3.0/README.md) | 2026-10-09 | Recent analyses, the open analysis kept across reloads, ways through an analysis, and a web version that installs as an app, saves tables as CSV, and prints cleanly |
| [1.2.2](v1.2.2/README.md) | 2026-10-07 | The shade on tables that scroll sideways on phones along their whole edge, and older releases on npmjs.com |
| [1.2.1](v1.2.1/README.md) | 2026-10-06 | npm packages on npmjs.com, installed without a token, and small fixes to accessibility, tables on phones, `repodna serve`, and clone errors |
| [1.2.0](v1.2.0/README.md) | 2026-10-05 | The web interface as a container image, npm packages on GitHub Packages, and a web interface and reports that fit phone screens |
| [1.1.1](v1.1.1/README.md) | 2026-10-05 | Small fixes across the command line, reports, and the web interface |
| [1.1.0](v1.1.0/README.md) | 2026-10-04 | Optional AI explanations, grounded in the analysis and checked against its evidence |
| [1.0.0](v1.0.0/README.md) | 2026-09-27 | The first stable release |

Each release page shows screenshots of that version; all of them, side by side, are in
[`docs/images`](../images/README.md).

## Where the files are

- **Command line and desktop app:** attached to each
  [GitHub release](https://github.com/sanskarIN/RepoDNA/releases), with a
  `SHA256SUMS.txt` to check them.
- **Container images:** `ghcr.io/sanskarin/repodna` since 1.0.0 and
  `ghcr.io/sanskarin/repodna-web` since 1.2.0, tagged with the version (`1.2.0`), its
  minor and major versions (`1.2`, `1`), and `latest`.
- **npm packages:** `@sanskarin/repodna`, `@sanskarin/repodna-schema`, and
  `@sanskarin/repodna-visualization` on npmjs.com and GitHub Packages from 1.2.0 on, and
  `@sanskarin/repodna-web` from 1.3.0 on; see [installation](../installation.md#npm-packages).
- **Web version:** [sanskarin.github.io/RepoDNA](https://sanskarin.github.io/RepoDNA/)
  always runs the latest release.

## Adding a release

When a version is published, add a folder named after its tag, such as `v1.3.0`, with a
`README.md` in the same shape as the others: its notes from the changelog, its downloads,
how to install it, and its screenshots, which go in `docs/images/v1.3.0`. Then add a row
at the top of the table above. The [release steps](../development.md#releasing) list this
with the rest.
