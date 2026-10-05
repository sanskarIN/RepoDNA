#!/usr/bin/env node
// Stages RepoDNA's npm packages for GitHub Packages:
//
//   @sanskarin/repodna-schema         artifact types and helpers (packages/schema)
//   @sanskarin/repodna-visualization  chart geometry (packages/visualization)
//
// Usage: node packaging/npm/build.mjs [--version 1.2.0] [--out target/npm]
//
// Each package is written to <out>/<name without the scope>, ready for `npm publish`.

import { spawnSync } from "node:child_process";
import {
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
import { SCOPE } from "./launcher/lib/platforms.js";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../..");
const REPOSITORY = "https://github.com/sanskarIN/RepoDNA";
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
    `${message}\nUsage: node packaging/npm/build.mjs [--version X.Y.Z] [--out DIR]\n`,
  );
  process.exit(2);
}

function parseArgs(argv) {
  const options = {
    version: JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version,
    out: join(root, "target", "npm"),
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
    publishConfig: { registry: "https://npm.pkg.github.com" },
  };
}

function writeJson(path, value) {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

function copyLegal(dir) {
  for (const file of ["LICENSE", "NOTICE"]) {
    copyFileSync(join(root, file), join(dir, file));
  }
}

function writeReadme(dir, template) {
  copyFileSync(join(here, "readme", template), join(dir, "README.md"));
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

export function build(options) {
  rmSync(options.out, { recursive: true, force: true });
  mkdirSync(options.out, { recursive: true });
  return LIBRARIES.map((library) => stageLibrary(library, options));
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const options = parseArgs(process.argv.slice(2));
  for (const dir of build(options)) {
    const { name, version } = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
    process.stdout.write(`${name}@${version}  ${dir}\n`);
  }
}
