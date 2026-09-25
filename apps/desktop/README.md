# RepoDNA desktop app

The RepoDNA web interface in a native window, built with [Tauri 2](https://v2.tauri.app/).
The interface talks to the Rust core through Tauri commands instead of HTTP, so it works
without `repodna serve`, keeps everything on your machine, and adds native folder pickers
and save dialogs. See [docs/desktop.md](../../docs/desktop.md) for what it can do.

## Prerequisites

- Rust and Node.js, as for the rest of the repository
- Tauri's system dependencies for your platform: <https://v2.tauri.app/start/prerequisites/>
  (on Debian and Ubuntu: `libwebkit2gtk-4.1-dev libxdo-dev libssl-dev
  libayatana-appindicator3-dev librsvg2-dev`)
- The Tauri command line: `cargo install tauri-cli --version "^2" --locked`

## Develop

From this directory:

```sh
cargo tauri dev
```

This starts the web interface's development server (`npm run dev -w @repodna/web`) and
opens the app with live reloading.

## Build

```sh
cargo tauri build
```

This builds the web interface, embeds it, and writes installers for your platform to
`src-tauri/target/release/bundle/`. Without the Tauri command line,
`cargo build --release --features custom-protocol` in `src-tauri/` builds the app binary
with the interface embedded (build the web interface first with
`npm run build -w @repodna/web` from the repository root).

`src-tauri` is deliberately outside the Cargo workspace, so building the command-line tool
does not require the desktop system libraries.
