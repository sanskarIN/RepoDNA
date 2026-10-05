# RepoDNA 1.1.0

Released on 2026-10-04: [GitHub release](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.1.0) · [changes since 1.0.0](https://github.com/sanskarIN/RepoDNA/compare/v1.0.0...v1.1.0) · [source code at v1.1.0](https://github.com/sanskarIN/RepoDNA/tree/v1.1.0).

Optional AI explanations: prose about a repository, grounded in the analysis, checked
against its evidence, and off until you configure a provider.

## What changed

### Explanations

- `repodna explain` explains a repository (`--about repository`, `architecture`,
  `history`, `dependencies`, or `onboarding`), one module (`--module`), one hotspot
  (`--hotspot`), or answers a question (`--ask`), as text, Markdown, or JSON with its
  provenance: provider, model, revision, cited evidence, and token use.
- The model receives a numbered selection of evidence from the analysis, never the
  repository, and its answer is checked against it: citations of evidence that was not
  sent are removed, statements without a valid citation are labeled as not supported, and
  statements the model marks as inferences are labeled as such.
- Three kinds of providers: a local program that reads the prompt on standard input, a
  server with a compatible chat API, on your machine or hosted, and a hosted provider's
  own API. The [AI explanations guide](../../ai.md#providers) describes each one.
- `--dry-run` shows the exact prompt, the evidence, where it would be sent, and the
  highest cost from prices you set, without contacting anything.
- Answers are cached on this machine; `--fresh` asks again and `repodna cache clear`
  removes them. When a model declines, RepoDNA reports the reason the provider gives.
- `repodna doctor` reports the configured provider without contacting it.

### Privacy and safety

- AI is set only in your user configuration: a repository's own configuration cannot
  enable it or allow remote providers.
- A provider whose endpoint is not on this machine is refused unless you allow remote AI
  with `privacy.remote_ai = true` or, for one run, `--allow-remote-ai`.
- API keys are read from an environment variable you name and never stored. A provider's
  default key variable is sent only to that provider's own endpoint.
- No source code is sent unless you enable `ai.include_source_excerpts`: excerpts are a
  few lines per file, have likely secrets redacted, and are never read through a link
  that leads out of the checkout. Contributor names and email addresses are never sent.
- The model gets no tools: it cannot run commands, read files, or use the network through
  RepoDNA. Repository text is sent as data, and answers are cleaned of terminal control
  sequences and escaped in Markdown.
- RepoDNA uses the network only to clone a Git URL you give and to reach an AI provider
  you configure.

### Web interface and desktop app

- The start page, the settings, and the analysis details say that explanations are
  optional and need a provider you configure, and whether an analysis used remote AI.

## Downloads

The files are attached to the [GitHub release](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.1.0).

| File | What it is | Size |
|---|---|---|
| [`repodna-1.1.0-x86_64-unknown-linux-musl.tar.gz`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.0/repodna-1.1.0-x86_64-unknown-linux-musl.tar.gz) | Command line for Linux, x86_64 (static) | 11.5 MB |
| [`repodna-1.1.0-aarch64-unknown-linux-musl.tar.gz`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.0/repodna-1.1.0-aarch64-unknown-linux-musl.tar.gz) | Command line for Linux, ARM64 (static) | 10.6 MB |
| [`repodna-1.1.0-aarch64-apple-darwin.tar.gz`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.0/repodna-1.1.0-aarch64-apple-darwin.tar.gz) | Command line for macOS, Apple silicon | 10.4 MB |
| [`repodna-1.1.0-x86_64-apple-darwin.tar.gz`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.0/repodna-1.1.0-x86_64-apple-darwin.tar.gz) | Command line for macOS, Intel | 11.0 MB |
| [`repodna-1.1.0-x86_64-pc-windows-msvc.zip`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.0/repodna-1.1.0-x86_64-pc-windows-msvc.zip) | Command line for Windows, x86_64 | 11.0 MB |
| [`RepoDNA_1.1.0_amd64.deb`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.0/RepoDNA_1.1.0_amd64.deb) | Desktop app for Debian and Ubuntu | 12.0 MB |
| [`RepoDNA-1.1.0-1.x86_64.rpm`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.0/RepoDNA-1.1.0-1.x86_64.rpm) | Desktop app for Fedora and openSUSE | 12.0 MB |
| [`RepoDNA_1.1.0_universal.dmg`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.0/RepoDNA_1.1.0_universal.dmg) | Desktop app for macOS (Apple silicon and Intel) | 21.2 MB |
| [`RepoDNA_1.1.0_x64_en-US.msi`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.0/RepoDNA_1.1.0_x64_en-US.msi) | Desktop app for Windows (MSI installer) | 11.5 MB |
| [`RepoDNA_1.1.0_x64-setup.exe`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.0/RepoDNA_1.1.0_x64-setup.exe) | Desktop app for Windows (setup program) | 7.6 MB |
| [`SHA256SUMS.txt`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.0/SHA256SUMS.txt) | SHA-256 checksums of every file | 1.0 kB |

Also published for this version:

- `ghcr.io/sanskarin/repodna:1.1.0`: the command line with Git, for `linux/amd64` and `linux/arm64`.

## Install this version

```sh
# Linux, x86_64
curl -LO https://github.com/sanskarIN/RepoDNA/releases/download/v1.1.0/repodna-1.1.0-x86_64-unknown-linux-musl.tar.gz
tar -xzf repodna-1.1.0-x86_64-unknown-linux-musl.tar.gz
sudo install -m 0755 repodna-1.1.0-x86_64-unknown-linux-musl/repodna /usr/local/bin/repodna

# From source, with a Rust toolchain
cargo install --git https://github.com/sanskarIN/RepoDNA --tag v1.1.0 --locked repodna-cli

# In a container
docker run --rm -v "$PWD:/work" ghcr.io/sanskarin/repodna:1.1.0 analyze .
```

The [installation guide at v1.1.0](https://github.com/sanskarIN/RepoDNA/blob/v1.1.0/docs/installation.md) covers every
platform, checking the downloads against `SHA256SUMS.txt`, and opening the unsigned
binaries on macOS and Windows.

## Screenshots

<a href="../../images/v1.1.0/desktop/overview-light.png"><img src="../../images/v1.1.0/desktop/overview-light.png" alt="The overview in RepoDNA 1.1.0" width="400"></a> <a href="../../images/v1.1.0/desktop/architecture-dark.png"><img src="../../images/v1.1.0/desktop/architecture-dark.png" alt="The module map in RepoDNA 1.1.0" width="400"></a>

Twelve desktop views and the phone screens of this version, showing its own analysis of
this repository at `v1.1.0`, are in [`docs/images/v1.1.0`](../../images/README.md#110).

[All releases](../README.md)
