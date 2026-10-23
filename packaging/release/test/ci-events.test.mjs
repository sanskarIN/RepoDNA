import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const ci = readFileSync(".github/workflows/ci.yml", "utf8");

test("runs CI after merges to main and for each pull request", () => {
  assert.match(ci, /\n  push:\n    branches: \["main"\]/);
  assert.match(ci, /\n  pull_request:\n/);
});

test("preserves manual CI dispatch for standalone branches", () => {
  assert.match(ci, /\n  workflow_dispatch:\n/);
  assert.doesNotMatch(ci, /branches: \["\*\*"\]/);
});
