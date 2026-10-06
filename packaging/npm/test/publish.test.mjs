// Tests for the npm publishing script, with a stand-in for npm. Run them with
// node --test "packaging/npm/test/*.test.mjs"

import assert from "node:assert/strict";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, beforeEach, test } from "node:test";
import { isOlder, npmrc, publish, publishOrder } from "../publish.mjs";

let dir;
let staged;
let npm;
let calls;

// Records each call with the configuration it was given, answers `npm view` from
// FAKE_PUBLISHED and the latest versions from FAKE_LATEST, and fails `npm view` for
// FAKE_BROKEN.
const FAKE_NPM = `#!/usr/bin/env node
const { appendFileSync, readFileSync } = require("node:fs");
const args = process.argv.slice(2);
const userconfig = args[args.indexOf("--userconfig") + 1];
appendFileSync(process.env.FAKE_CALLS, JSON.stringify({ args, config: readFileSync(userconfig, "utf8") }) + "\\n");
if (args[0] === "view" && args[2] === "dist-tags.latest") {
  const latest = JSON.parse(process.env.FAKE_LATEST || "{}")[args[1]];
  if (latest) {
    process.stdout.write(latest + "\\n");
    process.exit(0);
  }
  process.stderr.write("npm error code E404\\nnpm error 404 Not Found\\n");
  process.exit(1);
}
if (args[0] === "view") {
  if (process.env.FAKE_BROKEN === args[1]) {
    process.stderr.write("npm error code E401\\n");
    process.exit(1);
  }
  if ((process.env.FAKE_PUBLISHED || "").split(",").includes(args[1])) {
    process.stdout.write(args[1].split("@").pop() + "\\n");
    process.exit(0);
  }
  process.stderr.write("npm error code E404\\nnpm error 404 Not Found\\n");
  process.exit(1);
}
`;

function stage(name, version) {
  const path = join(staged, name.replace("@sanskarin/", ""));
  mkdirSync(path, { recursive: true });
  writeFileSync(join(path, "package.json"), JSON.stringify({ name, version }));
}

function readCalls() {
  return readFileSync(calls, "utf8")
    .trim()
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

before(() => {
  dir = mkdtempSync(join(tmpdir(), "repodna-publish-test-"));
  staged = join(dir, "staged");
  npm = join(dir, "npm");
  writeFileSync(npm, FAKE_NPM);
  chmodSync(npm, 0o755);
  for (const name of ["repodna", "repodna-linux-x64", "repodna-schema", "repodna-win32-x64"]) {
    stage(`@sanskarin/${name}`, "9.8.7");
  }
  mkdirSync(join(staged, "not-a-package"));
});

beforeEach(() => {
  calls = join(dir, `calls-${Date.now()}-${Math.random()}.jsonl`);
  writeFileSync(calls, "");
  process.env.FAKE_CALLS = calls;
  delete process.env.FAKE_PUBLISHED;
  delete process.env.FAKE_LATEST;
  delete process.env.FAKE_BROKEN;
  delete process.env.NODE_AUTH_TOKEN;
});

after(() => rmSync(dir, { recursive: true, force: true }));

test("publishes the launcher after the packages it names", () => {
  assert.deepEqual(
    publishOrder(staged).map((pkg) => pkg.name),
    [
      "@sanskarin/repodna-linux-x64",
      "@sanskarin/repodna-schema",
      "@sanskarin/repodna-win32-x64",
      "@sanskarin/repodna",
    ],
  );
});

test("sends the scope and the token to the chosen registry", () => {
  assert.equal(
    npmrc("https://registry.npmjs.org", true),
    [
      "registry=https://registry.npmjs.org/",
      "@sanskarin:registry=https://registry.npmjs.org/",
      "//registry.npmjs.org/:_authToken=${NODE_AUTH_TOKEN}",
      "",
    ].join("\n"),
  );
  assert.equal(
    npmrc("https://npm.pkg.github.com/", false),
    "registry=https://npm.pkg.github.com/\n@sanskarin:registry=https://npm.pkg.github.com/\n",
  );
});

test("skips versions the registry has and publishes the rest", () => {
  process.env.FAKE_PUBLISHED = "@sanskarin/repodna-schema@9.8.7";
  process.env.NODE_AUTH_TOKEN = "secret";
  const lines = [];
  const done = publish({
    registry: "https://registry.npmjs.org/",
    dir: staged,
    extra: ["--access", "public"],
    npm,
    log: (line) => lines.push(line),
  });
  assert.deepEqual(done, [
    "@sanskarin/repodna-linux-x64@9.8.7",
    "@sanskarin/repodna-win32-x64@9.8.7",
    "@sanskarin/repodna@9.8.7",
  ]);
  assert.deepEqual(lines, [
    "@sanskarin/repodna-schema@9.8.7 is already on https://registry.npmjs.org/.",
  ]);
  const published = readCalls().filter((call) => call.args[0] === "publish");
  assert.equal(published.length, 3);
  for (const call of published) {
    assert.deepEqual(call.args.slice(-2), ["--access", "public"]);
    assert.equal(call.args[call.args.indexOf("--registry") + 1], "https://registry.npmjs.org/");
    assert.match(call.config, /^\/\/registry\.npmjs\.org\/:_authToken=\$\{NODE_AUTH_TOKEN\}$/m);
  }
  assert.ok(published.at(-1).args[1].endsWith("repodna"), "the launcher goes last");
});

test("leaves the token out when there is none", () => {
  process.env.FAKE_PUBLISHED = [
    "@sanskarin/repodna@9.8.7",
    "@sanskarin/repodna-linux-x64@9.8.7",
    "@sanskarin/repodna-schema@9.8.7",
    "@sanskarin/repodna-win32-x64@9.8.7",
  ].join(",");
  const done = publish({ registry: "https://npm.pkg.github.com", dir: staged, npm, log: () => {} });
  assert.deepEqual(done, []);
  for (const call of readCalls()) {
    assert.doesNotMatch(call.config, /_authToken/);
  }
});

test("stops when the registry cannot say whether a version is there", () => {
  process.env.FAKE_BROKEN = "@sanskarin/repodna-schema@9.8.7";
  assert.throws(
    () => publish({ registry: "https://npm.pkg.github.com", dir: staged, npm, log: () => {} }),
    /Could not check whether @sanskarin\/repodna-schema@9\.8\.7 is published:\nnpm error code E401/,
  );
  const published = readCalls().filter((call) => call.args[0] === "publish");
  assert.deepEqual(
    published.map((call) => call.args[1].split("/").pop()),
    ["repodna-linux-x64"],
    "nothing after the failed check is published",
  );
});

test("compares release versions by their numbers", () => {
  assert.equal(isOlder("1.2.0", "1.2.1"), true);
  assert.equal(isOlder("1.9.9", "1.10.0"), true);
  assert.equal(isOlder("2.0.0", "10.0.0"), true);
  assert.equal(isOlder("1.10.0", "1.9.9"), false);
  assert.equal(isOlder("1.2.1", "1.2.1"), false);
});

test("publishes a release older than the latest under the previous tag", () => {
  process.env.FAKE_LATEST = JSON.stringify({
    "@sanskarin/repodna-schema": "9.10.0",
    "@sanskarin/repodna": "9.8.6",
  });
  publish({ registry: "https://registry.npmjs.org", dir: staged, npm, log: () => {} });
  const tags = Object.fromEntries(
    readCalls()
      .filter((call) => call.args[0] === "publish")
      .map((call) => [
        call.args[1].split("/").pop(),
        call.args.includes("--tag")
          ? call.args.slice(call.args.indexOf("--tag"), call.args.indexOf("--tag") + 2)
          : [],
      ]),
  );
  assert.deepEqual(tags, {
    "repodna-linux-x64": [],
    "repodna-schema": ["--tag", "previous"],
    "repodna-win32-x64": [],
    repodna: [],
  });
});

test("publishes a prerelease under the next tag", () => {
  const prerelease = join(dir, "prerelease");
  mkdirSync(join(prerelease, "repodna-schema"), { recursive: true });
  writeFileSync(
    join(prerelease, "repodna-schema", "package.json"),
    JSON.stringify({ name: "@sanskarin/repodna-schema", version: "9.9.0-rc.1" }),
  );
  publish({ registry: "https://registry.npmjs.org", dir: prerelease, npm, log: () => {} });
  publish({
    registry: "https://registry.npmjs.org",
    dir: prerelease,
    extra: ["--tag", "beta"],
    npm,
    log: () => {},
  });
  const tags = readCalls()
    .filter((call) => call.args[0] === "publish")
    .map((call) => call.args.slice(call.args.indexOf("--tag")));
  assert.deepEqual(tags, [
    ["--tag", "next"],
    ["--tag", "beta"],
  ]);
});
