import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const release = readFileSync(".github/workflows/release.yml", "utf8");

function needsOf(name) {
  const marker = "\n  " + name + ":\n";
  const start = release.indexOf(marker);
  assert.notEqual(start, -1, "missing job: " + name);
  const block = release.slice(start + marker.length).split(/\n  [a-z][a-z-]*:\n/, 1)[0];
  const match = /^    needs: \[([^\]]+)\]/m.exec(block);
  assert.ok(match, "missing explicit dependencies for " + name);
  return match[1].split(",").map((part) => part.trim());
}

test("registry publishing jobs wait for all installer and WinGet validation", () => {
  for (const name of ["container", "web-image", "npm"]) {
    const needs = needsOf(name);
    for (const prerequisite of ["prepare", "security", "cli", "desktop", "winget", "release-assets"]) {
      assert.ok(needs.includes(prerequisite), name + " must wait for " + prerequisite);
    }
  }
});

test("GitHub release is not created before packages and images finish", () => {
  const needs = needsOf("publish");
  for (const prerequisite of ["container", "web-image", "npm", "winget", "security", "release-assets"]) {
    assert.ok(needs.includes(prerequisite), "release must wait for " + prerequisite);
  }
  assert.ok(needsOf("catalog").includes("publish"), "catalog must follow publication");
});

test("release asset inventory is checked in both dry runs and published releases", () => {
  const checks = needsOf("release-assets");
  for (const prerequisite of ["prepare", "cli", "desktop", "winget"]) {
    assert.ok(checks.includes(prerequisite), "inventory must wait for " + prerequisite);
  }
  assert.match(release, /node packaging\/release\/check-windows-inventory\.mjs release-assets/);
  assert.doesNotMatch(
    release.slice(release.indexOf("\n  release-assets:\n"), release.indexOf("\n  container:\n")),
    /\n    if:/,
  );
});

test("unpublished branch trials create verified checksums without publishing", () => {
  assert.match(release, /branches: \["release\/v\*"\]/);
  assert.match(
    release,
    /github\.event_name == 'push' && github\.ref_type == 'tag' && github\.ref_name/,
  );

  const start = release.indexOf("\n  trial-assets:\n");
  const end = release.indexOf("\n  publish:\n");
  assert.ok(start >= 0 && end > start, "trial assets must precede publication");
  const trial = release.slice(start, end);
  assert.match(trial, /if: needs\.prepare\.outputs\.publish == 'false'/);
  assert.match(trial, /sha256sum --check SHA256SUMS\.txt/);
  assert.match(trial, /name: trial-sha256sums/);
  for (const name of ["prepare", "security", "web", "cli", "desktop", "winget", "release-assets"]) {
    assert.ok(needsOf("trial-assets").includes(name), "trial needs " + name);
  }

  const publishStart = release.indexOf("\n  publish:\n");
  const catalogStart = release.indexOf("\n  catalog:\n");
  const publish = release.slice(publishStart, catalogStart);
  assert.match(publish, /if: needs\.prepare\.outputs\.publish == 'true'/);
});
