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
    for (const prerequisite of ["prepare", "security", "cli", "desktop", "winget"]) {
      assert.ok(needs.includes(prerequisite), name + " must wait for " + prerequisite);
    }
  }
});

test("GitHub release is not created before packages and images finish", () => {
  const needs = needsOf("publish");
  for (const prerequisite of ["container", "web-image", "npm", "winget", "security"]) {
    assert.ok(needs.includes(prerequisite), "release must wait for " + prerequisite);
  }
  assert.ok(needsOf("catalog").includes("publish"), "catalog must follow publication");
});
