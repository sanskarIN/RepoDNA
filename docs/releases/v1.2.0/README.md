# RepoDNA 1.2.0

Released on 2026-10-05: [GitHub release](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.2.0) · [changes since 1.1.1](https://github.com/sanskarIN/RepoDNA/compare/v1.1.1...v1.2.0) · [source code at v1.2.0](https://github.com/sanskarIN/RepoDNA/tree/v1.2.0).

More ways to get RepoDNA from GitHub Packages, and a round of fixes and refinements to the
web interface, the desktop app, and reports, above all on phones and in narrow windows.

## What changed

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

The files are attached to the [GitHub release](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.2.0).

| File | What it is | Size |
|---|---|---|
| [`repodna-1.2.0-x86_64-unknown-linux-musl.tar.gz`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.2.0/repodna-1.2.0-x86_64-unknown-linux-musl.tar.gz) | Command line for Linux, x86_64 (static) | 11.5 MB |
| [`repodna-1.2.0-aarch64-unknown-linux-musl.tar.gz`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.2.0/repodna-1.2.0-aarch64-unknown-linux-musl.tar.gz) | Command line for Linux, ARM64 (static) | 10.7 MB |
| [`repodna-1.2.0-aarch64-apple-darwin.tar.gz`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.2.0/repodna-1.2.0-aarch64-apple-darwin.tar.gz) | Command line for macOS, Apple silicon | 10.4 MB |
| [`repodna-1.2.0-x86_64-apple-darwin.tar.gz`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.2.0/repodna-1.2.0-x86_64-apple-darwin.tar.gz) | Command line for macOS, Intel | 11.0 MB |
| [`repodna-1.2.0-x86_64-pc-windows-msvc.zip`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.2.0/repodna-1.2.0-x86_64-pc-windows-msvc.zip) | Command line for Windows, x86_64 | 11.0 MB |
| [`RepoDNA_1.2.0_amd64.deb`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.2.0/RepoDNA_1.2.0_amd64.deb) | Desktop app for Debian and Ubuntu | 12.0 MB |
| [`RepoDNA-1.2.0-1.x86_64.rpm`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.2.0/RepoDNA-1.2.0-1.x86_64.rpm) | Desktop app for Fedora and openSUSE | 12.0 MB |
| [`RepoDNA_1.2.0_universal.dmg`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.2.0/RepoDNA_1.2.0_universal.dmg) | Desktop app for macOS (Apple silicon and Intel) | 21.3 MB |
| [`RepoDNA_1.2.0_x64_en-US.msi`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.2.0/RepoDNA_1.2.0_x64_en-US.msi) | Desktop app for Windows (MSI installer) | 11.5 MB |
| [`RepoDNA_1.2.0_x64-setup.exe`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.2.0/RepoDNA_1.2.0_x64-setup.exe) | Desktop app for Windows (setup program) | 7.6 MB |
| [`SHA256SUMS.txt`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.2.0/SHA256SUMS.txt) | SHA-256 checksums of every file | 1.0 kB |

Also published for this version:

- `ghcr.io/sanskarin/repodna:1.2.0`: the command line with Git, for `linux/amd64` and `linux/arm64`.
- `ghcr.io/sanskarin/repodna-web:1.2.0`: the web interface, served on port 8080.
- `@sanskarin/repodna@1.2.0` (the command line, with one package per platform), `@sanskarin/repodna-schema@1.2.0`, and `@sanskarin/repodna-visualization@1.2.0` on the npm registry of GitHub Packages.

## Install this version

```sh
# Linux, x86_64
curl -LO https://github.com/sanskarIN/RepoDNA/releases/download/v1.2.0/repodna-1.2.0-x86_64-unknown-linux-musl.tar.gz
tar -xzf repodna-1.2.0-x86_64-unknown-linux-musl.tar.gz
sudo install -m 0755 repodna-1.2.0-x86_64-unknown-linux-musl/repodna /usr/local/bin/repodna

# From source, with a Rust toolchain
cargo install --git https://github.com/sanskarIN/RepoDNA --tag v1.2.0 --locked repodna-cli

# In a container
docker run --rm -v "$PWD:/work" ghcr.io/sanskarin/repodna:1.2.0 analyze .

# With npm, after setting up GitHub Packages (see the installation guide)
npm install --global @sanskarin/repodna@1.2.0
```

The [installation guide at v1.2.0](https://github.com/sanskarIN/RepoDNA/blob/v1.2.0/docs/installation.md) covers every
platform, checking the downloads against `SHA256SUMS.txt`, and opening the unsigned
binaries on macOS and Windows.

## Screenshots

<a href="../../images/v1.2.0/desktop/overview-light.png"><img src="../../images/v1.2.0/desktop/overview-light.png" alt="The overview in RepoDNA 1.2.0" width="400"></a> <a href="../../images/v1.2.0/desktop/architecture-dark.png"><img src="../../images/v1.2.0/desktop/architecture-dark.png" alt="The module map in RepoDNA 1.2.0" width="400"></a>

Twelve desktop views and the phone screens of this version, showing its own analysis of
this repository at `v1.2.0`, are in [`docs/images/v1.2.0`](../../images/README.md#120).

## Images

Screenshots, promo images, and Project DNA cards for posts about this release are in
the [media kit](../../media/README.md#120).

[All releases](../README.md)
