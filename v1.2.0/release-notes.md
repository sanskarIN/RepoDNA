# RepoDNA 1.2.0: release notes

The notes of the GitHub release [RepoDNA 1.2.0](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.2.0), tag
`v1.2.0`, as published on 2026-10-05. The [release page](README.md) adds its
downloads with their sizes.

---

More ways to get RepoDNA from GitHub Packages, and a round of fixes and refinements to the
web interface, the desktop app, and reports, above all on phones and in narrow windows.

### Added

- A container image of the web interface, `ghcr.io/sanskarin/repodna-web`, to serve the
  web version on your own network or without an internet connection. nginx serves it as
  an unprivileged user on port 8080, with the same security headers as `repodna serve`, on
  `linux/amd64` and `linux/arm64`.
- npm packages on the GitHub Packages registry. `@sanskarin/repodna` installs the
  `repodna` command line, with the binary for your platform from
  `@sanskarin/repodna-linux-x64`, `-linux-arm64`, `-darwin-x64`, `-darwin-arm64`, or
  `-win32-x64`. `@sanskarin/repodna-schema` has the TypeScript types of the analysis
  artifact, helpers to load and check it, and the JSON Schemas of the artifact and the
  configuration file. `@sanskarin/repodna-visualization` has the chart geometry of the
  web interface.
- Copy buttons for the commands on the start page, on the Reports & export page, and in
  each step of the onboarding guide.
- In narrow windows, the navigation folds behind a Menu button, and the top bar fits on a
  phone screen.

### Changed

- Summary tiles are laid out in even rows: six tiles form two rows of three instead of a
  row of five and one tile on its own.
- Sortable table columns show a faint arrow, and text columns sort numbers by their value,
  so `file2.rs` comes before `file10.rs`.
- The severity filters of the Findings view span its width and highlight under the
  pointer, like the buttons they are.
- The Hotspots view describes a single hotspot instead of drawing a map with one box.
- Search shows views, commands, and findings in the interface font, keeps the code font
  for paths and package names, and says when it lists only the first 60 of its results.
- Opening a file that is not an analysis names the file, and says in plain words when it
  is empty or is not JSON.
- The story of a repository dates a new directory like the other events: "src/ledger/
  appeared on 2022-11-07." instead of "(2022-11-07)".

### Fixed

- On phones, the one-column layout shrinks to the screen, long commands scroll inside
  their box, and long link addresses on the About page wrap, instead of widening the page.
- HTML reports wrap long paths in findings and onboarding answers, so a phone no longer
  scrolls the whole report sideways.
- The chart and table toggle and the commands in the start page's drop zone stay on one
  line in narrow panels.
- Chart tooltips stay inside narrow phone screens, and open above the pointer in the
  lower half of the window instead of being cut off by its bottom edge.
- Moving through search results with the arrow keys keeps the highlighted result in view,
  and screen readers hear "No matches." when a search finds nothing.

### Documentation

- The installation guide describes the web interface image and how to install the npm
  packages from GitHub Packages. The README and the web interface guide mention them, and
  the development guide lists the packages each release publishes and how to check them.
- The release steps in the development guide list every file that names the version.
- The security policy names 1.2.x as the version that receives security fixes; it still
  named 1.0.x.

## Downloads

- `repodna-1.2.0-<target>`: the `repodna` command line, with the web interface of
  `repodna serve` built in. The Linux builds are static and run on any distribution.
- `RepoDNA_1.2.0_amd64.deb`, `RepoDNA-1.2.0-1.x86_64.rpm`,
  `RepoDNA_1.2.0_universal.dmg`, `RepoDNA_1.2.0_x64_en-US.msi`, and
  `RepoDNA_1.2.0_x64-setup.exe`: the desktop app.
- `ghcr.io/sanskarin/repodna:1.2.0`: a container image with the command line and
  Git, for CI jobs: `docker run --rm -v "$PWD:/work" ghcr.io/sanskarin/repodna analyze .`
- `ghcr.io/sanskarin/repodna-web:1.2.0`: the web interface, served on port 8080, for
  your own network: `docker run --rm -p 8080:8080 ghcr.io/sanskarin/repodna-web`
- npm packages on GitHub Packages: `@sanskarin/repodna` (the command line, with one
  package per platform), `@sanskarin/repodna-schema`, and
  `@sanskarin/repodna-visualization`. Installing them needs a GitHub token; see the
  installation guide.
- `SHA256SUMS.txt`: checksums of every file.
- Web version: https://sanskarin.github.io/RepoDNA/ — open analyses and the demo in your browser, nothing to install.

The binaries are not code-signed, so macOS and Windows ask for confirmation the first time they start; see the [installation guide](https://github.com/sanskarIN/RepoDNA/blob/v1.2.0/docs/installation.md).

More open-source projects by Sanskar, the creator of RepoDNA: https://sanskarin.github.io
