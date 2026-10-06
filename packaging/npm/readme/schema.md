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

```sh
npm install @sanskarin/repodna-schema
```

The package is on npmjs.com and on GitHub Packages, which asks for a GitHub token even to
install public packages; see the [installation guide][guide].

Licensed under the Apache License 2.0.

[guide]: https://github.com/sanskarIN/RepoDNA/blob/main/docs/installation.md#npm-packages
