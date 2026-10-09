#!/usr/bin/env node
// Stages RepoDNA's npm packages, for npmjs.com and GitHub Packages:
//
//   @sanskarin/repodna-schema         artifact types and helpers (packages/schema)
//   @sanskarin/repodna-visualization  chart geometry (packages/visualization)
//   @sanskarin/repodna-web            the web interface, built, with a server to run it
//   @sanskarin/repodna                the command line: a launcher that runs the binary
//   @sanskarin/repodna-<os>-<cpu>     the binary for one platform
//
// Usage: node packaging/npm/build.mjs [--version 1.2.0] [--out target/npm] [--binaries DIR]
//                                     [--web DIR]
//
// The web interface is staged only with --web, the directory of the built interface
// (apps/web/dist, from `npm run build -w @repodna/web`). The command line packages are
// staged only with --binaries, a directory that holds <target>/repodna (repodna.exe on
// Windows) for every target in launcher/lib/platforms.js.
// Each package is written to <out>/<name without the scope>, ready for publish.mjs, which
// chooses the registry.

import { spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { PLATFORMS, SCOPE, platformPackage } from "./launcher/lib/platforms.js";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../..");
const REPOSITORY = "https://github.com/sanskarIN/RepoDNA";
const OS_LABELS = { linux: "Linux", darwin: "macOS", win32: "Windows" };
const KEYWORDS = ["repodna", "repository", "code-analysis", "architecture", "git-history"];

const LIBRARIES = [
  {
    source: "packages/schema",
    name: "repodna-schema",
    readme: "schema.md",
    schemas: ["repodna-artifact.schema.json", "repodna-config.schema.json"],
  },
  { source: "packages/visualization", name: "repodna-visualization", readme: "visualization.md" },
];

function usage(message) {
  process.stderr.write(
    `${message}\nUsage: node packaging/npm/build.mjs [--version X.Y.Z] [--out DIR] [--binaries DIR] [--web DIR]\n`,
  );
  process.exit(2);
}

function parseArgs(argv) {
  const options = {
    version: JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version,
    out: join(root, "target", "npm"),
    binaries: null,
    web: null,
  };
  for (let index = 0; index < argv.length; index += 2) {
    const [flag, value] = [argv[index], argv[index + 1]];
    if (value === undefined) {
      usage(`${flag} needs a value.`);
    }
    if (flag === "--version") {
      options.version = value;
    } else if (flag === "--out") {
      options.out = resolve(value);
    } else if (flag === "--binaries") {
      options.binaries = resolve(value);
    } else if (flag === "--web") {
      options.web = resolve(value);
    } else {
      usage(`Unknown option ${flag}.`);
    }
  }
  if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(options.version)) {
    usage(`${options.version} is not a version such as 1.2.0.`);
  }
  return options;
}

/** The fields every package shares. */
function manifest(name, description, directory, version) {
  return {
    name,
    version,
    description,
    license: "Apache-2.0",
    author: "Sanskar (https://sanskarin.github.io)",
    homepage: REPOSITORY,
    repository: { type: "git", url: `git+${REPOSITORY}.git`, directory },
    bugs: `${REPOSITORY}/issues`,
    keywords: KEYWORDS,
  };
}

function writeJson(path, value) {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

function copyLegal(dir, { notices = false } = {}) {
  for (const file of ["LICENSE", "NOTICE", ...(notices ? ["THIRD-PARTY-NOTICES.txt"] : [])]) {
    copyFileSync(join(root, file), join(dir, file));
  }
}

function writeReadme(dir, template, values = {}) {
  const text = readFileSync(join(here, "readme", template), "utf8").replace(
    /\{\{(\w+)\}\}/g,
    (_, key) => values[key] ?? "",
  );
  writeFileSync(join(dir, "README.md"), text);
}

/** Copies the directory `from` into `to`, files and subdirectories alike. */
function copyTree(from, to) {
  mkdirSync(to, { recursive: true });
  for (const entry of readdirSync(from, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      copyTree(join(from, entry.name), join(to, entry.name));
    } else if (entry.isFile()) {
      copyFileSync(join(from, entry.name), join(to, entry.name));
    }
  }
}

function files(dir) {
  return readdirSync(dir).flatMap((entry) => {
    const path = join(dir, entry);
    return statSync(path).isDirectory() ? files(path) : [path];
  });
}

/**
 * Adds `.js` to relative import and export specifiers. The sources omit it, as bundlers
 * allow, but Node needs it to load ES modules and TypeScript to follow the declarations.
 */
export function addExtensions(dir) {
  for (const file of files(dir).filter((path) => /\.(js|d\.ts)$/.test(path))) {
    const text = readFileSync(file, "utf8");
    const fixed = text.replace(
      /((?:\bfrom|\bimport)\s*\(?\s*["'])(\.\.?\/[^"']+)(["'])/g,
      (match, before, specifier, after) => {
        if (/\.(c|m)?js$|\.json$/.test(specifier)) {
          return match;
        }
        const target = join(dirname(file), specifier);
        if (existsSync(`${target}.js`) || existsSync(`${target}.d.ts`)) {
          return `${before}${specifier}.js${after}`;
        }
        if (existsSync(join(target, "index.js"))) {
          return `${before}${specifier}/index.js${after}`;
        }
        throw new Error(`${file}: cannot find the module "${specifier}".`);
      },
    );
    if (fixed !== text) {
      writeFileSync(file, fixed);
    }
  }
}

function compile(source, out) {
  const tsc = join(root, "node_modules", "typescript", "bin", "tsc");
  const result = spawnSync(
    process.execPath,
    [
      tsc,
      join(source, "src", "index.ts"),
      ...["--outDir", out, "--rootDir", join(source, "src"), "--declaration"],
      ...["--target", "ES2022", "--module", "ESNext", "--moduleResolution", "Bundler"],
      ...["--lib", "ES2022,DOM", "--newLine", "lf", "--strict", "--skipLibCheck"],
      ...["--verbatimModuleSyntax", "--isolatedModules"],
    ],
    { stdio: "inherit" },
  );
  if (result.status !== 0) {
    throw new Error(`TypeScript could not compile ${source}.`);
  }
  addExtensions(out);
}

function stageLibrary(library, options) {
  const dir = join(options.out, library.name);
  const source = join(root, library.source);
  const description = JSON.parse(readFileSync(join(source, "package.json"), "utf8")).description;
  compile(source, join(dir, "dist"));
  const exports = { ".": { types: "./dist/index.d.ts", default: "./dist/index.js" } };
  for (const schema of library.schemas ?? []) {
    mkdirSync(join(dir, "schemas"), { recursive: true });
    copyFileSync(join(root, "schemas", schema), join(dir, "schemas", schema));
    exports[`./${schema.replace(/^repodna-/, "")}`] = `./schemas/${schema}`;
  }
  exports["./package.json"] = "./package.json";
  writeJson(join(dir, "package.json"), {
    ...manifest(`${SCOPE}/${library.name}`, description, library.source, options.version),
    type: "module",
    main: "./dist/index.js",
    types: "./dist/index.d.ts",
    exports,
    sideEffects: false,
    files: ["dist", ...(library.schemas ? ["schemas"] : []), "README.md", "LICENSE", "NOTICE"],
  });
  writeReadme(dir, library.readme);
  copyLegal(dir);
  return dir;
}

function stagePlatform(platform, options) {
  const label = `${OS_LABELS[platform.os]} ${platform.cpu}`;
  const dir = join(options.out, `repodna-${platform.os}-${platform.cpu}`);
  const binary = join(options.binaries, platform.target, platform.binary);
  if (!existsSync(binary)) {
    throw new Error(`${binary} is missing.`);
  }
  mkdirSync(join(dir, "bin"), { recursive: true });
  copyFileSync(binary, join(dir, "bin", platform.binary));
  chmodSync(join(dir, "bin", platform.binary), 0o755);
  writeJson(join(dir, "package.json"), {
    ...manifest(
      platformPackage(platform),
      `The repodna command line for ${label}. Install @sanskarin/repodna, which uses it.`,
      "packaging/npm",
      options.version,
    ),
    os: [platform.os],
    cpu: [platform.cpu],
    files: ["bin", "README.md", "LICENSE", "NOTICE", "THIRD-PARTY-NOTICES.txt"],
    preferUnplugged: true,
  });
  writeReadme(dir, "platform.md", { label, target: platform.target });
  copyLegal(dir, { notices: true });
  return dir;
}

function stageWeb(options) {
  if (!existsSync(join(options.web, "index.html"))) {
    throw new Error(`${options.web} holds no built web interface (index.html is missing).`);
  }
  const dir = join(options.out, "repodna-web");
  copyTree(options.web, join(dir, "dist"));
  for (const part of ["bin", "lib"]) {
    copyTree(join(here, "web", part), join(dir, part));
  }
  chmodSync(join(dir, "bin", "repodna-web.js"), 0o755);
  writeJson(join(dir, "package.json"), {
    ...manifest(
      `${SCOPE}/repodna-web`,
      "RepoDNA's web interface, built: open analyses and the demo in your browser, offline or on your own network.",
      "apps/web",
      options.version,
    ),
    type: "module",
    bin: { "repodna-web": "bin/repodna-web.js" },
    main: "./lib/index.js",
    exports: { ".": "./lib/index.js", "./package.json": "./package.json" },
    files: ["bin", "lib", "dist", "README.md", "LICENSE", "NOTICE"],
    engines: { node: ">=18" },
  });
  writeReadme(dir, "web.md");
  copyLegal(dir);
  return dir;
}

function stageLauncher(options) {
  const dir = join(options.out, "repodna");
  for (const part of ["bin", "lib"]) {
    mkdirSync(join(dir, part), { recursive: true });
    for (const file of readdirSync(join(here, "launcher", part))) {
      copyFileSync(join(here, "launcher", part, file), join(dir, part, file));
    }
  }
  chmodSync(join(dir, "bin", "repodna.js"), 0o755);
  writeJson(join(dir, "package.json"), {
    ...manifest(
      `${SCOPE}/repodna`,
      "Local-first repository intelligence and code archaeology: the repodna command line.",
      "packaging/npm",
      options.version,
    ),
    type: "module",
    bin: { repodna: "bin/repodna.js" },
    files: ["bin", "lib", "README.md", "LICENSE", "NOTICE"],
    engines: { node: ">=18" },
    optionalDependencies: Object.fromEntries(
      PLATFORMS.map((platform) => [platformPackage(platform), options.version]),
    ),
  });
  writeReadme(dir, "repodna.md");
  copyLegal(dir);
  return dir;
}

export function build(options) {
  rmSync(options.out, { recursive: true, force: true });
  mkdirSync(options.out, { recursive: true });
  const staged = LIBRARIES.map((library) => stageLibrary(library, options));
  if (options.web) {
    staged.push(stageWeb(options));
  }
  if (options.binaries) {
    staged.push(...PLATFORMS.map((platform) => stagePlatform(platform, options)));
    staged.push(stageLauncher(options));
  }
  return staged;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const options = parseArgs(process.argv.slice(2));
  for (const dir of build(options)) {
    const { name, version } = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
    process.stdout.write(`${name}@${version}  ${dir}\n`);
  }
}
