// Tests for the npm packaging script. Run them with
// node --test "packaging/npm/test/*.test.mjs"

import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { pathToFileURL } from "node:url";
import { build } from "../build.mjs";

let dir;
let out;
const manifest = (name) => JSON.parse(readFileSync(join(out, name, "package.json"), "utf8"));

before(() => {
  dir = mkdtempSync(join(tmpdir(), "repodna-npm-"));
  out = join(dir, "out");
  build({ version: "9.8.7", out });
});

after(() => rmSync(dir, { recursive: true, force: true }));

test("stages every package with the version and the repository", () => {
  for (const name of ["repodna-schema", "repodna-visualization"]) {
    const pkg = manifest(name);
    assert.equal(pkg.name, `@sanskarin/${name}`);
    assert.equal(pkg.version, "9.8.7");
    assert.equal(pkg.license, "Apache-2.0");
    assert.equal(pkg.repository.url, "git+https://github.com/sanskarIN/RepoDNA.git");
    assert.equal(pkg.publishConfig.registry, "https://npm.pkg.github.com");
    for (const file of ["README.md", "LICENSE", "NOTICE"]) {
      assert.ok(statSync(join(out, name, file)).size > 0, `${name}/${file}`);
    }
  }
});

test("builds libraries that Node can load", async () => {
  const visualization = await import(
    pathToFileURL(join(out, "repodna-visualization", "dist", "index.js")).href
  );
  assert.equal(visualization.thousands(12345), "12,345");
  const schema = await import(pathToFileURL(join(out, "repodna-schema", "dist", "index.js")).href);
  assert.equal(typeof schema.parseArtifact, "function");
  const pkg = manifest("repodna-schema");
  assert.equal(pkg.exports["./artifact.schema.json"], "./schemas/repodna-artifact.schema.json");
  const declarations = readFileSync(join(out, "repodna-schema", "dist", "index.d.ts"), "utf8");
  assert.match(declarations, /from "\.\/generated\.js"/);
});
