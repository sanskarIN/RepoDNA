# RepoDNA 1.3.1: pull requests

The pull requests merged into `main` for RepoDNA 1.3.1, released on 2026-10-10, after the release before it on 2026-10-09.

| Pull request | Branch | Merged |
|---|---|---|
| [#33 build(deps): bump skrifa from 0.47.0 to 0.48.0](https://github.com/sanskarIN/RepoDNA/pull/33) | `dependabot/cargo/skrifa-0.48.0` | 2026-10-10 |
| [#34 build(deps): bump the npm group with 7 updates](https://github.com/sanskarIN/RepoDNA/pull/34) | `dependabot/npm_and_yarn/npm-62a67576bc` | 2026-10-10 |
| [#35 build(deps): bump the desktop group in /apps/desktop/src-tauri with 2 updates](https://github.com/sanskarIN/RepoDNA/pull/35) | `dependabot/cargo/apps/desktop/src-tauri/desktop-8aabb39049` | 2026-10-10 |
| [#36 build(deps): bump skrifa from 0.47.0 to 0.48.0 in /apps/desktop/src-tauri](https://github.com/sanskarIN/RepoDNA/pull/36) | `dependabot/cargo/apps/desktop/src-tauri/skrifa-0.48.0` | 2026-10-10 |
| [#37 Release v1.3.1](https://github.com/sanskarIN/RepoDNA/pull/37) | `release-v1.3.1` | 2026-10-10 |

## #37 Release v1.3.1

[https://github.com/sanskarIN/RepoDNA/pull/37](https://github.com/sanskarIN/RepoDNA/pull/37) · branch `release-v1.3.1`

### What this changes

Prepares RepoDNA 1.3.1: version 1.3.1 everywhere, the changelog, docs, screenshots, and media. The main work is in the web version, which now analyzes repositories itself.

**Web version**
- **Analyze in the browser.** "Analyze a repository in this browser" on the start page takes a folder, or a `.zip`, `.tar`, `.tar.gz`, or `.tgz` archive, and a profile. A folder or an archive dropped anywhere on the page is analyzed too. The page shows the stages, the files done, and **Stop**.
  - RepoDNA's analysis runs as WebAssembly in a Web Worker. This is the new `repodna-wasm` crate, built with a size-tuned `web-engine` profile.
  - Files are read only when the analysis opens them, so files that `.gitignore` excludes are never read, and nothing is uploaded.
  - There is no Git history, because browsers can't run Git.
- **Reports and cards in the browser.** Reports & export makes the HTML and Markdown reports and the Project DNA card as SVG or PNG, with the same code as `repodna report` and `repodna card`.
- **Works offline.** A service worker keeps the page, its files, the demo, and the analysis program once used. It keeps no analyses, and it is turned off when a server or the desktop app is present.
- **Security policy.** The web interface's security policy now allows `'wasm-unsafe-eval'`. This applies to the page, `repodna serve`, the nginx image, and `@sanskarin/repodna-web`. The image and the npm server send `.wasm` files as `application/wasm`.
- **Print or save as PDF** is now in the command palette.

**Desktop app fixes**
- Saving the full report folder no longer replaces another repository's report saved in the same folder. Each analysis gets its own folder, with `-2`, `-3`, and so on for repeats.
- Saved files are named after the analysis, such as `repodna-widget-2026-10-10.html`.
- The Project DNA card is saved from its own panel, as SVG or PNG, light or dark as the preview shows. Before, "Save the DNA card" ignored the preview.
- The labels "Save markdown report" and "Save jSON artifact" are fixed.
- Profile menus were cut off; they now show short profile names, with the chosen profile's description under them.

**Packages and downloads**
- `@sanskarin/repodna-wasm` (new, on npmjs.com and GitHub Packages): the analysis, reports, and cards as WebAssembly, with a command line (`npx @sanskarin/repodna-wasm analyze .`) and a JavaScript API for Node.js and browsers. Its source is `packages/wasm`, published as `@repodna/wasm` in the workspace.
- `repodna-<version>-wasm32-wasip1.zip` for WASI runtimes such as wasmtime.
- Windows on Arm:
  - `aarch64-pc-windows-msvc` command line.
  - Desktop installers `RepoDNA_<version>_arm64_en-US.msi` and `-setup.exe`.
  - `@sanskarin/repodna-win32-arm64`, which the launcher picks on that platform.
- An official GitHub Action (`action.yml`, `packaging/action/install.sh`, `docs/github-action.md`):
  - Installs the release, checked against `SHA256SUMS.txt`, and analyzes once.
  - Writes the summary and annotations, and keeps the `.repodna` analysis and, if asked, the full report.
  - Fails at a chosen severity, optionally against a baseline.

**Engine**
- With one thread, discovery walks the repository without starting a parallel walk. The `ignore` crate's parallel walker isn't available on WASI.
- On WebAssembly, the platform is recorded as `wasi`/`wasm`, and the temp directory is `/tmp`.

**Build and CI**
- CI has new jobs for the WebAssembly build and for the Action on Linux, macOS, and Windows.
- The desktop jobs on Windows and macOS now build the web interface themselves, as release builds do.
- Fixed a Windows-only web build failure: the service worker step read `public/` from a URL pathname (`D:\D:\…`). It now uses a file path, which also fixes folders with spaces in their names.
- The release workflow builds and packages the WebAssembly program, builds the web version with it, and adds the Windows Arm builds.
- Third-party notices cover the new targets.

**Docs and images**
- Updated: the web, installation, privacy, desktop, architecture, and development guides, plus README, PRIVACY.md, ROADMAP.md, and the CI example.
- Added: a release page for 1.3.0, the 1.3.1 screenshots (`docs/images/v1.3.1`), and the 1.3.1 promo images and Project DNA cards (`docs/media/v1.3.1`).

### Why

- The web version could only open analyses made somewhere else. Now someone can analyze a repository from the browser alone, with nothing to install and nothing uploaded.
- The desktop app could overwrite reports and save a card that didn't match its preview.
- More people can use RepoDNA:
  - Windows on Arm users get native builds.
  - CI users get a one-line GitHub Action.
  - Node.js and browser users get a WebAssembly package that needs no native program.

### How it was tested

- **Rust workspace:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked` pass (45 suites, 607 tests). The new `repodna-wasm` tests and the discovery test `one_thread_walks_like_many` are among them.
- **Desktop app:** format, clippy (`--features custom-protocol`), and tests pass, including new tests for report folder and file names.
- **WebAssembly:** `cargo clippy -p repodna-wasm --target wasm32-wasip1` passes.
  - Its analysis of this repository matches the native one in files, languages, and modules, without the Git-based findings.
  - The `@repodna/wasm` tests run the real program, including a Devanagari folder name, a TAR archive, and error messages.
- **Web:** `npm run format:check`, `npm run typecheck`, and `npm test` pass (119 web tests plus the schema, visualization, and wasm package tests).
- **Packaging and notices:** `node --test "packaging/npm/test/*.test.mjs"` passes (24 tests), and `cargo xtask notices --check` is up to date.
- **Browser end-to-end (Chromium, Playwright):**
  - Folder and archive analysis, broken archives, and **Stop**.
  - Report and card downloads.
  - Offline reload with the service worker.
  - The simulated desktop bridge.
- **Real desktop app (Linux, Xvfb):** an analysis, Reports & export, and **Save PNG…** through the native dialog, which saved a 2400 × 1260 card.
- **CI** is green on the head commit, including the Action on all three operating systems and the web build on Windows and macOS.
- **Trial Release build** (no tag, nothing published): all 15 jobs pass. That covers every command line target including `aarch64-pc-windows-msvc`, every desktop installer including Windows arm64, the web image test, the container image, the npm packages, and the WebAssembly zip: https://github.com/sanskarIN/RepoDNA/actions/runs/38051411268
- **Screenshots of 1.3.1** are in `docs/images/v1.3.1` and `docs/media/v1.3.1`. The start page shows analysis in the browser:

![The web version's start page with Analyze a repository in this browser](https://raw.githubusercontent.com/sanskarIN/RepoDNA/release-v1.3.1/docs/images/v1.3.1/desktop/start-light.png)

### Checklist

- [x] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked` pass
- [x] For web changes: `npm run format:check`, `npm run typecheck`, and `npm test` pass
- [x] New or changed findings carry evidence, a method, and limitations, and have tests with realistic samples (no findings were added or changed)
- [x] No network access, telemetry, or execution of repository code was added to the analysis (the web version loads its analysis program from its own site; files are never uploaded)
- [x] Documentation in `docs/` is updated, and user-visible changes are listed in `CHANGELOG.md` (under `[1.3.1] - 2026-10-10`, since this is the release)
