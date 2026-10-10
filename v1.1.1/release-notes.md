# RepoDNA 1.1.1: release notes

The notes of the GitHub release [RepoDNA 1.1.1](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.1.1), tag
`v1.1.1`, as published on 2026-10-05. The [release page](README.md) adds its
downloads with their sizes.

---

A maintenance release: small fixes across the command line, reports, and the web
interface, and a link to more open-source projects by RepoDNA's creator.

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

- `repodna-1.1.1-<target>`: the `repodna` command line, with the web interface of
  `repodna serve` built in. The Linux builds are static and run on any distribution.
- `RepoDNA_1.1.1_amd64.deb`, `RepoDNA-1.1.1-1.x86_64.rpm`,
  `RepoDNA_1.1.1_universal.dmg`, `RepoDNA_1.1.1_x64_en-US.msi`, and
  `RepoDNA_1.1.1_x64-setup.exe`: the desktop app.
- `ghcr.io/sanskarin/repodna:1.1.1`: a container image with the command line and
  Git, for CI jobs: `docker run --rm -v "$PWD:/work" ghcr.io/sanskarin/repodna analyze .`
- `SHA256SUMS.txt`: checksums of every file.
- Web version: https://sanskarin.github.io/RepoDNA/ — open analyses and the demo in your browser, nothing to install.

The binaries are not code-signed, so macOS and Windows ask for confirmation the first time they start; see the [installation guide](https://github.com/sanskarIN/RepoDNA/blob/v1.1.1/docs/installation.md).

More open-source projects by Sanskar, the creator of RepoDNA: https://sanskarin.github.io
