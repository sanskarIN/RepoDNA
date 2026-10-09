#!/usr/bin/env node
// Publishes the packages that build.mjs staged to one npm registry, npmjs.com or GitHub
// Packages. A version that is already on the registry is skipped, so a release can be
// published again, or published to the other registry later. A prerelease goes under the
// `next` tag and a release older than the registry's latest under `previous`, so that
// `npm install` keeps picking the newest release.
//
// Usage: node packaging/npm/publish.mjs --registry URL DIR [-- npm publish options]
//
// The platform packages go first, then the launcher, which names them, and the libraries
// and the web interface last. So the launcher never names a platform package that is not
// there yet, and a package the registry refuses, such as a new one that a token may not
// create, cannot keep the command line from being published. The token comes from
// NODE_AUTH_TOKEN; on GitHub Actions, npm 11.5.1 or later can use a trusted publisher set
// up on npmjs.com instead.

import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { PLATFORMS, SCOPE, platformPackage } from "./launcher/lib/platforms.js";

const LAUNCHER = `${SCOPE}/repodna`;

function usage(message) {
  process.stderr.write(
    `${message}\nUsage: node packaging/npm/publish.mjs --registry URL DIR [-- npm publish options]\n`,
  );
  process.exit(2);
}

function parseArgs(argv) {
  const split = argv.indexOf("--");
  const own = split === -1 ? argv : argv.slice(0, split);
  const extra = split === -1 ? [] : argv.slice(split + 1);
  let registry = null;
  let dir = null;
  for (let index = 0; index < own.length; index += 1) {
    if (own[index] === "--registry") {
      registry = own[index + 1] ?? usage("--registry needs a value.");
      index += 1;
    } else if (own[index].startsWith("-")) {
      usage(`Unknown option ${own[index]}.`);
    } else if (dir === null) {
      dir = resolve(own[index]);
    } else {
      usage("Give one directory of staged packages.");
    }
  }
  if (!registry || !dir) {
    usage("The registry and the directory are both needed.");
  }
  return { registry, dir, extra };
}

/** The registry address with a trailing slash, as npm configurations write it. */
function normalize(registry) {
  const url = new URL(registry);
  return `${url.origin}${url.pathname.replace(/\/?$/, "/")}`;
}

/** An npm configuration that sends the scope, and the token if there is one, to the registry. */
export function npmrc(registry, withToken) {
  const address = normalize(registry);
  const lines = [`registry=${address}`, `${SCOPE}:registry=${address}`];
  if (withToken) {
    lines.push(`${address.replace(/^https?:/, "")}:_authToken=\${NODE_AUTH_TOKEN}`);
  }
  return `${lines.join("\n")}\n`;
}

/** The staged packages in `dir` as `{ dir, name, version }`, in publishing order: the
 *  platform packages, the launcher, then the others. */
export function publishOrder(dir) {
  const packages = readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && existsSync(join(dir, entry.name, "package.json")))
    .map((entry) => {
      const path = join(dir, entry.name);
      const { name, version } = JSON.parse(readFileSync(join(path, "package.json"), "utf8"));
      return { dir: path, name, version };
    })
    .sort((a, b) => a.name.localeCompare(b.name));
  const platforms = new Set(PLATFORMS.map(platformPackage));
  return [
    ...packages.filter((pkg) => platforms.has(pkg.name)),
    ...packages.filter((pkg) => pkg.name === LAUNCHER),
    ...packages.filter((pkg) => pkg.name !== LAUNCHER && !platforms.has(pkg.name)),
  ];
}

/** Whether release version `a`, such as 1.2.0, comes before release version `b`. */
export function isOlder(a, b) {
  const parse = (version) => version.split("-")[0].split(".").map(Number);
  const [x, y] = [parse(a), parse(b)];
  for (let index = 0; index < 3; index += 1) {
    if (x[index] !== y[index]) {
      return x[index] < y[index];
    }
  }
  return false;
}

/** The version the registry's `latest` tag names, or null for a package it does not have. */
function latest(npm, config, name) {
  const result = spawnSync(npm, ["view", name, "dist-tags.latest", ...config], {
    encoding: "utf8",
  });
  if (result.status === 0) {
    return result.stdout.trim() || null;
  }
  if (/\bE404\b/.test(result.stderr)) {
    return null;
  }
  throw new Error(
    `Could not read the latest version of ${name}:\n${result.stderr || result.stdout}`,
  );
}

/** Whether `name@version` is on the registry; throws when the registry cannot tell. */
function published(npm, config, pkg) {
  const result = spawnSync(npm, ["view", `${pkg.name}@${pkg.version}`, "version", ...config], {
    encoding: "utf8",
  });
  if (result.status === 0 && result.stdout.trim() === pkg.version) {
    return true;
  }
  if (result.status !== 0 && /\bE404\b/.test(result.stderr)) {
    return false;
  }
  throw new Error(
    `Could not check whether ${pkg.name}@${pkg.version} is published:\n${result.stderr || result.stdout}`,
  );
}

/**
 * Publishes every staged package in `dir` that the registry does not have yet, and returns
 * the names and versions it published. `extra` goes to each `npm publish`.
 */
export function publish({
  registry,
  dir,
  extra = [],
  npm = process.env.REPODNA_NPM || "npm",
  log = (line) => process.stdout.write(`${line}\n`),
}) {
  const home = mkdtempSync(join(tmpdir(), "repodna-publish-"));
  try {
    const userconfig = join(home, ".npmrc");
    writeFileSync(userconfig, npmrc(registry, Boolean(process.env.NODE_AUTH_TOKEN)));
    const config = ["--userconfig", userconfig, "--registry", normalize(registry)];
    const done = [];
    for (const pkg of publishOrder(dir)) {
      if (published(npm, config, pkg)) {
        log(`${pkg.name}@${pkg.version} is already on ${normalize(registry)}.`);
        continue;
      }
      let tag = [];
      if (!extra.includes("--tag")) {
        if (pkg.version.includes("-")) {
          // A prerelease such as 1.3.0-rc.1 must not become what `npm install` picks,
          tag = ["--tag", "next"];
        } else {
          // nor must a release older than the latest, such as 1.2.0 published after 1.2.1.
          const current = latest(npm, config, pkg.name);
          if (current && isOlder(pkg.version, current)) {
            tag = ["--tag", "previous"];
          }
        }
      }
      const result = spawnSync(npm, ["publish", pkg.dir, ...config, ...tag, ...extra], {
        stdio: "inherit",
      });
      if (result.status !== 0) {
        throw new Error(`npm could not publish ${pkg.name}@${pkg.version}.`);
      }
      done.push(`${pkg.name}@${pkg.version}`);
    }
    return done;
  } finally {
    rmSync(home, { recursive: true, force: true });
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const options = parseArgs(process.argv.slice(2));
  try {
    const done = publish(options);
    const verb = options.extra.includes("--dry-run") ? "Would publish" : "Published";
    process.stdout.write(
      done.length > 0 ? `${verb} ${done.join(", ")}.\n` : "Nothing new to publish.\n",
    );
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exit(1);
  }
}
