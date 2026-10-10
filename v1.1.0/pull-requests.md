# RepoDNA 1.1.0: pull requests

The pull requests merged into `main` for RepoDNA 1.1.0, released on 2026-10-04, after the release before it on 2026-09-27.

| Pull request | Branch | Merged |
|---|---|---|
| [#13 build(deps): bump the npm group with 4 updates](https://github.com/sanskarIN/RepoDNA/pull/13) | `dependabot/npm_and_yarn/npm-c38cc3e0dd` | 2026-10-03 |
| [#14 build(deps): bump the desktop group in /apps/desktop/src-tauri with 4 updates](https://github.com/sanskarIN/RepoDNA/pull/14) | `dependabot/cargo/apps/desktop/src-tauri/desktop-07b7d523f5` | 2026-10-03 |
| [#15 Release v1.1.0](https://github.com/sanskarIN/RepoDNA/pull/15) | `release-v1.1.0` | 2026-10-04 |

## #15 Release v1.1.0

[https://github.com/sanskarIN/RepoDNA/pull/15](https://github.com/sanskarIN/RepoDNA/pull/15) · branch `release-v1.1.0`

Optional AI explanations: prose about a repository, grounded in the analysis, checked
against its evidence, and off until you configure a provider.

#### Explanations

- `repodna explain` explains a repository (`--about repository`, `architecture`,
  `history`, `dependencies`, or `onboarding`), one module (`--module`), one hotspot
  (`--hotspot`), or answers a question (`--ask`), as text, Markdown, or JSON with its
  provenance: provider, model, revision, cited evidence, and token use.
- The model receives a numbered selection of evidence from the analysis, never the
  repository, and its answer is checked against it: citations of evidence that was not
  sent are removed, statements without a valid citation are labeled as not supported, and
  statements the model marks as inferences are labeled as such.
- Three providers: a local program that reads the prompt on standard input (for example
  `ollama run <model>`), any OpenAI-compatible server (Ollama, llama.cpp, LM Studio, vLLM,
  or a hosted service), and the Anthropic API.
- `--dry-run` shows the exact prompt, the evidence, where it would be sent, and the
  highest cost from prices you set, without contacting anything.
- Answers are cached on this machine; `--fresh` asks again and `repodna cache clear`
  removes them. When a model declines, RepoDNA reports the reason the provider gives.
- `repodna doctor` reports the configured provider without contacting it.

#### Privacy and safety

- AI is set only in your user configuration: a repository's own configuration cannot
  enable it or allow remote providers.
- A provider whose endpoint is not on this machine is refused unless you allow remote AI
  with `privacy.remote_ai = true` or, for one run, `--allow-remote-ai`.
- API keys are read from an environment variable you name and never stored. Without one,
  `ANTHROPIC_API_KEY` is sent only to Anthropic's own endpoint.
- No source code is sent unless you enable `ai.include_source_excerpts`: excerpts are a
  few lines per file, have likely secrets redacted, and are never read through a link
  that leads out of the checkout. Contributor names and email addresses are never sent.
- The model gets no tools: it cannot run commands, read files, or use the network through
  RepoDNA. Repository text is sent as data, and answers are cleaned of terminal control
  sequences and escaped in Markdown.
- RepoDNA uses the network only to clone a Git URL you give and to reach an AI provider
  you configure.

#### Web interface and desktop app

- The start page, the settings, and the analysis details say that explanations are
  optional and need a provider you configure, and whether an analysis used remote AI.

### Downloads

- `repodna-1.1.0-<target>`: the `repodna` command line, with the web interface of
  `repodna serve` built in. The Linux builds are static and run on any distribution.
- `RepoDNA_1.1.0_amd64.deb`, `RepoDNA-1.1.0-1.x86_64.rpm`,
  `RepoDNA_1.1.0_universal.dmg`, `RepoDNA_1.1.0_x64_en-US.msi`, and
  `RepoDNA_1.1.0_x64-setup.exe`: the desktop app.
- `ghcr.io/sanskarin/repodna:1.1.0`: a container image with the command line and
  Git, for CI jobs: `docker run --rm -v "$PWD:/work" ghcr.io/sanskarin/repodna analyze .`
- `SHA256SUMS.txt`: checksums of every file.
- Web version: https://sanskarin.github.io/RepoDNA/ — open analyses and the demo in your browser, nothing to install.

The binaries are not code-signed, so macOS and Windows ask for confirmation the first time they start; see the [installation guide](https://github.com/sanskarIN/RepoDNA/blob/v1.1.0/docs/installation.md).
