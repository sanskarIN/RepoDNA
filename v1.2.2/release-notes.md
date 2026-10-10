# RepoDNA 1.2.2: release notes

The notes of the GitHub release [RepoDNA 1.2.2](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.2.2), tag
`v1.2.2`, as published on 2026-10-07. The [release page](README.md) adds its
downloads with their sizes.

---

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

- Screenshots of 1.2.1 and 1.2.2 in [`docs/images`](https://github.com/sanskarIN/RepoDNA/blob/v1.2.2/docs/images/README.md), and the
  promo images and Project DNA cards of 1.2.1 in
  [`docs/media/v1.2.1`](https://github.com/sanskarIN/RepoDNA/blob/v1.2.2/docs/media/README.md). Each release has a folder of its own in
  the media kit, and the README and the guides show the screenshots of 1.2.2.
- The notes of a release on GitHub show its screenshots, when its tag has them.
- A page for 1.2.1 in [`docs/releases`](https://github.com/sanskarIN/RepoDNA/blob/v1.2.2/docs/releases/README.md), with its downloads and
  their sizes.
- The installation guide and the 1.2.0 release page say that 1.2.0 is on npmjs.com too.

## Screenshots

<a href="https://github.com/sanskarIN/RepoDNA/blob/v1.2.2/docs/images/v1.2.2/desktop/overview-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/v1.2.2/docs/images/v1.2.2/desktop/overview-light.png" alt="The overview in RepoDNA 1.2.2" width="400"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/v1.2.2/docs/images/v1.2.2/desktop/architecture-dark.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/v1.2.2/docs/images/v1.2.2/desktop/architecture-dark.png" alt="The module map in RepoDNA 1.2.2" width="400"></a>

<a href="https://github.com/sanskarIN/RepoDNA/blob/v1.2.2/docs/images/v1.2.2/phone/overview-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/v1.2.2/docs/images/v1.2.2/phone/overview-light.png" alt="The overview on a phone, in the light theme" width="190"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/v1.2.2/docs/images/v1.2.2/phone/menu-dark.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/v1.2.2/docs/images/v1.2.2/phone/menu-dark.png" alt="The navigation menu on a phone, in the dark theme" width="190"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/v1.2.2/docs/images/v1.2.2/phone/history-dark.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/v1.2.2/docs/images/v1.2.2/phone/history-dark.png" alt="History on a phone, in the dark theme" width="190"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/v1.2.2/docs/images/v1.2.2/phone/hotspots-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/v1.2.2/docs/images/v1.2.2/phone/hotspots-light.png" alt="Hotspots on a phone, in the light theme" width="190"></a>

Every screenshot of this release is in [docs/images/v1.2.2](https://github.com/sanskarIN/RepoDNA/blob/v1.2.2/docs/images/README.md#122).

## Downloads

- `repodna-1.2.2-<target>`: the `repodna` command line, with the web interface of
  `repodna serve` built in. The Linux builds are static and run on any distribution.
- `RepoDNA_1.2.2_amd64.deb`, `RepoDNA-1.2.2-1.x86_64.rpm`,
  `RepoDNA_1.2.2_universal.dmg`, `RepoDNA_1.2.2_x64_en-US.msi`, and
  `RepoDNA_1.2.2_x64-setup.exe`: the desktop app.
- `ghcr.io/sanskarin/repodna:1.2.2`: a container image with the command line and
  Git, for CI jobs: `docker run --rm -v "$PWD:/work" ghcr.io/sanskarin/repodna analyze .`
- `ghcr.io/sanskarin/repodna-web:1.2.2`: the web interface, served on port 8080, for
  your own network: `docker run --rm -p 8080:8080 ghcr.io/sanskarin/repodna-web`
- npm packages on npmjs.com and GitHub Packages: `npm install --global @sanskarin/repodna`
  installs the command line, with one package per platform;
  `@sanskarin/repodna-schema` and `@sanskarin/repodna-visualization` are the TypeScript
  libraries.
- `SHA256SUMS.txt`: checksums of every file.
- Web version: https://sanskarin.github.io/RepoDNA/ — open analyses and the demo in your browser, nothing to install.

The binaries are not code-signed, so macOS and Windows ask for confirmation the first time they start; see the [installation guide](https://github.com/sanskarIN/RepoDNA/blob/v1.2.2/docs/installation.md).

More open-source projects by Sanskar, the creator of RepoDNA: https://sanskarin.github.io
