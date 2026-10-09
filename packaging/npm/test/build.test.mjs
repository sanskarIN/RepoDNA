// Tests for the npm packaging script. Run them with
// node --test "packaging/npm/test/*.test.mjs"

import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { pathToFileURL } from "node:url";
import { build } from "../build.mjs";
import { PLATFORMS } from "../launcher/lib/platforms.js";

let dir;
let out;
const manifest = (name) => JSON.parse(readFileSync(join(out, name, "package.json"), "utf8"));

before(() => {
  dir = mkdtempSync(join(tmpdir(), "repodna-npm-"));
  out = join(dir, "out");
  const binaries = join(dir, "binaries");
  for (const platform of PLATFORMS) {
    mkdirSync(join(binaries, platform.target), { recursive: true });
    writeFileSync(join(binaries, platform.target, platform.binary), "binary");
  }
  const web = join(dir, "web");
  mkdirSync(join(web, "assets"), { recursive: true });
  writeFileSync(join(web, "index.html"), "<!doctype html><title>RepoDNA</title>");
  writeFileSync(join(web, "assets", "index-abc.js"), "export {};");
  build({ version: "9.8.7", out, binaries, web });
});

after(() => rmSync(dir, { recursive: true, force: true }));

test("stages every package with the version and the repository", () => {
  const names = [
    "repodna-schema",
    "repodna-visualization",
    "repodna-web",
    ...PLATFORMS.map((platform) => `repodna-${platform.os}-${platform.cpu}`),
    "repodna",
  ];
  for (const name of names) {
    const pkg = manifest(name);
    assert.equal(pkg.name, `@sanskarin/${name}`);
    assert.equal(pkg.version, "9.8.7");
    assert.equal(pkg.license, "Apache-2.0");
    assert.equal(pkg.repository.url, "git+https://github.com/sanskarIN/RepoDNA.git");
    assert.equal(pkg.publishConfig, undefined, "the registry is chosen when publishing");
    for (const file of ["README.md", "LICENSE", "NOTICE"]) {
      assert.ok(statSync(join(out, name, file)).size > 0, `${name}/${file}`);
    }
  }
});

test("pins the launcher to the platform packages of the same version", () => {
  const launcher = manifest("repodna");
  assert.deepEqual(launcher.bin, { repodna: "bin/repodna.js" });
  assert.deepEqual(
    Object.values(launcher.optionalDependencies),
    PLATFORMS.map(() => "9.8.7"),
  );
  const linux = manifest("repodna-linux-arm64");
  assert.deepEqual([linux.os, linux.cpu], [["linux"], ["arm64"]]);
  const mode = statSync(join(out, "repodna-linux-arm64", "bin", "repodna")).mode;
  assert.equal(mode & 0o111, 0o111, "the binary is executable");
  assert.ok(statSync(join(out, "repodna-win32-x64", "THIRD-PARTY-NOTICES.txt")).size > 0);
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

test("stages the built web interface with a server to run it", async () => {
  const web = manifest("repodna-web");
  assert.deepEqual(web.bin, { "repodna-web": "bin/repodna-web.js" });
  assert.deepEqual(web.files, ["bin", "lib", "dist", "README.md", "LICENSE", "NOTICE"]);
  const mode = statSync(join(out, "repodna-web", "bin", "repodna-web.js")).mode;
  assert.equal(mode & 0o111, 0o111, "the server is executable");
  const { root } = await import(pathToFileURL(join(out, "repodna-web", "lib", "index.js")).href);
  assert.equal(
    readFileSync(join(root, "index.html"), "utf8"),
    "<!doctype html><title>RepoDNA</title>",
  );
  assert.ok(statSync(join(root, "assets", "index-abc.js")).isFile());
});

test("stages no web interface without a built one", () => {
  assert.throws(
    () =>
      build({
        version: "9.8.7",
        out: join(dir, "empty-out"),
        binaries: null,
        web: join(dir, "nothing"),
      }),
    /index\.html is missing/,
  );
});
