// Tests for the launcher of @sanskarin/repodna. Run them with
// node --test "packaging/npm/test/*.test.mjs"

import assert from "node:assert/strict";
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { LauncherError, locate, run } from "../launcher/lib/launcher.js";
import { PLATFORMS, findPlatform, platformPackage } from "../launcher/lib/platforms.js";

test("names one package per platform", () => {
  assert.deepEqual(PLATFORMS.map(platformPackage), [
    "@sanskarin/repodna-linux-x64",
    "@sanskarin/repodna-linux-arm64",
    "@sanskarin/repodna-darwin-x64",
    "@sanskarin/repodna-darwin-arm64",
    "@sanskarin/repodna-win32-x64",
  ]);
  assert.equal(findPlatform("win32", "x64")?.binary, "repodna.exe");
  assert.equal(findPlatform("linux", "ia32"), undefined);
});

test("finds the binary in the package for this platform", () => {
  const resolve = (request) => {
    assert.equal(request, "@sanskarin/repodna-darwin-arm64/package.json");
    return "/modules/@sanskarin/repodna-darwin-arm64/package.json";
  };
  assert.equal(
    locate("darwin", "arm64", resolve),
    join("/modules/@sanskarin/repodna-darwin-arm64", "bin", "repodna"),
  );
});

test("explains an unsupported platform and a missing package", () => {
  assert.throws(
    () => locate("aix", "ppc64", () => "unused"),
    (error) => error instanceof LauncherError && error.message.includes("/releases"),
  );
  const missing = () => {
    throw new Error("Cannot find module");
  };
  assert.throws(
    () => locate("linux", "x64", missing),
    (error) =>
      error instanceof LauncherError &&
      error.message.includes("@sanskarin/repodna-linux-x64") &&
      error.message.includes("--omit=optional"),
  );
});

test("passes arguments and the exit status through", { skip: process.platform === "win32" }, () => {
  const dir = mkdtempSync(join(tmpdir(), "repodna-launcher-"));
  try {
    const out = join(dir, "args.txt");
    const binary = join(dir, "repodna");
    writeFileSync(binary, `#!/bin/sh\nprintf '%s\\n' "$@" > '${out}'\nexit 3\n`);
    // Not executable: the launcher restores the executable bit and tries again.
    chmodSync(binary, 0o644);
    assert.deepEqual(run(binary, ["analyze", "two words"]), { status: 3, signal: null });
    assert.equal(readFileSync(out, "utf8"), "analyze\ntwo words\n");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("reports a binary that cannot start", () => {
  assert.throws(
    () => run(join(tmpdir(), "repodna-no-such-binary"), []),
    (error) => error instanceof LauncherError && error.message.startsWith("Could not start"),
  );
});
