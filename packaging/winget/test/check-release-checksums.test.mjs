import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { checkReleaseChecksums } from "../check-release-checksums.mjs";

const version = "9.9.9";
const names = [
  `repodna-${version}-x86_64-pc-windows-msvc.zip`,
  `repodna-${version}-aarch64-pc-windows-msvc.zip`,
  `RepoDNA_${version}_x64_en-US.msi`,
  `RepoDNA_${version}_arm64_en-US.msi`,
];

function fixtures(t) {
  const dir = mkdtempSync(join(tmpdir(), "repodna-sums-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const lines = names.map((name) => {
    const content = "asset " + name;
    writeFileSync(join(dir, name), content);
    return createHash("sha256").update(content).digest("hex") + "  " + name;
  });
  writeFileSync(join(dir, "SHA256SUMS.txt"), lines.join("\n") + "\n");
  return { dir, lines };
}

test("accepts checksums for all required Windows release artifacts", async (t) => {
  const { dir } = fixtures(t);
  await checkReleaseChecksums(dir, version);
});

test("rejects a release manifest missing an installer checksum", async (t) => {
  const { dir, lines } = fixtures(t);
  writeFileSync(join(dir, "SHA256SUMS.txt"), lines.slice(1).join("\n") + "\n");
  await assert.rejects(checkReleaseChecksums(dir, version), /Missing published checksum/);
});

test("rejects mismatching SHA256 checksums for an installer", async (t) => {
  const { dir } = fixtures(t);
  writeFileSync(join(dir, names[2]), "changed installer");
  await assert.rejects(checkReleaseChecksums(dir, version), /Checksum mismatch/);
});
