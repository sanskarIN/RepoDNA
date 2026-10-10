# Installation

RepoDNA is one command-line program, `repodna`, with the web interface built in. A desktop
app is also available. Everything runs on your machine. To analyze a repository or look at
analyses without installing anything, use the [web version](web.md#the-web-version), which
analyzes in your browser, without Git history. In GitHub Actions, use the
[GitHub Action](github-action.md).

- [Prebuilt binaries](#prebuilt-binaries)
- [The desktop app](#the-desktop-app)
- [Container image](#container-image)
- [Web interface image](#web-interface-image)
- [npm packages](#npm-packages)
- [WebAssembly](#webassembly)
- [GitHub Action](github-action.md)
- [Build from source](#build-from-source)
- [Check the installation](#check-the-installation)
- [Uninstall](#uninstall)

**Requirements.** Git on your `PATH` for history analysis (RepoDNA works without it, but
reports no history). Nothing else: the binaries have no runtime dependencies.

## Prebuilt binaries

Download the archive for your platform from the
[releases page](https://github.com/sanskarIN/RepoDNA/releases):

| Platform | Archive |
|---|---|
| Linux, x86_64 | `repodna-<version>-x86_64-unknown-linux-musl.tar.gz` |
| Linux, ARM64 | `repodna-<version>-aarch64-unknown-linux-musl.tar.gz` |
| macOS, Apple silicon | `repodna-<version>-aarch64-apple-darwin.tar.gz` |
| macOS, Intel | `repodna-<version>-x86_64-apple-darwin.tar.gz` |
| Windows, x86_64 | `repodna-<version>-x86_64-pc-windows-msvc.zip` |
| Windows on Arm (ARM64), from 1.3.1 on | `repodna-<version>-aarch64-pc-windows-msvc.zip` |

The Linux binaries are statically linked and run on any distribution.

### Linux and macOS

```sh
tar -xzf repodna-1.3.1-x86_64-unknown-linux-musl.tar.gz
sudo install -m 0755 repodna-1.3.1-x86_64-unknown-linux-musl/repodna /usr/local/bin/repodna
# or, without sudo:
mkdir -p ~/.local/bin && install -m 0755 repodna-1.3.1-x86_64-unknown-linux-musl/repodna ~/.local/bin/
```

### Windows

Extract the `.zip`, move `repodna.exe` to a folder of your choice (for example
`%LOCALAPPDATA%\Programs\RepoDNA`), and add that folder to your `PATH`.

### Verify the download

Each release has a `SHA256SUMS.txt`:

```sh
sha256sum --check --ignore-missing SHA256SUMS.txt           # Linux
shasum -a 256 --check --ignore-missing SHA256SUMS.txt       # macOS
```

```powershell
Get-FileHash .\repodna-1.3.1-x86_64-pc-windows-msvc.zip -Algorithm SHA256   # compare with SHA256SUMS.txt
```

### Unsigned binaries

The binaries and installers are not signed with a developer certificate.

- **macOS** blocks programs downloaded from the internet that Apple has not checked. For the
  command line, remove the quarantine attribute:
  `xattr -d com.apple.quarantine /usr/local/bin/repodna`. For the desktop app, open it once
  and close the warning, then open **System Settings > Privacy & Security**, choose
  **Open Anyway** next to the message about RepoDNA, and confirm. On macOS 14 and earlier,
  you can instead Control-click the app in Finder and choose **Open**.
- **Windows** SmartScreen may say "Windows protected your PC". Choose **More info**, then
  **Run anyway**.

## The desktop app

Installers for Linux (`.deb`, `.rpm`), macOS (`.dmg`), and Windows (`.msi`, `.exe`) are
attached to each release, and from 1.3.1 on for Windows on Arm (`_arm64_en-US.msi`,
`_arm64-setup.exe`). See [the desktop app](desktop.md).

## Container image

Each release is also published as a container image with the command line and Git, for CI
jobs and machines where you would rather not install anything. It runs on `linux/amd64` and
`linux/arm64`:

```sh
docker run --rm -v "$PWD:/work" ghcr.io/sanskarin/repodna analyze .
docker run --rm -v "$PWD:/work" --user "$(id -u):$(id -g)" \
  ghcr.io/sanskarin/repodna report . --output repodna-report
```

The current directory is mounted as `/work`, and `--user` makes the files RepoDNA writes
yours. Tags follow the releases: `1.3.1`, `1.3`, `1`, and `latest`. Stored analyses live in
`/tmp/repodna` inside the container and disappear with it; mount a volume there
(`-v repodna-data:/tmp/repodna`) to keep them. `repodna serve` in a container listens on the
container's own loopback address, so use an installed binary, the desktop app, or the
[web interface image](#web-interface-image) for the web interface. The image is based on
Alpine Linux; the Alpine packages in it, such as Git, keep their own licenses, and their
sources are available from [Alpine Linux](https://gitlab.alpinelinux.org/alpine/aports).

## Web interface image

The web interface is also published as a container image, for teams that want the
[web version](web.md#the-web-version) on their own network or without an internet
connection. It runs on `linux/amd64` and `linux/arm64`:

```sh
docker run --rm -p 8080:8080 ghcr.io/sanskarin/repodna-web
```

Then open <http://localhost:8080>. Like the web version, it opens analyses from files in
the browser (`repodna analyze . --format json > repodna.json`, or a `.repodna` export) and
never uploads them; the server only sends the interface. It cannot analyze repositories
itself: use the command line or the desktop app for that. nginx serves it as an
unprivileged user on port 8080, with the same security headers as `repodna serve`. Tags
follow the releases, as for the command line image.

## npm packages

With Node.js 18 or later, npm installs the command line:

```sh
npm install --global @sanskarin/repodna
repodna --version
```

| Package | What it is |
|---|---|
| `@sanskarin/repodna` | The `repodna` command line. npm adds the binary for your platform from `@sanskarin/repodna-linux-x64`, `-linux-arm64`, `-darwin-x64`, `-darwin-arm64`, `-win32-x64`, or, from 1.3.1 on, `-win32-arm64`. |
| `@sanskarin/repodna-web` | The web interface, built, from 1.3.0 on. `npx @sanskarin/repodna-web` serves it on this machine, at <http://127.0.0.1:8080/>, to analyze folders and open analysis files and the demo offline; its `root` export is the directory to serve from your own web server. |
| `@sanskarin/repodna-wasm` | RepoDNA's analysis, reports, and cards as WebAssembly, from 1.3.1 on, for Node.js and browsers on any system: `npx @sanskarin/repodna-wasm analyze .`. See [WebAssembly](#webassembly). |
| `@sanskarin/repodna-schema` | TypeScript types for the analysis artifact, helpers to load and check it, and the JSON Schemas of the artifact and the configuration file. |
| `@sanskarin/repodna-visualization` | The chart geometry the web interface and desktop app use: palettes, scales, treemaps, layered graphs, and heatmaps. |

The packages are on [npmjs.com](https://www.npmjs.com/package/@sanskarin/repodna) and on
GitHub Packages from 1.2.0 on. Install the command line with optional
dependencies, which npm includes by default: `--omit=optional` leaves out the platform
package and the command cannot start. The binaries are the same as the
[prebuilt binaries](#prebuilt-binaries), so the notes about unsigned binaries apply here
too.

### From GitHub Packages

GitHub Packages asks for a token even to install public packages. Create a
[personal access token (classic)](https://github.com/settings/tokens) with the
`read:packages` scope, then send the `@sanskarin` scope there:

```sh
npm config set @sanskarin:registry https://npm.pkg.github.com
npm config set //npm.pkg.github.com/:_authToken YOUR_TOKEN
npm install --global @sanskarin/repodna
```

`npm config delete @sanskarin:registry` goes back to npmjs.com.

## WebAssembly

From 1.3.1 on, RepoDNA's analysis, reports, and Project DNA cards are also built as
WebAssembly, the program the web version runs in the browser. It runs wherever Node.js or a
WASI runtime does, with nothing native to install, but without Git it analyzes no history.

```sh
# With Node.js
npx @sanskarin/repodna-wasm analyze path/to/repo --output repo.repodna
npx @sanskarin/repodna-wasm report repo.repodna --format html --output report.html

# With a WASI runtime, from the release's repodna-<version>-wasm32-wasip1.zip. The folder
# is mapped to /my-project, whose name the analysis takes.
wasmtime run --dir .::/my-project repodna.wasm analyze /my-project > my-project.repodna
```

In JavaScript, `@sanskarin/repodna-wasm/node` analyzes a folder or an archive on disk, and
`@sanskarin/repodna-wasm` runs the program in a browser on files you give it; see the
package's README.

## Build from source

Prerequisites:

- Git
- A current stable [Rust](https://www.rust-lang.org/tools/install) toolchain (install with
  `rustup`)
- [Node.js](https://nodejs.org/) 20.19 or newer with npm, to build the web interface

```sh
git clone https://github.com/sanskarIN/RepoDNA.git
cd RepoDNA
npm ci                                   # web interface dependencies
npm run build -w @repodna/web            # builds apps/web/dist, which the binary embeds
cargo install --path crates/repodna-cli --locked
```

`cargo install` puts `repodna` in `~/.cargo/bin`, which `rustup` adds to your `PATH`.

Without Node.js, skip the two `npm` commands: everything works except the interactive web
interface of `repodna serve`, which then shows a basic page that lists stored analyses and
their reports. You can also install directly from Git this way:

```sh
cargo install --git https://github.com/sanskarIN/RepoDNA --tag v1.3.1 --locked repodna-cli
```

To build the desktop app, see [apps/desktop/README.md](../apps/desktop/README.md). For
development builds, see [development](development.md).

## Shell completions

```sh
repodna completions bash > ~/.local/share/bash-completion/completions/repodna
repodna completions zsh > "${fpath[1]}/_repodna"
repodna completions fish > ~/.config/fish/completions/repodna.fish
repodna completions powershell >> $PROFILE
```

## Check the installation

```sh
repodna --version
repodna doctor
```

`repodna doctor` checks storage, the cache, Git, the language definitions, your
configuration, plugins, and AI settings, and says what to do about anything that is wrong.

## Uninstall

Delete the `repodna` binary (or run `cargo uninstall repodna-cli`), and remove the data
and configuration directories listed in [privacy](privacy.md#what-is-stored-and-where) if
you want to delete stored analyses and settings too.
