# RepoDNA artifact schema

TypeScript types for the [RepoDNA](https://github.com/sanskarIN/RepoDNA) analysis artifact,
the JSON that `repodna analyze --format json` writes, generated from its JSON Schema, with
helpers to load, check, and describe artifacts. The JSON Schemas of the artifact and of the
configuration file are included.

```ts
import { readFile } from "node:fs/promises";
import { findingCounts, parseArtifact } from "@sanskarin/repodna-schema";

const { artifact, warnings } = parseArtifact(await readFile("repodna.json", "utf8"));
console.log(artifact.identity.name, findingCounts(artifact), warnings);
```

The schemas are exported as `@sanskarin/repodna-schema/artifact.schema.json` and
`@sanskarin/repodna-schema/config.schema.json`.

## Install

The package is on GitHub Packages, which asks for a GitHub token even to install public
packages. With a [personal access token (classic)](https://github.com/settings/tokens) that
has the `read:packages` scope:

```sh
npm config set @sanskarin:registry https://npm.pkg.github.com
npm config set //npm.pkg.github.com/:_authToken YOUR_TOKEN
npm install @sanskarin/repodna-schema
```

Licensed under the Apache License 2.0.
