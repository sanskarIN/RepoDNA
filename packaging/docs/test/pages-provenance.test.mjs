import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { manifestFromEnvironment, stampDeployment } from "../stamp-deployment.mjs";

const GOOD = {
  GITHUB_SHA: "a".repeat(40),
  GITHUB_RUN_ID: "12345",
  GITHUB_RUN_ATTEMPT: "2",
};

test("stamps only public, exact source identity into an existing site", () => {
  const dir = mkdtempSync(join(tmpdir(), "repodna-pages-"));
  try {
    const output = stampDeployment(dir, {
      ...GOOD,
      GITHUB_TOKEN: "SECRET_MUST_NOT_APPEAR",
      ACTIONS_RUNTIME_TOKEN: "ANOTHER_SECRET",
      GITHUB_REPOSITORY: "private-project",
    });
    assert.equal(output, join(dir, "deployment.json"));
    const published = readFileSync(output, "utf8");
    const manifest = JSON.parse(published);
    assert.deepEqual(manifest, {
      schema: "repodna-pages-deployment/1",
      commitSha: "a".repeat(40),
      workflowRunId: 12345,
      workflowRunAttempt: 2,
    });
    assert.doesNotMatch(published, /SECRET|private-project|TOKEN/);
    assert.match(published, /\n$/);
    assert.throws(() => stampDeployment(dir, GOOD), /EEXIST/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("rejects malformed commit/run provenance and does not create a manifest", () => {
  const dir = mkdtempSync(join(tmpdir(), "repodna-pages-"));
  try {
    for (const invalid of [
      { ...GOOD, GITHUB_SHA: "x".repeat(40) },
      { ...GOOD, GITHUB_SHA: "f".repeat(39) },
      { ...GOOD, GITHUB_RUN_ID: "0" },
      { ...GOOD, GITHUB_RUN_ID: "1.5" },
      { ...GOOD, GITHUB_RUN_ID: "9007199254740992" },
      { ...GOOD, GITHUB_RUN_ATTEMPT: "0" },
      { ...GOOD, GITHUB_RUN_ATTEMPT: "-1" },
    ]) {
      assert.throws(() => stampDeployment(dir, invalid), /Pages provenance/);
    }
    assert.equal(readFileSync(join(dir, "deployment.json"), "utf8"), "");
  } catch (error) {
    // No output file is expected. Check the absence explicitly without
    // treating ENOENT as a successful manifest read.
    if (error.code !== "ENOENT") throw error;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("refuses a path that is not an already built site", () => {
  const dir = mkdtempSync(join(tmpdir(), "repodna-pages-"));
  try {
    assert.throws(
      () => stampDeployment(join(dir, "missing"), GOOD),
      /existing built site directory/,
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("manifest is deterministic and validates all fields", () => {
  assert.deepEqual(manifestFromEnvironment(GOOD), manifestFromEnvironment({ ...GOOD }));
  assert.throws(() => manifestFromEnvironment({}), /GITHUB_SHA/);
});
