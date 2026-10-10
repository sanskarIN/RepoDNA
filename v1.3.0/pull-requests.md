# RepoDNA 1.3.0: pull requests

The pull requests merged into `main` for RepoDNA 1.3.0, released on 2026-10-09, after the release before it on 2026-10-07.

| Pull request | Branch | Merged |
|---|---|---|
| [#31 Add 'About me' link to README](https://github.com/sanskarIN/RepoDNA/pull/31) | `Add-about-link-in-README.md` | 2026-10-08 |
| [#32 Release v1.3.0](https://github.com/sanskarIN/RepoDNA/pull/32) | `release-v1.3.0` | 2026-10-09 |

## #32 Release v1.3.0

[https://github.com/sanskarIN/RepoDNA/pull/32](https://github.com/sanskarIN/RepoDNA/pull/32) · branch `release-v1.3.0`

### What this changes

Release of v1.3.0, a release about using the web interface. The full list is the 1.3.0
entry of `CHANGELOG.md`, which the release workflow turns into the release notes.

- **Recent analyses and reloads:** the web interface keeps the last five analysis files
  you open, in this browser only, and lists them on the start page and in search. A reload
  opens the same analysis on the same view. Both can be turned off and cleared in Settings.
- **Ways through an analysis:**
  - The overview's numbers open the view about them.
  - Views with four panels or more list them under their title.
  - Every view links to the previous and the next one, also with the `[` and `]` keys.
  - Long pages get **Back to top**.
  - The sidebar shows the findings count.
  - Findings load 25 at a time and can be opened filtered from the address.
- **Web version:**
  - **Download CSV** under every table.
  - A file dropped anywhere on the page opens.
  - It can be installed as an app, and when installed in Chrome or Edge it opens
    `.repodna` files from the file manager.
  - Printing is clean.
  - The start page of `repodna serve` and the desktop app lists the newest eight
    stored analyses, with a filter.
- **New npm package `@sanskarin/repodna-web`:** the built web interface with a small
  server. `npx @sanskarin/repodna-web` serves it at http://127.0.0.1:8080/. The release
  and npm workflows build the web interface and stage it with the other packages. The
  command-line packages are now published first, so a refused package can't block them.
- **Desktop app:**
  - A new `save_file` command saves CSV files and the loaded analysis through a save
    dialog. It only takes a file name from the page, never a folder.
  - The window no longer takes dropped files before the page sees them.
- **Bugs fixed:**
  - Dark-theme printouts were pale and hard to read.
  - The desktop app's drop zone did nothing.
  - Choosing the same file again did nothing.
  - On phones, **Show all** scrolled out of sight.
  - Wide tables couldn't be scrolled with the keyboard.
  - Two tables had the same screen-reader name.
  - Long module names were cut inside their nodes.
  - The page title said "RepoDNA" twice for RepoDNA's own analysis.
- **Release prep:**
  - Version 1.3.0 everywhere, plus the 1.3.0 changelog entry.
  - The 1.2.2 page in `docs/releases`, with 1.2.2 dated 2026-10-07, the day it was
    published.
  - `SECURITY.md` supports 1.3.x.
  - 23 screenshots, 3 promo images and 2 Project DNA cards of 1.3.0, used in the README
    and the guides.

The analysis is unchanged: the Rust crates only change version.

### Why

The web interface was hard to come back to and to move around in:
- A reload lost the open analysis.
- There was no way back to a file opened earlier.
- Long views could only be scrolled.
- Tables could only be read on screen.
- Dark-theme printouts were unreadable.
- The desktop app's drop zone was broken.

The new npm package gives a way to run the web interface offline without installing the
command line or Docker.

### How it was tested

- [CI](https://github.com/sanskarIN/RepoDNA/actions/runs/37928213662) passes on this branch.
- A [trial Release build](https://github.com/sanskarIN/RepoDNA/actions/runs/37928221105)
  of this branch passes and publishes nothing:
  - All 5 command-line targets.
  - The desktop app on Linux, macOS and Windows.
  - Both container images.
  - The npm packages, which are staged and checked, including that the new web package
    serves its page.
- Locally:
  - `cargo fmt`, `clippy` and `test` for the workspace and the desktop app. The desktop
    app has a new test for the file names its save dialog offers.
  - 113 web tests, 10 library tests, and 22 npm packaging tests, including new tests for
    the web package and its server.
- Every view was checked in a browser with an accessibility scan and no console errors.
  This covered the demo and all 16 test-fixture repositories, at desktop and phone sizes,
  in both themes.

Screenshots (all of them are in `docs/images/v1.3.0`):

<a href="https://github.com/sanskarIN/RepoDNA/blob/d0d359987948fa9b23d56ceef60ebf90a1be67ec/docs/images/v1.3.0/desktop/start-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/d0d359987948fa9b23d56ceef60ebf90a1be67ec/docs/images/v1.3.0/desktop/start-light.png" alt="The start page with recent analyses" width="400"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/d0d359987948fa9b23d56ceef60ebf90a1be67ec/docs/images/v1.3.0/desktop/overview-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/d0d359987948fa9b23d56ceef60ebf90a1be67ec/docs/images/v1.3.0/desktop/overview-light.png" alt="The overview in RepoDNA 1.3.0" width="400"></a>

<a href="https://github.com/sanskarIN/RepoDNA/blob/d0d359987948fa9b23d56ceef60ebf90a1be67ec/docs/images/v1.3.0/phone/overview-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/d0d359987948fa9b23d56ceef60ebf90a1be67ec/docs/images/v1.3.0/phone/overview-light.png" alt="The overview on a phone, in the light theme" width="190"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/d0d359987948fa9b23d56ceef60ebf90a1be67ec/docs/images/v1.3.0/phone/menu-dark.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/d0d359987948fa9b23d56ceef60ebf90a1be67ec/docs/images/v1.3.0/phone/menu-dark.png" alt="The navigation menu on a phone, in the dark theme" width="190"></a>
<a href="https://github.com/sanskarIN/RepoDNA/blob/d0d359987948fa9b23d56ceef60ebf90a1be67ec/docs/images/v1.3.0/phone/hotspots-light.png"><img src="https://raw.githubusercontent.com/sanskarIN/RepoDNA/d0d359987948fa9b23d56ceef60ebf90a1be67ec/docs/images/v1.3.0/phone/hotspots-light.png" alt="Hotspots on a phone, in the light theme" width="190"></a>

### Checklist

- [x] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked` pass
- [x] For web changes: `npm run format:check`, `npm run typecheck`, and `npm test` pass
- [x] New or changed findings carry evidence, a method, and limitations, and have tests with realistic samples (not applicable: no findings change)
- [x] No network access, telemetry, or execution of repository code was added to the analysis
- [x] Documentation in `docs/` is updated, and user-visible changes are listed in `CHANGELOG.md` (under 1.3.0, as this is the release)
