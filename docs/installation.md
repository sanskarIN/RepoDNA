# Installation

RepoDNA is one command-line program, `repodna`, with the web interface built in. A desktop
app is also available. Everything runs on your machine. To look at analyses without
installing anything, use the [web version](web.md#the-web-version).

- [Prebuilt binaries](#prebuilt-binaries)
- [The desktop app](#the-desktop-app)
- [Container image](#container-image)
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

The Linux binaries are statically linked and run on any distribution.

### Linux and macOS

```sh
tar -xzf repodna-1.0.0-x86_64-unknown-linux-musl.tar.gz
sudo install -m 0755 repodna-1.0.0-x86_64-unknown-linux-musl/repodna /usr/local/bin/repodna
# or, without sudo:
mkdir -p ~/.local/bin && install -m 0755 repodna-1.0.0-x86_64-unknown-linux-musl/repodna ~/.local/bin/
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
Get-FileHash .\repodna-1.0.0-x86_64-pc-windows-msvc.zip -Algorithm SHA256   # compare with SHA256SUMS.txt
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
attached to each release. See [the desktop app](desktop.md).

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
yours. Tags follow the releases: `1.0.0`, `1.0`, `1`, and `latest`. Stored analyses live in
`/tmp/repodna` inside the container and disappear with it; mount a volume there
(`-v repodna-data:/tmp/repodna`) to keep them. `repodna serve` in a container listens on the
container's own loopback address, so use an installed binary or the desktop app for the web
interface. The image is based on Alpine Linux; the Alpine packages in it, such as Git, keep
their own licenses, and their sources are available from
[Alpine Linux](https://gitlab.alpinelinux.org/alpine/aports).

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
cargo install --git https://github.com/sanskarIN/RepoDNA --tag v1.0.0 --locked repodna-cli
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
configuration, and plugins, and says what to do about anything that is wrong.

## Uninstall

Delete the `repodna` binary (or run `cargo uninstall repodna-cli`), and remove the data
and configuration directories listed in [privacy](privacy.md#what-is-stored-and-where) if
you want to delete stored analyses and settings too.
