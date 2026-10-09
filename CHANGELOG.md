# Changelog

All notable changes to RepoDNA are documented in this file. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and RepoDNA follows
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [1.3.0] - 2026-10-09

A release about using the web interface. The analysis you had open survives a reload, the
last files you opened wait on the start page, and links lead from each view to the next
and to each of its panels. The web version can be installed as an app, opens files
dropped anywhere on the page, saves any table as CSV, and prints cleanly. A new npm
package, `@sanskarin/repodna-web`, runs it on your own machine.

### Added

- Recent analyses: the web interface keeps the last five analysis files you open in this
  browser, never uploading them, lists them on the start page and in search, and opens
  them again in one click. Remove them one by one or all at once, or turn this off, in
  Settings.
- Reloading the page opens the same analysis on the same view again, in the web version
  too. A new tab starts on the start page.
- Ways through an analysis: the numbers at the top of the overview open the view about
  them; a view with four panels or more lists them under its title; links under each view
  lead to the previous and the next one, as do the `[` and `]` keys; long pages get a
  **Back to top** button; and the sidebar shows how many findings the analysis has.
- Findings are listed 25 at a time, with buttons for more, and open filtered from the
  address, such as `#/findings?severity=warning`.
- **Download CSV** under every table saves all of its rows, in the order shown, for a
  spreadsheet: numbers stay numbers, and text that a spreadsheet would run as a formula is
  written so that it does not run. The desktop app asks where to save the file.
- An analysis file dropped anywhere on the page opens, also while another one is open. A
  file dropped on the new drop zone of the Compare view is compared with the open one.
- The web version can be installed as an app in browsers that offer it, with a window and
  an icon of its own. Installed in Chrome or Edge on a computer, it can open `.repodna`
  files from the file manager. The browser's bars around the page take its color, in the
  theme chosen in the header.
- The start page of `repodna serve` and the desktop app lists the newest eight stored
  analyses, with a button for the rest and a filter by name or location.
- The `@sanskarin/repodna-web` npm package, on npmjs.com and GitHub Packages: the web
  interface, built, with a small server. `npx @sanskarin/repodna-web` runs it at
  <http://127.0.0.1:8080/> to open analysis files and the demo offline, and its `root`
  export is the directory to serve from your own web server.

### Changed

- A printed view, or one saved as PDF, leaves out the buttons and links made for the
  screen, shows every column of wide tables, and keeps headings with what follows them.
- In the desktop app, **Download the analysis as loaded** opens a save dialog, as the
  report buttons do, instead of saving the file in a default folder without saying where.
- The npm packages are published with the command line first: the platform packages, the
  launcher, and then the libraries and the web interface, so that a package the registry
  refuses cannot keep the command line from being published.

### Fixed

- Printing in the dark theme gave pale text on white paper, since browsers leave out the
  dark background. Printouts are now always in the light theme.
- In the desktop app, the start page's drop zone did nothing: the window took every
  dropped file before the page could see it.
- A file dropped beside the start page's drop zone made the browser leave the page to
  show the file.
- Choosing the same analysis file again, after it failed to open or after closing it, did
  nothing.
- On a phone, a table's **Show all** button scrolled out of sight with the columns of a
  wide table.
- A table wider than the screen could only be scrolled sideways with a finger or a mouse
  when it held no buttons. It can now be reached with Tab and scrolled with the arrow
  keys.
- The two tables of **How to run the tests** had the same name for screen readers, and
  their CSV files the same file name.
- In the module map, a long module name was shortened in the middle of a word inside its
  node, which accessibility checks report as visible text missing from the node's name.
  The names are now drawn apart from the nodes, and are never covered by a node drawn
  later.
- The Compare view titled its file column "A file or demo" when it listed no demo.
- The page title named RepoDNA twice for RepoDNA's own analysis.

### Documentation

- The [web interface guide](docs/web.md) describes recent analyses, the ways through an
  analysis, CSV downloads, printing, dropping files, installing the web version, and the
  `@sanskarin/repodna-web` package; the [privacy policy](PRIVACY.md) says what the web
  interface keeps in the browser and how to remove it.
- A page for 1.2.2 in [`docs/releases`](docs/releases/README.md), with its downloads, and
  the 1.2.2 entry of this changelog dated on the day it was published.
- Screenshots of 1.3.0 in [`docs/images`](docs/images/README.md), with one of the start
  page and its recent analyses, and the promo images and Project DNA cards of 1.3.0 in
  [`docs/media/v1.3.0`](docs/media/README.md). The README and the guides show the 1.3.0
  screenshots.

## [1.2.2] - 2026-10-07

A small follow-up to 1.2.1: the shade on tables that scroll sideways on phones shows along
the whole edge, and the npm packages of older releases can be published to npmjs.com,
which put 1.2.0 there too. The notes of this release show its screenshots.

### Fixed

- On phones, the shade on the edge of a table that scrolls sideways is as strong at the
  top of the table as in its middle. In 1.2.1 it faded toward the top and bottom of long
  tables, so the first rows showed almost none.
- The npm packages workflow can publish a release older than the newest one, such as
  1.2.0 after 1.2.1. npm refused to give it the `latest` tag; it now goes under the
  `previous` tag, and `latest` stays on the newest release. 1.2.0 is on npmjs.com this
  way, so `npm install --global @sanskarin/repodna@1.2.0` works without a token too.

### Documentation

- Screenshots of 1.2.1 and 1.2.2 in [`docs/images`](docs/images/README.md), and the
  promo images and Project DNA cards of 1.2.1 in
  [`docs/media/v1.2.1`](docs/media/README.md). Each release has a folder of its own in
  the media kit, and the README and the guides show the screenshots of 1.2.2.
- The notes of a release on GitHub show its screenshots, when its tag has them.
- A page for 1.2.1 in [`docs/releases`](docs/releases/README.md), with its downloads and
  their sizes.
- The installation guide and the 1.2.0 release page say that 1.2.0 is on npmjs.com too.

## [1.2.1] - 2026-10-06

The npm packages on npmjs.com, so that `npm install --global @sanskarin/repodna` works
without a token, and a round of small fixes: the Project DNA card in `repodna serve`,
clearer errors when a clone fails, accessibility in the web interface and in reports, and
tables and long paths on phones.

### Added

- The npm packages are published to npmjs.com as well as GitHub Packages, so
  `npm install --global @sanskarin/repodna` works without a GitHub token. Releases publish
  them there, with provenance, when the repository has an `NPM_TOKEN` secret or trusted
  publishing; prereleases go under the `next` tag instead of `latest`.
- An npm packages workflow publishes the npm packages of a release that is already out,
  made from the archives attached to it and checked against its `SHA256SUMS.txt`.

### Changed

- The start page of the web version explains how a repository gets analyzed: in the
  desktop app, with `repodna serve`, or with `repodna analyze`, whose `repodna.json` opens
  on the page.
- On phones, tables that scroll sideways shade each edge that has more of the table
  beyond it.

### Fixed

- `repodna serve` shows the Project DNA card on the Reports page again. Its Content
  Security Policy did not allow the `blob:` image the page makes; the web interface image
  sends the same policy and was changed too.
- A failed clone is reported as `git clone failed` with Git's reason, instead of every
  option RepoDNA passes to Git, and says what to do when the repository is private or
  missing, or its host cannot be reached.
- `repodna ci` and the onboarding guide count recent changes "in the last 90 days of
  history": the window ends at the latest commit, which can be long before today.
- On phones, the web interface no longer lets the Copy button cover a long command; paths
  in tables wrap after their slashes and hyphens instead of every few letters, with the
  table scrolling sideways when it needs more room; and long paths in findings wrap on
  320-pixel screens instead of widening the page.
- Accessibility of the web interface: primary buttons reach 4.8:1 contrast; the top bar
  with search and the theme menu is a landmark; finding titles keep heading levels in
  order; command boxes and the output of commands that ran scroll with the keyboard; and
  each bar and column of a chart is named for screen readers.
- Accessibility of HTML reports: muted text reaches 4.7:1 contrast or more in light
  reports and on light Project DNA cards, and 5:1 in dark ones; finding titles keep
  heading levels in order; and wide tables and preformatted blocks scroll with the
  keyboard, each table in a region with a name of its own.
- The web version on GitHub Pages stays up when GitHub Pages deploys from a branch. Until
  now, every push to `main` let GitHub's own Pages build publish the repository's files
  over the interface, which then showed the README. The Web version workflow now
  publishes the interface again as soon as that build finishes, and warns on its runs
  until **Settings > Pages > Source** is set to **GitHub Actions**.

### Documentation

- A media kit in [`docs/media`](docs/media/README.md): screenshots of the web interface on
  a desktop and a phone, in light and dark, promo images for the 1.2.0 release, and
  RepoDNA's own Project DNA cards, sized for social networks.
- A folder for every release in [`docs/releases`](docs/releases/README.md), with its notes,
  its downloads and their sizes, and the commands that install it.
- Screenshots of the web interface in every release, 1.0.0 to 1.2.0, in
  [`docs/images`](docs/images/README.md): each shows that release's own analysis of this
  repository at its tag, and a gallery page compares them side by side. The screenshots
  of the media kit moved there too.
- The README and the guides show screenshots of 1.2.0, including the desktop app and the
  web interface on a phone, instead of screenshots taken with 1.0.0.
- The 1.1.1 entry of this changelog is dated on the day it was published.

## [1.2.0] - 2026-10-05

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

## [1.1.1] - 2026-10-05

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

## [1.1.0] - 2026-10-04

Optional AI explanations: prose about a repository, grounded in the analysis, checked
against its evidence, and off until you configure a provider.

### Explanations

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

### Privacy and safety

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

### Web interface and desktop app

- The start page, the settings, and the analysis details say that explanations are
  optional and need a provider you configure, and whether an analysis used remote AI.

## [1.0.0] - 2026-09-27

The first stable release: local-first repository intelligence and code archaeology, with
every conclusion backed by evidence.

### Analysis

- One analysis of a directory, a Git URL (cloned into a temporary directory), or an archive
  (`.zip`, `.tar`, `.tar.gz`, `.tgz`), with three profiles: `quick`, `standard`, and `deep`.
- Structure and languages: 65 built-in languages, 25 of them with lexical analysis of
  imports, symbols, and complexity; file classification that honors `.gitignore`,
  `.gitattributes` (`linguist-generated`, `linguist-vendored`), and your own rules; entry
  points and the directory tree.
- Architecture: modules from package manifests and directories, import resolution, the
  file and module dependency graphs, cycles, layers, centrality, and the inferred style
  with a confidence level.
- Dependencies: manifests and lockfiles for Cargo, npm, PyPI, Go, Maven and Gradle, NuGet,
  RubyGems, Composer, pub, and Swift Package Manager, read offline.
- History and code archaeology: commits, contributors with `.mailmap`, ownership, releases,
  quiet periods, file history across renames, recent changes, and ranked change hotspots.
- The Codebase Time Machine: snapshots rebuilt from Git without a checkout, epochs, events,
  the architecture at each snapshot (deep profile), and the project's story, with facts
  and interpretations kept apart.
- Quality signals: cyclomatic complexity, long and deeply nested functions, large files,
  duplicated and similar code, markers such as TODO and FIXME, and dead-code candidates.
- Security signals: 20 secret rules (values are never stored), 18 risky-pattern rules for
  code, configuration, CI workflows, and containers, and file permission checks.
- Tests, build, and documentation: test frameworks and files, build systems and commands,
  CI providers, documentation checks, and a getting-started guide assembled from the
  repository. Build and test commands run only when execution is enabled in the user
  configuration.
- Findings with evidence, method, limitations, and next steps, a severity (critical,
  warning, attention, info) and a confidence (high, medium, low), and suppressions that
  require a reason.
- The DNA fingerprint: eight descriptive dimensions and a DNA hash for each analyzed
  snapshot.
- The RepositoryDNA artifact, a versioned JSON document (schema 1.0) with published JSON
  Schemas, and reproducible output with `--reproducible` and `SOURCE_DATE_EPOCH`.

### Command line

- `repodna` with `analyze` (alias `scan`), `report`, `architecture`, `dependencies`,
  `history`, `hotspots`, `timeline`, `findings`, `show`, `compare`, `card`, `badge`,
  `onboarding`, `ci`, `export`, `import`, `list` (alias `ls`), `init`, `config`, `plugins`,
  `cache`, `clean`, `doctor`, `version`, `serve`, `schema`, and `completions`.
- `repodna ci` with `--fail-on`, baselines, `--new-only`, GitHub Actions annotations, and
  step summaries; documented exit codes.
- `repodna doctor` checks the installation, and `--export` writes a diagnostics bundle for
  bug reports that contains no source code.

### Reports and sharing

- Self-contained HTML reports in four themes, Markdown, JSON, and CSV tables, written one
  at a time or as a bundle.
- Project DNA cards (SVG and PNG, light and dark), README badges, onboarding guides, and
  side-by-side comparisons that describe differences without ranking.
- Portable `.repodna` exports and imports, and the privacy presets `local`, `share`, and
  `public`.

### Web interface and desktop app

- `repodna serve`: a local web interface on 127.0.0.1 with a session token, covering every
  part of the analysis, with search, a command palette, keyboard shortcuts, light and dark
  themes, a table view for every chart, and a bundled demo that works offline.
- A desktop app for Linux, macOS, and Windows, built with Tauri on the same Rust core.
- A container image with the command line and Git, `ghcr.io/sanskarin/repodna`, for
  `linux/amd64` and `linux/arm64`.
- About & support, Privacy Policy, Terms of Use, and Licenses pages in the web interface
  and the desktop app; every download includes the licenses of the third-party software it
  contains (`THIRD-PARTY-NOTICES.txt`).

### Extensibility

- Plugins: declarative language definitions and analyzers in any language that exchange
  JSON with RepoDNA, enabled only by the user; two example plugins.
- Configuration in `repodna.toml` and a user configuration file, validated strictly; a
  repository's own configuration cannot enable plugins or command execution.

### Safety and privacy

- No telemetry, no AI, and no network use except cloning a Git URL you give.
- A hardened Git runner, restricted clone URLs, safe archive extraction with limits,
  linear-time regular expressions, and local storage in SQLite that can be checked,
  repaired, and cleaned.

[Unreleased]: https://github.com/sanskarIN/RepoDNA/compare/v1.3.0...HEAD
[1.3.0]: https://github.com/sanskarIN/RepoDNA/compare/v1.2.2...v1.3.0
[1.2.2]: https://github.com/sanskarIN/RepoDNA/compare/v1.2.1...v1.2.2
[1.2.1]: https://github.com/sanskarIN/RepoDNA/compare/v1.2.0...v1.2.1
[1.2.0]: https://github.com/sanskarIN/RepoDNA/compare/v1.1.1...v1.2.0
[1.1.1]: https://github.com/sanskarIN/RepoDNA/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/sanskarIN/RepoDNA/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/sanskarIN/RepoDNA/releases/tag/v1.0.0
