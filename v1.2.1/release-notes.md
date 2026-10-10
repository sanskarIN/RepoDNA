# RepoDNA 1.2.1: release notes

The notes of the GitHub release [RepoDNA 1.2.1](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.2.1), tag
`v1.2.1`, as published on 2026-10-06. The [release page](README.md) adds its
downloads with their sizes.

---

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

- A media kit in [`docs/media`](https://github.com/sanskarIN/RepoDNA/blob/v1.2.1/docs/media/README.md): screenshots of the web interface on
  a desktop and a phone, in light and dark, promo images for the 1.2.0 release, and
  RepoDNA's own Project DNA cards, sized for social networks.
- A folder for every release in [`docs/releases`](https://github.com/sanskarIN/RepoDNA/blob/v1.2.1/docs/releases/README.md), with its notes,
  its downloads and their sizes, and the commands that install it.
- Screenshots of the web interface in every release, 1.0.0 to 1.2.0, in
  [`docs/images`](https://github.com/sanskarIN/RepoDNA/blob/v1.2.1/docs/images/README.md): each shows that release's own analysis of this
  repository at its tag, and a gallery page compares them side by side. The screenshots
  of the media kit moved there too.
- The README and the guides show screenshots of 1.2.0, including the desktop app and the
  web interface on a phone, instead of screenshots taken with 1.0.0.
- The 1.1.1 entry of this changelog is dated on the day it was published.

## Downloads

- `repodna-1.2.1-<target>`: the `repodna` command line, with the web interface of
  `repodna serve` built in. The Linux builds are static and run on any distribution.
- `RepoDNA_1.2.1_amd64.deb`, `RepoDNA-1.2.1-1.x86_64.rpm`,
  `RepoDNA_1.2.1_universal.dmg`, `RepoDNA_1.2.1_x64_en-US.msi`, and
  `RepoDNA_1.2.1_x64-setup.exe`: the desktop app.
- `ghcr.io/sanskarin/repodna:1.2.1`: a container image with the command line and
  Git, for CI jobs: `docker run --rm -v "$PWD:/work" ghcr.io/sanskarin/repodna analyze .`
- `ghcr.io/sanskarin/repodna-web:1.2.1`: the web interface, served on port 8080, for
  your own network: `docker run --rm -p 8080:8080 ghcr.io/sanskarin/repodna-web`
- npm packages on npmjs.com and GitHub Packages: `npm install --global @sanskarin/repodna`
  installs the command line, with one package per platform;
  `@sanskarin/repodna-schema` and `@sanskarin/repodna-visualization` are the TypeScript
  libraries.
- `SHA256SUMS.txt`: checksums of every file.
- Web version: https://sanskarin.github.io/RepoDNA/ — open analyses and the demo in your browser, nothing to install.

The binaries are not code-signed, so macOS and Windows ask for confirmation the first time they start; see the [installation guide](https://github.com/sanskarIN/RepoDNA/blob/v1.2.1/docs/installation.md).

More open-source projects by Sanskar, the creator of RepoDNA: https://sanskarin.github.io
