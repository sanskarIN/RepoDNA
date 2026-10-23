#!/usr/bin/env node
// Validate the complete Windows installer inventory in the final release artifacts.
// This also runs for unpublished release trials, before any registries are updated.
import { statSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** Every Windows asset documented as part of the same release. */
export function windowsAssets(version) {
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) {
    throw new Error("Expected a stable X.Y.Z release version.");
  }
  return [
    `repodna-${version}-x86_64-pc-windows-msvc.zip`,
    `repodna-${version}-aarch64-pc-windows-msvc.zip`,
    `RepoDNA_${version}_x64_en-US.msi`,
    `RepoDNA_${version}_arm64_en-US.msi`,
    `RepoDNA_${version}_x64-setup.exe`,
    `RepoDNA_${version}_arm64-setup.exe`,
    `repodna-${version}-winget-manifests.zip`,
  ];
}

export function verifyWindowsAssets(directory, version) {
  const required = windowsAssets(version);
  for (const filename of required) {
    let size = 0;
    try {
      const stats = statSync(join(directory, filename));
      size = stats.isFile() ? stats.size : 0;
    } catch {
      // Report the missing asset by its documented release filename below.
    }
    if (size <= 0) {
      throw new Error("Missing or empty Windows release artifact: " + filename);
    }
  }
  return required;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const found = verifyWindowsAssets(process.argv[2], process.argv[3]);
    process.stdout.write("Verified " + found.length + " Windows release artifacts.\n");
  } catch (error) {
    process.stderr.write((error instanceof Error ? error.message : String(error)) + "\n");
    process.exitCode = 1;
  }
}
