# RepoDNA as WebAssembly

[RepoDNA](https://github.com/sanskarIN/RepoDNA) is local-first repository intelligence and
code archaeology: evidence-backed analysis of a repository's structure, architecture,
history, and health, on your own machine and without telemetry.

This package is RepoDNA's analysis, reports, and Project DNA cards built as WebAssembly, the
program the [web version](https://sanskarin.github.io/RepoDNA/) runs to analyze a folder in
the browser. It runs in Node.js and in browsers, on any system they run on, with no native
program to install, and it reads the files where they are: nothing is uploaded.

Browsers and WebAssembly cannot run Git, so an analysis made with this package has no
history: no commits, contributors, hotspots, or Time Machine. For those, use the
[command line](https://www.npmjs.com/package/@sanskarin/repodna).

## On the command line

```sh
npx @sanskarin/repodna-wasm analyze path/to/repo --output repo.repodna
npx @sanskarin/repodna-wasm report repo.repodna --format html --output report.html
npx @sanskarin/repodna-wasm card repo.repodna --png --output dna-card.png
```

`analyze` takes a folder or a `.zip`, `.tar`, `.tar.gz`, or `.tgz` archive, and `--profile`
(`quick`, `standard`, or `deep`). The `.repodna` file it writes opens in the web version,
`repodna serve`, and the desktop app.

## In Node.js

```js
import { analyze, card, report } from "@sanskarin/repodna-wasm/node";

const artifact = await analyze("path/to/repo", { profile: "quick" });
const dna = JSON.parse(artifact);
console.log(dna.identity.name, dna.languages.primary);
const html = await report(artifact, { format: "html", theme: "dark", privacy: "share" });
const svg = await card(artifact, { dark: true });
```

## In a browser

Load the program from `@sanskarin/repodna-wasm/repodna.wasm`, preferably in a worker, and
give it files with their size and a way to read them:

```js
import { RepoDNA } from "@sanskarin/repodna-wasm";

const repodna = await RepoDNA.load(fetch(programUrl));
const artifact = await repodna.analyzeFolder("my-project", [
  { path: "src/main.rs", size: bytes.length, read: () => bytes },
]);
```

`analyzeArchive`, `report`, `card`, `cardPng`, and `version` do the rest. A file is read only
when the analysis opens it, so files that `.gitignore` excludes are never read. The page's
Content Security Policy must allow `'wasm-unsafe-eval'` in `script-src`.

## Install

```sh
npm install @sanskarin/repodna-wasm
```

The package is on npmjs.com and on GitHub Packages, which asks for a GitHub token even to
install public packages; see the [installation guide][guide].

RepoDNA is made by [Sanskar](https://sanskarin.github.io) and licensed under the Apache
License 2.0. `THIRD-PARTY-NOTICES.txt` lists the licenses of the software it includes.

[guide]: https://github.com/sanskarIN/RepoDNA/blob/main/docs/installation.md#npm-packages
