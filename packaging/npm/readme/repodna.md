# RepoDNA command line

[RepoDNA](https://github.com/sanskarIN/RepoDNA) is local-first repository intelligence and
code archaeology: evidence-backed analysis of a repository's structure, architecture,
history, and health, on your own machine and without telemetry.

This package installs the `repodna` command. npm adds the binary for your platform from one
of `@sanskarin/repodna-linux-x64`, `@sanskarin/repodna-linux-arm64`,
`@sanskarin/repodna-darwin-x64`, `@sanskarin/repodna-darwin-arm64`, and
`@sanskarin/repodna-win32-x64`.

## Install

The package is on GitHub Packages, which asks for a GitHub token even to install public
packages. Create a [personal access token (classic)](https://github.com/settings/tokens)
with the `read:packages` scope, then:

```sh
npm config set @sanskarin:registry https://npm.pkg.github.com
npm config set //npm.pkg.github.com/:_authToken YOUR_TOKEN
npm install --global @sanskarin/repodna
repodna analyze .
```

The [installation guide](https://github.com/sanskarIN/RepoDNA/blob/main/docs/installation.md)
describes the ways to install RepoDNA without a token: the downloads on the releases page,
the container image, and Cargo.

RepoDNA is made by [Sanskar](https://sanskarin.github.io) and licensed under the Apache
License 2.0.
