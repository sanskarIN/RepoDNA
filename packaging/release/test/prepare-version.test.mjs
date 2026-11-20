import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import test from "node:test";
import { VERSION_FILES, planVersionBump } from "../prepare-version.mjs";

const executable = resolve("packaging/release/prepare-version.mjs");
const checker = resolve("packaging/release/check-version.mjs");
const load = () =>
  Object.fromEntries(VERSION_FILES.map((path) => [path, readFileSync(path, "utf8")]));

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "repodna-version-prepare-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const path of [...VERSION_FILES, "CHANGELOG.md"]) {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    copyFileSync(resolve(path), join(root, path));
  }
  return root;
}

function run(root, args) {
  return spawnSync(process.execPath, [executable, ...args], {
    cwd: root,
    encoding: "utf8",
  });
}

test("plans one consistent version bump without mutating input or third-party dependencies", () => {
  const original = load();
  const current = JSON.parse(original["package.json"]).version;
  const target = "1.6.0";
  assert.notEqual(current, target, "use a future release in this test");
  const plan = planVersionBump(original, target);
  assert.equal(plan.current, current);
  assert.equal(plan.target, target);
  assert.equal(plan.changes.size, VERSION_FILES.length);
  for (const path of VERSION_FILES) {
    assert.equal(typeof plan.changes.get(path), "string", path);
    assert.ok(original[path].includes(current), path);
    assert.notEqual(plan.changes.get(path), original[path], path);
  }
  assert.equal(JSON.parse(original["package.json"]).version, current);
  assert.equal(JSON.parse(plan.changes.get("package.json")).version, target);
  const oldLock = JSON.parse(original["package-lock.json"]);
  const newLock = JSON.parse(plan.changes.get("package-lock.json"));
  for (const [name, entry] of Object.entries(oldLock.packages)) {
    if (name.startsWith("node_modules/")) {
      assert.deepEqual(newLock.packages[name], entry, "do not update third-party npm package " + name);
    }
  }
  for (const path of ["Cargo.lock", "apps/desktop/src-tauri/Cargo.lock"]) {
    const oldPackages = original[path].split(/(?=^\[\[package\]\]$)/m);
    const newPackages = plan.changes.get(path).split(/(?=^\[\[package\]\]$)/m);
    assert.equal(oldPackages.length, newPackages.length, path);
    for (let i = 0; i < oldPackages.length; i++) {
      const name = /^\[\[package\]\]\r?\nname = "([^"]+)"/.exec(oldPackages[i])?.[1];
      if (name && !(name.startsWith("repodna-") || (path === "Cargo.lock" && name === "xtask"))) {
        assert.equal(newPackages[i], oldPackages[i], `${path}: external crate ${name}`);
      }
    }
  }
});

test("prepared versions pass the repository release metadata checker after a dated changelog entry", (t) => {
  const root = fixture(t);
  const plan = planVersionBump(load(), "1.6.0");
  for (const [path, updated] of plan.changes) writeFileSync(join(root, path), updated);
  const changelog = join(root, "CHANGELOG.md");
  const original = readFileSync(changelog, "utf8");
  writeFileSync(changelog, original.replace("## [Unreleased]",
    "## [Unreleased]\n\n## [1.6.0] - 2026-11-20\n\n### Added\n\n- Release qualification fixture only.\n"));
  const checked = spawnSync(process.execPath, [checker], { cwd: root, encoding: "utf8" });
  assert.equal(checked.status, 0, checked.stderr);
});

test("rejects stale internal manifests and lockfiles before producing any plan", () => {
  const source = load();
  const badWeb = { ...source };
  badWeb["apps/web/package.json"] = badWeb["apps/web/package.json"].replace(
    '"@repodna/schema": "' + JSON.parse(source["package.json"]).version + '"',
    '"@repodna/schema": "0.0.1"',
  );
  assert.throws(() => planVersionBump(badWeb, "1.6.0"), /@repodna\/schema is not aligned/);

  const badLock = { ...source };
  badLock["Cargo.lock"] = badLock["Cargo.lock"].replace(
    /name = "repodna-core"\r?\nversion = "[^"]+"/,
    'name = "repodna-core"\nversion = "0.0.1"',
  );
  assert.throws(() => planVersionBump(badLock, "1.6.0"), /Cargo\.lock: repodna-core version mismatch/);
});

test("rejects old versions, prereleases, and malformed versions", () => {
  const source = load();
  for (const bad of ["1.5.0", "0.0.1", "1.6.0-alpha.1", "v1.6.0", "1.06.0", "1.6"]) {
    assert.throws(() => planVersionBump(source, bad), undefined, bad);
  }
});

test("CLI previews changes without touching disk; writing requires an explicit flag", (t) => {
  const root = fixture(t);
  const manifest = join(root, "package.json");
  const before = readFileSync(manifest, "utf8");
  const dry = run(root, ["1.6.0"]);
  assert.equal(dry.status, 0, dry.stderr);
  assert.match(dry.stdout, /dry run only/);
  assert.equal(readFileSync(manifest, "utf8"), before);
  const invalid = run(root, ["1.6.0", "--publish"]);
  assert.notEqual(invalid.status, 0);
  assert.equal(readFileSync(manifest, "utf8"), before);
  const apply = run(root, ["1.6.0", "--write"]);
  assert.equal(apply.status, 0, apply.stderr);
  assert.equal(JSON.parse(readFileSync(manifest, "utf8")).version, "1.6.0");
  assert.match(apply.stdout, /never publishes/);
});
