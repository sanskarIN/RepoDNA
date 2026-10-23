import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createReadStream, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

/** Check that every installer has a published checksum, not just files listed by SHA256SUMS. */
export async function checkReleaseChecksums(directory, version) {
  assert.match(version, /^\d+\.\d+\.\d+$/);
  const expected = [
    `repodna-${version}-x86_64-pc-windows-msvc.zip`,
    `repodna-${version}-aarch64-pc-windows-msvc.zip`,
    `RepoDNA_${version}_x64_en-US.msi`,
    `RepoDNA_${version}_arm64_en-US.msi`,
  ];
  const sums = new Map();
  for (const line of readFileSync(join(directory, "SHA256SUMS.txt"), "utf8").split(/\r?\n/)) {
    if (!line) continue;
    const match = /^([a-fA-F0-9]{64}) [ *](.+)$/.exec(line);
    if (!match) throw new Error("Malformed SHA256SUMS entry.");
    if (sums.has(match[2])) throw new Error("Duplicate checksum entry: " + match[2]);
    sums.set(match[2], match[1].toLowerCase());
  }
  for (const filename of expected) {
    const wanted = sums.get(filename);
    if (!wanted) throw new Error("Missing published checksum: " + filename);
    const hash = createHash("sha256");
    for await (const block of createReadStream(join(directory, filename))) hash.update(block);
    if (hash.digest("hex") !== wanted) throw new Error("Checksum mismatch: " + filename);
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  checkReleaseChecksums(process.argv[2], process.argv[3]).catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
