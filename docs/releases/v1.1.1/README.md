# RepoDNA 1.1.1

Released on 2026-10-05: [GitHub release](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.1.1) · [changes since 1.1.0](https://github.com/sanskarIN/RepoDNA/compare/v1.1.0...v1.1.1) · [source code at v1.1.1](https://github.com/sanskarIN/RepoDNA/tree/v1.1.1).

A maintenance release: small fixes across the command line, reports, and the web
interface, and a link to more open-source projects by RepoDNA's creator.

## What changed

### Added

- Links to [sanskarin.github.io](https://sanskarin.github.io), with more open-source
  projects by Sanskar, the creator of RepoDNA: on the About & support page and in the
  sidebar of the web interface and the desktop app, in `repodna version`, and in the
  README, the documentation, and the release notes.

### Fixed

- `repodna config path` prints only the path on standard output, so
  `$(repodna config path)` works before the file exists; the note that the file is missing
  goes to standard error.
- A local AI program that leaves a background process holding its output open no longer
  delays the answer until that process exits.
- A repository without an inferred architecture no longer shows "unavailable confidence"
  in the terminal summary, in reports, or in the web interface.
- Counts read correctly in the singular in the command line, reports, badges, the built-in
  page of `repodna serve`, plugin messages, and the web interface ("1 module", "1 file",
  "1 commit", "1 analysis"), and a comparison no longer says "half of the commits by 1"
  for a repository with one contributor.
- The first-look answers and the onboarding guide say when no test files were found
  instead of "0 test files. Run them with …", and a command that ends with a period,
  such as `pip install -e .`, no longer gets a second one.
- Changes to files at the top level are described as being at the repository root
  instead of in "(root)/".
- The Architecture view of the web interface names the inferred style instead of saying
  "Looks like a …".
- Published release notes keep the link to the web version.

### Documentation

- The README names the network use you can ask for: cloning a URL, or reaching an AI
  provider you configured.
- The web interface guide and troubleshooting explain why a copy of the web version on
  GitHub Pages can show the README instead of the interface.
- Risky-pattern findings are named `security.<rule>` in the guide to the RepositoryDNA
  model, as in the security guide.
- The 1.1.0 entry of this changelog is dated on the day it was published.

## Downloads

The files are attached to the [GitHub release](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.1.1).

| File | What it is | Size |
|---|---|---|
| [`repodna-1.1.1-x86_64-unknown-linux-musl.tar.gz`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.1/repodna-1.1.1-x86_64-unknown-linux-musl.tar.gz) | Command line for Linux, x86_64 (static) | 11.5 MB |
| [`repodna-1.1.1-aarch64-unknown-linux-musl.tar.gz`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.1/repodna-1.1.1-aarch64-unknown-linux-musl.tar.gz) | Command line for Linux, ARM64 (static) | 10.7 MB |
| [`repodna-1.1.1-aarch64-apple-darwin.tar.gz`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.1/repodna-1.1.1-aarch64-apple-darwin.tar.gz) | Command line for macOS, Apple silicon | 10.4 MB |
| [`repodna-1.1.1-x86_64-apple-darwin.tar.gz`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.1/repodna-1.1.1-x86_64-apple-darwin.tar.gz) | Command line for macOS, Intel | 11.0 MB |
| [`repodna-1.1.1-x86_64-pc-windows-msvc.zip`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.1/repodna-1.1.1-x86_64-pc-windows-msvc.zip) | Command line for Windows, x86_64 | 11.0 MB |
| [`RepoDNA_1.1.1_amd64.deb`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.1/RepoDNA_1.1.1_amd64.deb) | Desktop app for Debian and Ubuntu | 12.0 MB |
| [`RepoDNA-1.1.1-1.x86_64.rpm`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.1/RepoDNA-1.1.1-1.x86_64.rpm) | Desktop app for Fedora and openSUSE | 12.0 MB |
| [`RepoDNA_1.1.1_universal.dmg`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.1/RepoDNA_1.1.1_universal.dmg) | Desktop app for macOS (Apple silicon and Intel) | 21.3 MB |
| [`RepoDNA_1.1.1_x64_en-US.msi`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.1/RepoDNA_1.1.1_x64_en-US.msi) | Desktop app for Windows (MSI installer) | 11.5 MB |
| [`RepoDNA_1.1.1_x64-setup.exe`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.1/RepoDNA_1.1.1_x64-setup.exe) | Desktop app for Windows (setup program) | 7.6 MB |
| [`SHA256SUMS.txt`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.1/SHA256SUMS.txt) | SHA-256 checksums of every file | 1.0 kB |

Also published for this version:

- `ghcr.io/sanskarin/repodna:1.1.1`: the command line with Git, for `linux/amd64` and `linux/arm64`.

## Install this version

```sh
# Linux, x86_64
curl -LO https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.1/repodna-1.1.1-x86_64-unknown-linux-musl.tar.gz
tar -xzf repodna-1.1.1-x86_64-unknown-linux-musl.tar.gz
sudo install -m 0755 repodna-1.1.1-x86_64-unknown-linux-musl/repodna /usr/local/bin/repodna

# From source, with a Rust toolchain
cargo install --git https://github.com/sanskarIN/RepoDNA --tag v1.1.1 --locked repodna-cli

# In a container
docker run --rm -v "$PWD:/work" ghcr.io/sanskarin/repodna:1.1.1 analyze .
```

The [installation guide at v1.1.1](https://github.com/sanskarIN/RepoDNA/blob/v1.1.1/docs/installation.md) covers every
platform, checking the downloads against `SHA256SUMS.txt`, and opening the unsigned
binaries on macOS and Windows.

[All releases](../README.md)
