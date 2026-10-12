# RepoDNA 1.3.2: release notes

The notes for the GitHub release **RepoDNA 1.3.2**, tag `v1.3.2`, made by the release
workflow from the changelog, the screenshots on the `images` branch, and the downloads.
Once it is published, the notes as published replace these, and a release page with
its downloads is added next to them.

---

A follow-up to 1.3.1. The documentation is now a site of its own, at
<https://sanskarin.github.io/RepoDNA/docs/>, with every guide, search, and the project's
pages; the support links lead to the new Buy Me a Coffee and Razorpay pages; and
`repodna-wasm` says plainly when a file or a folder it is given does not exist.

### Added

- The [documentation site](https://sanskarin.github.io/RepoDNA/docs/), published with the
  web version: every guide, the changelog, the roadmap, the project's policies, and the
  examples, benchmarks, and fixtures pages. It has a sidebar that follows the
  documentation index, search over every section, the sections of each page, previous and
  next pages, copy buttons on code, and the light or dark theme chosen in the web version.
  It fits phones and prints cleanly. Links between the guides lead to their pages, and
  links to other files of the repository to them on GitHub.
- A Sponsor button on the repository's page on GitHub, with Buy Me a Coffee and Razorpay.

### Changed

- The support links lead to [buymeacoffee.com/SanskarCodes](https://buymeacoffee.com/SanskarCodes)
  and [razorpay.me/@SanskarCodes](https://razorpay.me/@SanskarCodes): on the About page of
  the web version and the desktop app, in the README, in SUPPORT.md, and in the FAQ.
- The Documentation link of the About page, the end of `repodna --help`, and the
  `repodna.toml` and `config.toml` files that RepoDNA writes lead to the documentation
  site.
- The web version's offline cache leaves the documentation published beside it to the
  network, so that offline a guide is never shown as the web version.

### Fixed

- `repodna-wasm` printed Node's own error for a file or a folder that does not exist
  (`ENOENT: no such file or directory, stat '/full/path'`), and read a folder given to
  `report` or `card` as an analysis file. It now says which file or folder does not
  exist, that `report` and `card` need an analysis file, and why it cannot write its
  output.
- The guide to the GitHub Action left the `security-only` profile out of the profiles its
  `profile` input takes.

### Documentation

- The [development guide](https://github.com/sanskarIN/RepoDNA/blob/v1.3.2/docs/development.md#the-documentation-site) explains how to
  build, preview, and check the documentation site, and the README and the documentation
  index link to it. CI and the Web version workflow fail on a broken link or anchor in the
  documentation.
- [Screenshots of 1.3.2](https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/README.md#132)
  on the `images` branch, the documentation site's among them, and the README and the
  guides show them.


## Screenshots

<a href="https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/v1.3.2/desktop/overview-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/images/screenshots/v1.3.2/desktop/overview-light.png" alt="The overview in RepoDNA 1.3.2" width="400"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/v1.3.2/desktop/architecture-dark.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/images/screenshots/v1.3.2/desktop/architecture-dark.png" alt="The module map in RepoDNA 1.3.2" width="400"></a>

<a href="https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/v1.3.2/docs-site/index-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/images/screenshots/v1.3.2/docs-site/index-light.png" alt="The documentation site of RepoDNA 1.3.2" width="400"></a>

<a href="https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/v1.3.2/phone/overview-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/images/screenshots/v1.3.2/phone/overview-light.png" alt="The overview on a phone, in the light theme" width="190"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/v1.3.2/phone/menu-dark.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/images/screenshots/v1.3.2/phone/menu-dark.png" alt="The navigation menu on a phone, in the dark theme" width="190"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/v1.3.2/phone/history-dark.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/images/screenshots/v1.3.2/phone/history-dark.png" alt="History on a phone, in the dark theme" width="190"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/v1.3.2/phone/hotspots-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/images/screenshots/v1.3.2/phone/hotspots-light.png" alt="Hotspots on a phone, in the light theme" width="190"></a>

Every screenshot of this release is in [screenshots/v1.3.2](https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/README.md#132).

## Downloads

- `repodna-1.3.2-<target>`: the `repodna` command line, with the web interface of
  `repodna serve` built in. The Linux builds are static and run on any distribution.
- `repodna-1.3.2-wasm32-wasip1.zip`: RepoDNA's analysis, reports, and cards as
  WebAssembly, the program the web version runs, for WASI runtimes:
  `wasmtime run --dir .::/my-project repodna.wasm analyze /my-project` (without Git
  history).
- `RepoDNA_1.3.2_amd64.deb`, `RepoDNA-1.3.2-1.x86_64.rpm`,
  `RepoDNA_1.3.2_universal.dmg`, `RepoDNA_1.3.2_x64_en-US.msi`,
  `RepoDNA_1.3.2_x64-setup.exe`, and for Windows on Arm
  `RepoDNA_1.3.2_arm64_en-US.msi` and `RepoDNA_1.3.2_arm64-setup.exe`: the
  desktop app.
- `ghcr.io/sanskarin/repodna:1.3.2`: a container image with the command line and
  Git, for CI jobs: `docker run --rm -v "$PWD:/work" ghcr.io/sanskarin/repodna analyze .`
- `ghcr.io/sanskarin/repodna-web:1.3.2`: the web interface, served on port 8080, for
  your own network: `docker run --rm -p 8080:8080 ghcr.io/sanskarin/repodna-web`
- npm packages on npmjs.com and GitHub Packages: `npm install --global @sanskarin/repodna`
  installs the command line, with one package per platform;
  `npx @sanskarin/repodna-web` runs the web interface on this machine;
  `npx @sanskarin/repodna-wasm analyze .` analyzes with the WebAssembly program, on any
  system Node.js runs on, and runs it in browsers too;
  `@sanskarin/repodna-schema` and `@sanskarin/repodna-visualization` are the TypeScript
  libraries.
- `SHA256SUMS.txt`: checksums of every file.
- Web version: https://sanskarin.github.io/RepoDNA/ — analyze a folder or an archive, or open analyses and the demo, in your browser: nothing to install, nothing uploaded.

The binaries are not code-signed, so macOS and Windows ask for confirmation the first time they start; see the [installation guide](https://github.com/sanskarIN/RepoDNA/blob/v1.3.2/docs/installation.md).

More open-source projects by Sanskar, the creator of RepoDNA: https://sanskarin.github.io
