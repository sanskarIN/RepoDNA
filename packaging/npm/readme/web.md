# RepoDNA web interface

[RepoDNA](https://github.com/sanskarIN/RepoDNA) is local-first repository intelligence and
code archaeology: evidence-backed analysis of a repository's structure, architecture,
history, and health, on your own machine and without telemetry.

This package is RepoDNA's web interface, built: the same interface as the
[web version](https://sanskarin.github.io/RepoDNA/), for opening analysis files and the
bundled demo without an internet connection, or for serving on your own network.

## Run it

```sh
npx @sanskarin/repodna-web
```

It serves the interface at <http://127.0.0.1:8080/>, on this machine only. `--port` picks
another port (`0` for a free one), and `--host 0.0.0.0` serves it to other machines too.
Analysis files you open are read in the browser and never sent to the server. Make them with
the [command line](https://www.npmjs.com/package/@sanskarin/repodna):
`repodna analyze --format json` or `repodna export`.

## Serve it yourself

The interface is a static application in `dist/`, with relative paths and hash-based
routing, so any web server can serve it from any path:

```js
import { root } from "@sanskarin/repodna-web";
// root is the directory that holds index.html, for example for express.static(root).
```

`createWebServer(root)` returns the Node HTTP server that `npx @sanskarin/repodna-web` uses,
with the same security headers as `repodna serve`. For a container instead, use
`ghcr.io/sanskarin/repodna-web`.

RepoDNA is made by [Sanskar](https://sanskarin.github.io) and licensed under the Apache
License 2.0. `dist/THIRD-PARTY-NOTICES.txt` lists the licenses of the software it includes.
