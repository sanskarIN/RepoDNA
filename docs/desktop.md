# The desktop app

The desktop app is the RepoDNA web interface in a native window, built with
[Tauri](https://v2.tauri.app/). It talks to the same Rust core as the command line through
native calls instead of HTTP, so there is no server, no port, and no sign-in link.

![The desktop app's start page](images/desktop.png)

## Install

Installers are attached to each [release](https://github.com/sanskarIN/RepoDNA/releases):

| Platform | File |
|---|---|
| Debian, Ubuntu | `RepoDNA_<version>_amd64.deb` |
| Fedora, openSUSE | `RepoDNA-<version>-1.x86_64.rpm` |
| macOS (Apple silicon and Intel) | `RepoDNA_<version>_universal.dmg` |
| Windows | `RepoDNA_<version>_x64_en-US.msi` or `RepoDNA_<version>_x64-setup.exe` |

On Linux the app needs WebKitGTK 4.1 (`libwebkit2gtk-4.1-0`), which the packages declare
as a dependency. The Linux packages are built on Ubuntu 24.04.

The installers are not code-signed. macOS asks for confirmation the first time: open the
app from Finder with Control-click, **Open**. Windows SmartScreen may show "Windows
protected your PC": choose **More info**, then **Run anyway**. See
[installation](installation.md#unsigned-binaries).

To build it yourself, see [apps/desktop/README.md](../apps/desktop/README.md).

## Use

- **Analyze a repository**: choose a folder with the native folder picker, or enter an
  archive path or a Git URL, pick a profile, and start. Progress is shown while it runs.
- **Stored analyses** are the same ones the command line uses: an analysis made with
  `repodna analyze` appears in the app, and the other way around.
- **Open an analysis file** (`repodna.json` or `.repodna`) or **try the demo**.
- **Save reports**: the Reports view saves the HTML report, the Markdown report, the JSON
  artifact, the DNA card, or the full report folder through native save dialogs.
- Every view, search, and keyboard shortcut of the [web interface](web.md) works the same
  way.

## Privacy and security

- Everything stays on your machine; the app makes no network requests of its own. Cloning
  a Git URL uses your `git` installation, as the command line does.
- The page has no access to the file system or the shell: reading, analyzing, and saving
  happen in Rust, through a fixed set of commands. File dialogs are native.
- Links open in your default browser, and only `https://` links are opened.
- The window enforces a Content Security Policy that allows no remote content.

## Data

The app uses the same storage and user configuration as the command line (see
[privacy](privacy.md#what-is-stored-and-where)); `REPODNA_HOME` works for both. Settings
and plugins enabled in your user configuration apply to analyses started from the app.
