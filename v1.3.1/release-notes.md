# RepoDNA 1.3.1: release notes

The notes prepared for the GitHub release **RepoDNA 1.3.1**, tag `v1.3.1`, which is not
published yet. Once it is, the notes as published replace these, and a release page with
its downloads is added next to them.

---

RepoDNA 1.3.1 is about the web version, which now analyzes repositories itself. Choose or drop a folder, or a `.zip` or `.tar.gz` archive, and RepoDNA analyzes it right in your browser, with its analysis built as WebAssembly: nothing to install and nothing uploaded, though without Git history. The web version also makes reports and Project DNA cards, and keeps working offline once opened. The same program is on npm as `@sanskarin/repodna-wasm`, RepoDNA has an official GitHub Action, the command line and the desktop app run on Windows on Arm, and the desktop app saves reports and cards better.

<img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/images/media/v1.3.1/promo/launch-16x9.png" alt="RepoDNA 1.3.1: analyze a repository right in your browser" width="800">

## Highlights

- **Analyze in your browser.** On the start page of the [web version](https://sanskarin.github.io/RepoDNA/), **Choose a folder…** or **Choose an archive…** (`.zip`, `.tar`, `.tar.gz`, `.tgz`), pick a profile, and watch each stage run, with **Stop**. A folder or archive dropped anywhere on the page works too. Files are read only when the analysis needs them, `.gitignore`d files are never read, and nothing is uploaded.
- **Reports and cards in the browser.** **Reports & export** makes the HTML and Markdown reports, with a theme and a privacy preset, and the Project DNA card as SVG or PNG, by the same code as `repodna report` and `repodna card`.
- **Works offline.** Once opened, the web version keeps its page, its files, the demo, and the analysis program for use without a connection.
- **An official GitHub Action.** `uses: sanskarIN/RepoDNA@v1.3.1` analyzes the repository on the runner, puts the summary on the run's page and the findings as annotations, keeps the analysis and the full report, and fails at the severity you choose, against a baseline if you give one. See [the guide](https://github.com/sanskarIN/RepoDNA/blob/v1.3.1/docs/github-action.md).
- **Windows on Arm.** The command line, the desktop app installers, and `@sanskarin/repodna-win32-arm64`, which `npm install --global @sanskarin/repodna` picks there.
- **RepoDNA as WebAssembly.** `npx @sanskarin/repodna-wasm analyze .` runs RepoDNA anywhere Node.js runs, with no native program, and `repodna-1.3.1-wasm32-wasip1.zip` runs in WASI runtimes such as wasmtime.

## Added

- Analysis in the web version, from the start page or by dropping a folder or an archive anywhere, with stages, files done, time taken, and **Stop**; the result opens like an analysis file and is kept with the recent analyses. The repository's own `repodna.toml` applies; Git history, which browsers cannot read, is left out.
- HTML and Markdown reports and the Project DNA card (SVG and PNG) on **Reports & export** in the web version.
- Offline use of the web version once opened, through a service worker that keeps no analyses; `repodna serve` and the desktop app do not use it.
- `@sanskarin/repodna-wasm` on npmjs.com and GitHub Packages, for Node.js and browsers on any system, with a command line and a JavaScript API.
- `repodna-1.3.1-wasm32-wasip1.zip` for WASI runtimes.
- The GitHub Action, installing the release checked against its `SHA256SUMS.txt`.
- Windows on Arm: `repodna-1.3.1-aarch64-pc-windows-msvc.zip`, `RepoDNA_1.3.1_arm64_en-US.msi`, `RepoDNA_1.3.1_arm64-setup.exe`, and `@sanskarin/repodna-win32-arm64`.
- The desktop app saves the Project DNA card from its own panel, as SVG or PNG, light or dark as its preview shows it.
- **Print this view, or save it as PDF** in the command palette (Ctrl/Cmd + K).

## Changed

- The desktop app names saved reports after the analysis, such as `repodna-widget-2026-10-10.html`, and the report folder `repodna-widget-2026-10-10-report`.
- The profile menus name the profiles, and the chosen profile's description shows under them.
- With one thread, discovery walks the repository without starting a parallel walk.
- Analyses made with WebAssembly record their platform as `wasi` and `wasm`.
- The web interface's security policy allows compiling WebAssembly (`'wasm-unsafe-eval'`), and the web image and the npm server send `.wasm` files with their type.

## Fixed

- In the desktop app, saving the full report folder replaced the report of another repository saved in the same folder before. Each now gets a folder of its own, and a second one of the same analysis gets `-2`.
- In the desktop app, **Save the DNA card** saved a light card unless the dark report theme was chosen, whatever the card's preview showed.
- The desktop app's report buttons read **Save markdown report** and **Save jSON artifact**.
- The profile menus were cut off in the desktop app and in narrow windows.

## Documentation

- The web interface guide covers analysis in the browser, reports and cards there, offline use, printing, and building the web version with its analysis.
- A guide to the GitHub Action; the installation guide covers Windows on Arm, WebAssembly, and the new packages.
- The Privacy Policy and the privacy guide describe the web version's analysis and the files it keeps to work offline.
- A page for 1.3.0 in `docs/releases`; 1.3.1 screenshots, promo images, and Project DNA cards.

## Install

```sh
npm install --global @sanskarin/repodna      # the command line
npx @sanskarin/repodna-wasm analyze .        # no native program
npx @sanskarin/repodna-web                   # the web interface, offline
```

Or download a file below: the command line for Linux, macOS, and Windows (x64 and Arm), the desktop app, and the WebAssembly program. Check downloads against `SHA256SUMS.txt`. Or use it in a workflow:

```yaml
- uses: actions/checkout@v7
  with:
    fetch-depth: 0
- uses: sanskarIN/RepoDNA@v1.3.1
  with:
    fail-on: warning
```

**Full changelog:** https://github.com/sanskarIN/RepoDNA/compare/v1.3.0...v1.3.1
