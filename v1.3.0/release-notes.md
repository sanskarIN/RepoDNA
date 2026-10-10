# RepoDNA 1.3.0: release notes

The notes of the GitHub release [RepoDNA 1.3.0](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.3.0), tag
`v1.3.0`, as published on 2026-10-09. The [release page](README.md) adds its
downloads with their sizes.

---

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

- The [web interface guide](https://github.com/sanskarIN/RepoDNA/blob/v1.3.0/docs/web.md) describes recent analyses, the ways through an
  analysis, CSV downloads, printing, dropping files, installing the web version, and the
  `@sanskarin/repodna-web` package; the [privacy policy](https://github.com/sanskarIN/RepoDNA/blob/v1.3.0/PRIVACY.md) says what the web
  interface keeps in the browser and how to remove it.
- A page for 1.2.2 in [`docs/releases`](https://github.com/sanskarIN/RepoDNA/blob/v1.3.0/docs/releases/README.md), with its downloads, and
  the 1.2.2 entry of this changelog dated on the day it was published.
- Screenshots of 1.3.0 in [`docs/images`](https://github.com/sanskarIN/RepoDNA/blob/v1.3.0/docs/images/README.md), with one of the start
  page and its recent analyses, and the promo images and Project DNA cards of 1.3.0 in
  [`docs/media/v1.3.0`](https://github.com/sanskarIN/RepoDNA/blob/v1.3.0/docs/media/README.md). The README and the guides show the 1.3.0
  screenshots.

## Screenshots

<a href="https://github.com/sanskarIN/RepoDNA/blob/v1.3.0/docs/media/v1.3.0/promo/launch-16x9.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/v1.3.0/docs/media/v1.3.0/promo/launch-16x9.png" alt="RepoDNA 1.3.0 on a desktop and a phone" width="800"></a>

<a href="https://github.com/sanskarIN/RepoDNA/blob/v1.3.0/docs/images/v1.3.0/desktop/overview-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/v1.3.0/docs/images/v1.3.0/desktop/overview-light.png" alt="The overview in RepoDNA 1.3.0" width="400"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/v1.3.0/docs/images/v1.3.0/desktop/architecture-dark.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/v1.3.0/docs/images/v1.3.0/desktop/architecture-dark.png" alt="The module map in RepoDNA 1.3.0" width="400"></a>

<a href="https://github.com/sanskarIN/RepoDNA/blob/v1.3.0/docs/images/v1.3.0/phone/overview-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/v1.3.0/docs/images/v1.3.0/phone/overview-light.png" alt="The overview on a phone, in the light theme" width="190"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/v1.3.0/docs/images/v1.3.0/phone/menu-dark.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/v1.3.0/docs/images/v1.3.0/phone/menu-dark.png" alt="The navigation menu on a phone, in the dark theme" width="190"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/v1.3.0/docs/images/v1.3.0/phone/history-dark.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/v1.3.0/docs/images/v1.3.0/phone/history-dark.png" alt="History on a phone, in the dark theme" width="190"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/v1.3.0/docs/images/v1.3.0/phone/hotspots-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/v1.3.0/docs/images/v1.3.0/phone/hotspots-light.png" alt="Hotspots on a phone, in the light theme" width="190"></a>

Every screenshot of this release is in [docs/images/v1.3.0](https://github.com/sanskarIN/RepoDNA/blob/v1.3.0/docs/images/README.md#130), and its promo images and Project DNA cards are in the [media kit](https://github.com/sanskarIN/RepoDNA/blob/v1.3.0/docs/media/README.md).

## Downloads

- `repodna-1.3.0-<target>`: the `repodna` command line, with the web interface of
  `repodna serve` built in. The Linux builds are static and run on any distribution.
- `RepoDNA_1.3.0_amd64.deb`, `RepoDNA-1.3.0-1.x86_64.rpm`,
  `RepoDNA_1.3.0_universal.dmg`, `RepoDNA_1.3.0_x64_en-US.msi`, and
  `RepoDNA_1.3.0_x64-setup.exe`: the desktop app.
- `ghcr.io/sanskarin/repodna:1.3.0`: a container image with the command line and
  Git, for CI jobs: `docker run --rm -v "$PWD:/work" ghcr.io/sanskarin/repodna analyze .`
- `ghcr.io/sanskarin/repodna-web:1.3.0`: the web interface, served on port 8080, for
  your own network: `docker run --rm -p 8080:8080 ghcr.io/sanskarin/repodna-web`
- npm packages on npmjs.com and GitHub Packages: `npm install --global @sanskarin/repodna`
  installs the command line, with one package per platform;
  `npx @sanskarin/repodna-web` runs the web interface on this machine;
  `@sanskarin/repodna-schema` and `@sanskarin/repodna-visualization` are the TypeScript
  libraries.
- `SHA256SUMS.txt`: checksums of every file.
- Web version: https://sanskarin.github.io/RepoDNA/ — open analyses and the demo in your browser, nothing to install.

The binaries are not code-signed, so macOS and Windows ask for confirmation the first time they start; see the [installation guide](https://github.com/sanskarIN/RepoDNA/blob/v1.3.0/docs/installation.md).

More open-source projects by Sanskar, the creator of RepoDNA: https://sanskarin.github.io
