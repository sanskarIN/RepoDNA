// RepoDNA's WebAssembly program for Node.js: analyze a folder or an archive on disk, and
// make reports and cards, on any system Node.js runs on and without a native program.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { RepoDNA, type AnalyzeOptions, type CardOptions, type FolderFile } from "./index";
import type { ReportOptions } from "./index";
import { keep } from "./paths";

export * from "./index";

/** The program that comes with this package. `REPODNA_WASM` can name another one. */
export function programPath(): string {
  return process.env.REPODNA_WASM ?? fileURLToPath(new URL("../repodna.wasm", import.meta.url));
}

const loaded = new Map<string, Promise<RepoDNA>>();

/** The program, compiled once. */
export function load(path: string = programPath()): Promise<RepoDNA> {
  let program = loaded.get(path);
  if (!program) {
    program = RepoDNA.load(readFileSync(path));
    loaded.set(path, program);
    program.catch(() => loaded.delete(path));
  }
  return program;
}

/**
 * The files of the folder `root`, with their paths from it. Each is read only when the
 * analysis opens it. Symbolic links are not followed, and version-control directories are
 * left out.
 */
export function folderFiles(root: string): FolderFile[] {
  const files: FolderFile[] = [];
  const walk = (directory: string, prefix: string): void => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = prefix + entry.name;
      const full = join(directory, entry.name);
      if (!keep(path)) {
        continue;
      }
      if (entry.isDirectory()) {
        walk(full, `${path}/`);
      } else if (entry.isFile()) {
        let size: number | undefined;
        files.push({
          path,
          get size() {
            return (size ??= statSync(full).size);
          },
          read: () => readFileSync(full),
        });
      }
    }
  };
  walk(root, "");
  return files;
}

const ARCHIVE = /\.(zip|tar|tar\.gz|tgz)$/i;

/**
 * Analyzes a folder, or a `.zip`, `.tar`, `.tar.gz`, or `.tgz` archive. Resolves to the
 * analysis as JSON text, the contents of a `.repodna` file.
 */
export async function analyze(path: string, options: AnalyzeOptions = {}): Promise<string> {
  const target = resolve(path);
  const stat = statSync(target);
  const program = await load();
  if (stat.isDirectory()) {
    return program.analyzeFolder(basename(target), folderFiles(target), options);
  }
  if (stat.isFile() && ARCHIVE.test(target)) {
    const archive = { size: stat.size, read: () => readFileSync(target) };
    return program.analyzeArchive(basename(target), archive, options);
  }
  throw new Error(`${path} is neither a folder nor a .zip, .tar, .tar.gz, or .tgz archive.`);
}

/** A report of an analysis (its JSON text, or the parsed object). */
export async function report(artifact: string | object, options?: ReportOptions): Promise<string> {
  return (await load()).report(artifact, options);
}

/** The Project DNA card of an analysis as SVG markup. */
export async function card(artifact: string | object, options?: CardOptions): Promise<string> {
  return (await load()).card(artifact, options);
}

/** The Project DNA card of an analysis as a PNG image. */
export async function cardPng(
  artifact: string | object,
  options?: CardOptions,
): Promise<Uint8Array<ArrayBuffer>> {
  return (await load()).cardPng(artifact, options);
}
