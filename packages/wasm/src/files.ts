// The files the program reads, in a small in-memory file system (WASI). Each file is read
// only when the program opens it, so that a large folder is not read before it is needed.

import { Directory, File as WasiFile, type Inode } from "@bjorn3/browser_wasi_shim";
import { keep } from "./paths";

/** A file the program may read: its size, and how to read its bytes when they are needed. */
export interface LazyFile {
  readonly size: number;
  read(): Uint8Array;
}

/** A file of a folder, at its path from the folder, with `/` between the parts. */
export interface FolderFile extends LazyFile {
  readonly path: string;
}

/** Where the program finds its input. */
export const WORK = "/work";

/** A read-only file whose bytes are read the first time the program needs them. */
class OnDemandFile extends WasiFile {
  constructor(file: LazyFile) {
    super(new Uint8Array(0), { readonly: true });
    let bytes: Uint8Array | null = null;
    Object.defineProperty(this, "data", {
      get: () => (bytes ??= file.read()),
      set: (value: Uint8Array) => {
        bytes = value;
      },
    });
    // Listing a folder asks for sizes; they are known without reading the file.
    Object.defineProperty(this, "size", {
      get: () => BigInt(bytes ? bytes.byteLength : file.size),
    });
  }
}

type Tree = Map<string, Tree | LazyFile>;

function directory(tree: Tree): Directory {
  const entries = new Map<string, Inode>();
  for (const [name, child] of tree) {
    entries.set(name, child instanceof Map ? directory(child) : new OnDemandFile(child));
  }
  // The constructor links the directories it is given to their parent.
  return new Directory(entries);
}

function root(work: Tree): Map<string, Inode> {
  return new Map<string, Inode>([
    [WORK.slice(1), directory(work)],
    ["tmp", new Directory(new Map())],
  ]);
}

/**
 * The file system for an analysis of the folder `name`: its files under `/work/<name>`,
 * and an empty `/tmp`. Paths that leave the folder and version-control directories are
 * left out.
 */
export function folderFileSystem(name: string, files: Iterable<FolderFile>): Map<string, Inode> {
  const top: Tree = new Map();
  for (const file of files) {
    const parts = file.path.split("/").filter((part) => part !== "" && part !== ".");
    const fileName = parts.pop();
    if (!fileName || parts.includes("..") || fileName === ".." || !keep(file.path)) {
      continue;
    }
    let current = top;
    for (const part of parts) {
      let child = current.get(part);
      if (!(child instanceof Map)) {
        child = new Map();
        current.set(part, child);
      }
      current = child;
    }
    current.set(fileName, file);
  }
  return root(new Map([[name, top]]));
}

/** The file system for one file, such as an archive or an analysis, under `/work`. */
export function fileFileSystem(name: string, file: LazyFile): Map<string, Inode> {
  return root(new Map([[name, file]]));
}

/** A file whose bytes are already in memory. */
export function bytesFile(bytes: Uint8Array): LazyFile {
  return { size: bytes.byteLength, read: () => bytes };
}
