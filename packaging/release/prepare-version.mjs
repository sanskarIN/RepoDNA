#!/usr/bin/env node
// Prepare a version bump without changing third-party dependency versions.
// Dry-run by default. This is not a release publisher or a changelog generator.
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const VERSION_FILES = [
  "Cargo.toml",
  "Cargo.lock",
  "apps/desktop/src-tauri/Cargo.toml",
  "apps/desktop/src-tauri/Cargo.lock",
  "package.json",
  "package-lock.json",
  "apps/web/package.json",
  "apps/desktop/package.json",
  "packages/schema/package.json",
  "packages/visualization/package.json",
  "packages/wasm/package.json",
  "apps/desktop/src-tauri/tauri.conf.json",
];

const JSON_MANIFESTS = [
  "package.json",
  "apps/web/package.json",
  "apps/desktop/package.json",
  "packages/schema/package.json",
  "packages/visualization/package.json",
  "packages/wasm/package.json",
  "apps/desktop/src-tauri/tauri.conf.json",
];
const WORKSPACE_PATHS = [
  "",
  "apps/web",
  "apps/desktop",
  "packages/schema",
  "packages/visualization",
  "packages/wasm",
];

function parseStableVersion(value) {
  assert.equal(typeof value, "string", "version must be a string");
  assert.ok(value.length <= 64 && /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(value),
    "expected a stable numeric X.Y.Z version (no prerelease or leading zeros)");
  return value.split(".").map(BigInt);
}

function assertNewer(current, target) {
  const before = parseStableVersion(current);
  const after = parseStableVersion(target);
  const cmp = after.findIndex((part, i) => part !== before[i]);
  assert.ok(cmp >= 0 && after[cmp] > before[cmp], `target ${target} must be newer than ${current}`);
}

function updateManifest(path, raw, before, after) {
  const data = JSON.parse(raw);
  assert.equal(data.version, before, `${path} has an unexpected current version`);
  data.version = after;
  if (path === "apps/web/package.json") {
    for (const dependency of ["schema", "visualization", "wasm"]) {
      const name = `@repodna/${dependency}`;
      assert.equal(data.dependencies?.[name], before, `${path}: ${name} is not aligned`);
      data.dependencies[name] = after;
    }
  }
  return JSON.stringify(data, null, 2) + "\n";
}

function updateNpmLock(raw, before, after) {
  const data = JSON.parse(raw);
  assert.equal(data.version, before, "npm lockfile root is out of sync");
  data.version = after;
  for (const workspace of WORKSPACE_PATHS) {
    const entry = data.packages?.[workspace];
    assert.ok(entry, `missing npm workspace in lockfile: ${workspace || "root"}`);
    assert.equal(entry.version, before, `npm lockfile ${workspace || "root"} version mismatch`);
    entry.version = after;
    if (workspace === "apps/web") {
      for (const dependency of ["schema", "visualization", "wasm"]) {
        const name = `@repodna/${dependency}`;
        assert.equal(entry.dependencies?.[name], before, `npm lockfile: ${name} version mismatch`);
        entry.dependencies[name] = after;
      }
    }
  }
  return JSON.stringify(data, null, 2) + "\n";
}

function updateRootCargo(raw, before, after) {
  const marker = `[workspace.package]\nversion = "${before}"`;
  assert.ok(raw.includes(marker), "Cargo workspace version is missing or out of sync");
  let result = raw.replace(marker, `[workspace.package]\nversion = "${after}"`);
  let dependencies = 0;
  result = result.replace(/^repodna-[a-z-]+\s*=\s*\{[^\n]+\}$/gm, (line) => {
    if (line.startsWith("repodna-testkit ")) return line; // Intentionally unversioned.
    const expected = `version = "${before}"`;
    assert.ok(line.includes(expected), `internal Cargo dependency out of sync: ${line}`);
    dependencies++;
    return line.replace(expected, `version = "${after}"`);
  });
  assert.equal(dependencies, 17, "unexpected internal versioned Cargo dependency inventory");
  return result;
}

function updateDesktopCargo(raw, before, after) {
  const marker = `[package]\nname = "repodna-desktop"\nversion = "${before}"`;
  assert.ok(raw.includes(marker), "desktop Cargo package/version mismatch");
  return raw.replace(marker, `[package]\nname = "repodna-desktop"\nversion = "${after}"`);
}

function updateCargoLock(path, raw, before, after) {
  let count = 0;
  const result = raw.split(/(?=^\[\[package\]\]$)/m).map((block) => {
    const name = /^\[\[package\]\]\r?\nname = "([^"]+)"/.exec(block)?.[1];
    if (!name || !(name.startsWith("repodna-") || (path === "Cargo.lock" && name === "xtask"))) {
      return block; // Every external crate stays exactly as locked.
    }
    const oldVersion = /^version = "([^"]+)"/m.exec(block)?.[1];
    assert.equal(oldVersion, before, `${path}: ${name} version mismatch`);
    count++;
    return block.replace(/^version = "[^"]+"/m, `version = "${after}"`);
  }).join("");
  assert.equal(count, path === "Cargo.lock" ? 21 : 18,
    `${path}: unexpected local package inventory`);
  return result;
}

/** Returns an immutable plan. Every file is validated before any caller may write. */
export function planVersionBump(sources, target) {
  const current = JSON.parse(sources["package.json"]).version;
  assertNewer(current, target);
  for (const path of VERSION_FILES) {
    assert.equal(typeof sources[path], "string", `missing required version file: ${path}`);
  }
  const changes = new Map();
  for (const path of JSON_MANIFESTS) {
    changes.set(path, updateManifest(path, sources[path], current, target));
  }
  changes.set("package-lock.json", updateNpmLock(sources["package-lock.json"], current, target));
  changes.set("Cargo.toml", updateRootCargo(sources["Cargo.toml"], current, target));
  changes.set("apps/desktop/src-tauri/Cargo.toml",
    updateDesktopCargo(sources["apps/desktop/src-tauri/Cargo.toml"], current, target));
  for (const path of ["Cargo.lock", "apps/desktop/src-tauri/Cargo.lock"]) {
    changes.set(path, updateCargoLock(path, sources[path], current, target));
  }
  for (const path of VERSION_FILES) {
    assert.notEqual(changes.get(path), undefined, `missing planned result: ${path}`);
  }
  return { current, target, changes };
}

function main(args) {
  if (args.length < 1 || args.length > 2 || (args[1] && args[1] !== "--write")) {
    throw new Error("Usage: node packaging/release/prepare-version.mjs X.Y.Z [--write]");
  }
  const sources = Object.fromEntries(VERSION_FILES.map((path) => [path, readFileSync(path, "utf8")]));
  const plan = planVersionBump(sources, args[0]);
  const write = args[1] === "--write";
  for (const path of VERSION_FILES) {
    const updated = plan.changes.get(path);
    if (write) writeFileSync(path, updated);
    console.log(`${write ? "Updated" : "Would update"} ${path}`);
  }
  console.log(`RepoDNA ${plan.current} -> ${plan.target}: ${write ? "metadata written" : "dry run only"}.`);
  console.log("Add and verify a dated changelog section separately; run release:check, both cargo metadata --locked checks, cargo xtask notices --check, and full CI. This script never publishes.");
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(`Release version preparation failed: ${error.message}`);
    process.exitCode = 1;
  }
}
