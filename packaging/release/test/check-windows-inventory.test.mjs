import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { verifyWindowsAssets, windowsAssets } from "../check-windows-inventory.mjs";

const version = "1.4.3";

function ready(t) {
  const root = mkdtempSync(join(tmpdir(), "repodna-release-assets-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const name of windowsAssets(version)) {
    writeFileSync(join(root, name), "an installer fixture");
  }
  return root;
}

test("a complete set of Windows release artifacts passes preflight", (t) => {
  const root = ready(t);
  assert.equal(verifyWindowsAssets(root, version).length, 7);
});

test("rejects a missing Windows Arm setup executable", (t) => {
  const root = ready(t);
  rmSync(join(root, `RepoDNA_${version}_arm64-setup.exe`));
  assert.throws(() => verifyWindowsAssets(root, version), /Missing or empty.*arm64-setup.exe/);
});

test("rejects empty installer output and directories that look like files", (t) => {
  const root = ready(t);
  const exe = join(root, `RepoDNA_${version}_x64-setup.exe`);
  writeFileSync(exe, "");
  assert.throws(() => verifyWindowsAssets(root, version), /Missing or empty.*x64-setup.exe/);
  rmSync(exe);
  mkdirSync(exe);
  assert.throws(() => verifyWindowsAssets(root, version), /Missing or empty.*x64-setup.exe/);
});

test("rejects malformed release version strings", (t) => {
  const root = ready(t);
  for (const invalid of ["v1.4.3", "1.4.3-rc.1", "01.4.3", "../1.4.3"]) {
    assert.throws(() => verifyWindowsAssets(root, invalid), /stable X.Y.Z release version/);
  }
});
