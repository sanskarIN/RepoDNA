// Finds the repodna binary that npm installed for this platform and runs it, passing the
// arguments, the standard streams, and the exit status through.

import { spawnSync } from "node:child_process";
import { chmodSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { findPlatform, platformPackage } from "./platforms.js";

const RELEASES = "https://github.com/sanskarIN/RepoDNA/releases";

/** A problem the user can act on; its message is shown without a stack trace. */
export class LauncherError extends Error {}

/**
 * The path of the binary for `os` and `cpu`. `resolve` finds a package's `package.json`
 * from the launcher, as `require.resolve` does, and throws when it is not installed.
 */
export function locate(os, cpu, resolve) {
  const platform = findPlatform(os, cpu);
  if (!platform) {
    throw new LauncherError(
      `RepoDNA has no npm package for ${os}-${cpu}. Download a build from ${RELEASES}.`,
    );
  }
  const name = platformPackage(platform);
  let manifest;
  try {
    manifest = resolve(`${name}/package.json`);
  } catch {
    throw new LauncherError(
      `The package ${name}, which holds the repodna binary for this platform, is not installed. ` +
        "Install @sanskarin/repodna again without --omit=optional or --no-optional.",
    );
  }
  return join(dirname(manifest), "bin", platform.binary);
}

/** Runs `binary` with `args` and returns its exit status, or the signal that ended it. */
export function run(binary, args) {
  let result = spawnSync(binary, args, { stdio: "inherit" });
  if (result.error && result.error.code === "EACCES") {
    // Some installers drop the executable bit; restore it once and try again.
    try {
      chmodSync(binary, 0o755);
    } catch {
      // Reported below with the original error.
    }
    result = spawnSync(binary, args, { stdio: "inherit" });
  }
  if (result.error) {
    throw new LauncherError(`Could not start ${binary}: ${result.error.message}`);
  }
  return { status: result.status, signal: result.signal };
}

/** The launcher's entry point. */
export function main(args) {
  const require = createRequire(import.meta.url);
  try {
    const { status, signal } = run(locate(process.platform, process.arch, require.resolve), args);
    if (signal) {
      // End the same way the binary ended, so callers see the signal.
      process.kill(process.pid, signal);
    }
    process.exitCode = status ?? 1;
  } catch (error) {
    if (!(error instanceof LauncherError)) {
      throw error;
    }
    process.stderr.write(`repodna: ${error.message}\n`);
    process.exitCode = 1;
  }
}
