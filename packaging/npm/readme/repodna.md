# RepoDNA command line

[RepoDNA](https://github.com/sanskarIN/RepoDNA) is local-first repository intelligence and
code archaeology: evidence-backed analysis of a repository's structure, architecture,
history, and health, on your own machine and without telemetry.

This package installs the `repodna` command. npm adds the binary for your platform from one
of `@sanskarin/repodna-linux-x64`, `@sanskarin/repodna-linux-arm64`,
`@sanskarin/repodna-darwin-x64`, `@sanskarin/repodna-darwin-arm64`,
`@sanskarin/repodna-win32-x64`, and `@sanskarin/repodna-win32-arm64`. On any other system,
`@sanskarin/repodna-wasm` analyzes with WebAssembly, without Git history.

## Install

```sh
npm install --global @sanskarin/repodna
repodna analyze .
```

npm installs the binary as an optional dependency, which it includes by default: with
`--omit=optional`, the command cannot start. The packages are on npmjs.com and on GitHub
Packages, which asks for a GitHub token even to install public packages. The [installation
guide][guide] explains both, and the ways to install RepoDNA without npm: the downloads on
the releases page, the container image, and Cargo.

RepoDNA is made by [Sanskar](https://sanskarin.github.io) and licensed under the Apache
License 2.0.

[guide]: https://github.com/sanskarIN/RepoDNA/blob/main/docs/installation.md#npm-packages
