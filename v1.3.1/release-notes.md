# RepoDNA 1.3.1: release notes

The notes for the GitHub release **RepoDNA 1.3.1**, tag `v1.3.1`, made by the release
workflow from the changelog, the screenshots on the `images` branch, and the downloads.
Once it is published, the notes as published replace these, and a release page with
its downloads is added next to them.

---

A release about the web version, which now analyzes repositories itself. Choose or drop a
folder or a `.zip` or `.tar.gz` archive, and RepoDNA analyzes it right in your browser,
with its analysis built as WebAssembly: nothing to install and nothing uploaded, though
without Git history. The web version also makes reports and Project DNA cards, and keeps
working offline once opened. The same program is on npm as `@sanskarin/repodna-wasm`,
RepoDNA has an official GitHub Action, the command line and the desktop app run on
Windows on Arm, and the desktop app saves reports and cards better.

### Added

- Analysis in the web version: **Analyze a repository in this browser** on the start page
  takes a folder, or a `.zip`, `.tar`, `.tar.gz`, or `.tgz` archive, and a profile, and a
  folder or an archive dropped anywhere on the page is analyzed too. The start page shows
  the stages as they run, the files done, and the time taken, with **Stop**. The analysis
  opens like an analysis file and is kept with the recent analyses. Files are read in the
  page only when the analysis needs them, so files that `.gitignore` excludes are never
  read; the repository's own `repodna.toml` applies, and Git history, which browsers
  cannot read, is left out.
- In the web version, **Reports & export** makes the HTML and Markdown reports of an open
  analysis, with a theme and a privacy preset, and shows its Project DNA card, with
  downloads as SVG and PNG, by the same code as `repodna report` and `repodna card`.
- The web version keeps working without a connection once opened: a service worker keeps
  the page, its files, and the demo, and the analysis program once it has been used. It
  keeps no analyses, and `repodna serve` and the desktop app do not use it.
- `@sanskarin/repodna-wasm`, on npmjs.com and GitHub Packages: RepoDNA's analysis, reports,
  and cards as WebAssembly, for Node.js and browsers on any system.
  `npx @sanskarin/repodna-wasm analyze .` analyzes a folder or an archive without a native
  program, and its JavaScript API runs the program on files in a browser.
- `repodna-1.3.1-wasm32-wasip1.zip` with each release: the same program for WASI
  runtimes such as wasmtime.
- An official [GitHub Action](https://github.com/sanskarIN/RepoDNA/blob/v1.3.1/docs/github-action.md): `uses: sanskarIN/RepoDNA@v1.3.1`
  installs the release on any runner, checked against its checksums, analyzes the
  repository once, adds the summary to the run's page and the findings as annotations,
  keeps the analysis and, when asked, the full report, and fails at the severity you
  choose, against a baseline if you give one.
- Windows on Arm: `repodna-1.3.1-aarch64-pc-windows-msvc.zip`, the desktop installers
  `RepoDNA_1.3.1_arm64_en-US.msi` and `RepoDNA_1.3.1_arm64-setup.exe`, and the npm
  package `@sanskarin/repodna-win32-arm64`, which `@sanskarin/repodna` installs there.
- The desktop app saves the Project DNA card from its own panel, as SVG or PNG, light or
  dark as its preview shows it.
- **Print this view, or save it as PDF** in the command palette (Ctrl/Cmd + K), for the
  desktop app, which has no print menu, and wherever Ctrl+P opens files instead.

### Changed

- The desktop app names saved reports after the analysis, such as
  `repodna-widget-2026-10-10.html`, instead of `repodna-report.html`, and the report folder
  `repodna-widget-2026-10-10-report`.
- The menus that choose an analysis profile name the profiles, and the chosen profile's
  description shows under them.
- With one thread, discovery walks the repository without starting a parallel walk.
- Analyses made with WebAssembly record their platform as `wasi` and `wasm`.
- The security policy of the web interface, in its page, `repodna serve`, the web image,
  and `@sanskarin/repodna-web`, allows compiling WebAssembly (`'wasm-unsafe-eval'`), and
  the web image and the npm server send `.wasm` files with their type.

### Fixed

- In the desktop app, saving the full report folder replaced the report of another
  repository saved in the same folder before. Each now gets a folder of its own, and a
  second one of the same analysis gets `-2`.
- In the desktop app, **Save the DNA card** saved a light card unless the dark report
  theme was chosen, whatever the card's preview showed.
- The desktop app's report buttons read **Save markdown report** and **Save jSON
  artifact**.
- The menus that choose an analysis profile were cut off in the desktop app and in narrow
  windows.

### Documentation

- The [web interface guide](https://github.com/sanskarIN/RepoDNA/blob/v1.3.1/docs/web.md) describes analyzing in the browser, reports and
  cards there, offline use, printing, and building and serving the web version with its
  analysis; a code block that ran into the next sentence is fixed.
- A [guide to the GitHub Action](https://github.com/sanskarIN/RepoDNA/blob/v1.3.1/docs/github-action.md), and the
  [installation guide](https://github.com/sanskarIN/RepoDNA/blob/v1.3.1/docs/installation.md) covers Windows on Arm, WebAssembly, and the
  new packages.
- The [Privacy Policy](https://github.com/sanskarIN/RepoDNA/blob/v1.3.1/PRIVACY.md) and the [privacy guide](https://github.com/sanskarIN/RepoDNA/blob/v1.3.1/docs/privacy.md) describe the
  web version's analysis and the files it keeps to work offline.
- The screenshots, promo images, and Project DNA cards of every release moved from
  `docs/images` and `docs/media` to the [`images`](https://github.com/sanskarIN/RepoDNA/tree/images) branch, and the
  release pages from `docs/releases` to the [`releases-info`](https://github.com/sanskarIN/RepoDNA/tree/releases-info)
  branch, which also keeps each release's notes as published and its pull requests. The
  code is about 42 MB smaller to download and check out. The README, the guides, and the
  release notes show the images from the `images` branch, and the release steps add each
  version's images and pages to these branches.
- A page for 1.3.0 on the [`releases-info`](https://github.com/sanskarIN/RepoDNA/tree/releases-info) branch, with its
  downloads.
- The README, the architecture and development guides, and the roadmap describe the
  WebAssembly analysis, the new packages, and the Action.
- [Screenshots of 1.3.1](https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/README.md#131) on a desktop, a
  phone, and in the desktop app, now also of the web version analyzing in the browser,
  and the README and the guides show them; the
  [media kit](https://github.com/sanskarIN/RepoDNA/blob/images/media/README.md) has 1.3.1 promo images and Project DNA
  cards.


## Screenshots

<a href="https://github.com/sanskarIN/RepoDNA/blob/images/media/v1.3.1/promo/launch-16x9.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/images/media/v1.3.1/promo/launch-16x9.png" alt="RepoDNA 1.3.1 on a desktop and a phone" width="800"></a>

<a href="https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/v1.3.1/desktop/overview-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/images/screenshots/v1.3.1/desktop/overview-light.png" alt="The overview in RepoDNA 1.3.1" width="400"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/v1.3.1/desktop/architecture-dark.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/images/screenshots/v1.3.1/desktop/architecture-dark.png" alt="The module map in RepoDNA 1.3.1" width="400"></a>

<a href="https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/v1.3.1/phone/overview-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/images/screenshots/v1.3.1/phone/overview-light.png" alt="The overview on a phone, in the light theme" width="190"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/v1.3.1/phone/menu-dark.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/images/screenshots/v1.3.1/phone/menu-dark.png" alt="The navigation menu on a phone, in the dark theme" width="190"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/v1.3.1/phone/history-dark.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/images/screenshots/v1.3.1/phone/history-dark.png" alt="History on a phone, in the dark theme" width="190"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/v1.3.1/phone/hotspots-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/images/screenshots/v1.3.1/phone/hotspots-light.png" alt="Hotspots on a phone, in the light theme" width="190"></a>

Every screenshot of this release is in [screenshots/v1.3.1](https://github.com/sanskarIN/RepoDNA/blob/images/screenshots/README.md#131), and its promo images and Project DNA cards are in the [media kit](https://github.com/sanskarIN/RepoDNA/blob/images/media/README.md).

## Downloads

- `repodna-1.3.1-<target>`: the `repodna` command line, with the web interface of
  `repodna serve` built in. The Linux builds are static and run on any distribution.
- `repodna-1.3.1-wasm32-wasip1.zip`: RepoDNA's analysis, reports, and cards as
  WebAssembly, the program the web version runs, for WASI runtimes:
  `wasmtime run --dir .::/my-project repodna.wasm analyze /my-project` (without Git
  history).
- `RepoDNA_1.3.1_amd64.deb`, `RepoDNA-1.3.1-1.x86_64.rpm`,
  `RepoDNA_1.3.1_universal.dmg`, `RepoDNA_1.3.1_x64_en-US.msi`,
  `RepoDNA_1.3.1_x64-setup.exe`, and for Windows on Arm
  `RepoDNA_1.3.1_arm64_en-US.msi` and `RepoDNA_1.3.1_arm64-setup.exe`: the
  desktop app.
- `ghcr.io/sanskarin/repodna:1.3.1`: a container image with the command line and
  Git, for CI jobs: `docker run --rm -v "$PWD:/work" ghcr.io/sanskarin/repodna analyze .`
- `ghcr.io/sanskarin/repodna-web:1.3.1`: the web interface, served on port 8080, for
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

The binaries are not code-signed, so macOS and Windows ask for confirmation the first time they start; see the [installation guide](https://github.com/sanskarIN/RepoDNA/blob/v1.3.1/docs/installation.md).

More open-source projects by Sanskar, the creator of RepoDNA: https://sanskarin.github.io
