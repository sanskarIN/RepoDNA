// Running RepoDNA's WebAssembly program on files the page was given. A small in-memory
// file system (WASI) holds them, and each file is read only when the program opens it,
// so that a large folder is not read into memory before it is needed.

import {
  ConsoleStdout,
  Directory,
  File as WasiFile,
  OpenFile,
  PreopenDirectory,
  WASI,
  type Inode,
} from "@bjorn3/browser_wasi_shim";
import { keep } from "./input";
import type { InputFile, ProgressEvent } from "./protocol";

/** Reads a Blob at once, as `FileReaderSync` does in a worker. */
export type ReadBlob = (blob: Blob) => Uint8Array;

/** Where the program finds its input, and the directory for its temporary files. */
export const WORK = "/work";
const TEMPORARY = "tmp";

/** A read-only file whose bytes are read the first time the program needs them. */
class BlobFile extends WasiFile {
  constructor(blob: Blob, read: ReadBlob) {
    super(new Uint8Array(0), { readonly: true });
    let bytes: Uint8Array | null = null;
    Object.defineProperty(this, "data", {
      get: () => (bytes ??= read(blob)),
      set: (value: Uint8Array) => {
        bytes = value;
      },
    });
    // Listing a folder asks for sizes; they come from the Blob without reading it.
    Object.defineProperty(this, "size", {
      get: () => BigInt(bytes ? bytes.byteLength : blob.size),
    });
  }
}

type Tree = Map<string, Tree | Blob>;

function directory(tree: Tree, read: ReadBlob): Directory {
  const entries = new Map<string, Inode>();
  for (const [name, child] of tree) {
    entries.set(name, child instanceof Map ? directory(child, read) : new BlobFile(child, read));
  }
  // The constructor links the directories it is given to their parent.
  return new Directory(entries);
}

/**
 * The file system for an analysis of the folder `name`: its files under `/work/<name>`,
 * and an empty `/tmp`.
 */
export function folderRoot(name: string, files: InputFile[], read: ReadBlob): Map<string, Inode> {
  const top: Tree = new Map();
  for (const { path, file } of files) {
    const parts = path.split("/").filter((part) => part !== "" && part !== ".");
    const fileName = parts.pop();
    if (!fileName || parts.includes("..") || !keep(path)) {
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
  return root(new Map([[name, top]]), read);
}

/** The file system for one file, such as an archive or an analysis, under `/work`. */
export function fileRoot(name: string, file: Blob, read: ReadBlob): Map<string, Inode> {
  return root(new Map([[name, file]]), read);
}

function root(work: Tree, read: ReadBlob): Map<string, Inode> {
  return new Map<string, Inode>([
    [WORK.slice(1), directory(work, read)],
    [TEMPORARY, new Directory(new Map())],
  ]);
}

function concat(chunks: Uint8Array[]): Uint8Array<ArrayBuffer> {
  const output = new Uint8Array(chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0));
  let offset = 0;
  for (const chunk of chunks) {
    output.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return output;
}

/**
 * Runs the program with `args` on the file system `files`, and returns what it wrote to
 * standard output. Progress is reported as the program writes it; a failure is thrown
 * with the program's own explanation.
 */
export async function runProgram(
  module: WebAssembly.Module,
  args: string[],
  files: Map<string, Inode>,
  onProgress?: (event: ProgressEvent) => void,
): Promise<Uint8Array<ArrayBuffer>> {
  const output: Uint8Array[] = [];
  // Lines that are not progress, such as the message of a crash.
  const notes: string[] = [];
  let failure: string | null = null;
  const stderr = ConsoleStdout.lineBuffered((line) => {
    let parsed: unknown;
    try {
      parsed = JSON.parse(line);
    } catch {
      if (line.trim()) {
        notes.push(line.trim());
      }
      return;
    }
    const event = parsed as { event?: string; message?: string };
    if (event.event === "error") {
      failure = event.message ?? null;
    } else if (event.event) {
      onProgress?.(parsed as ProgressEvent);
    }
  });
  const argv = ["repodna-wasm", ...args];
  const wasi = new WASI(
    argv,
    [],
    [
      new OpenFile(new WasiFile(new Uint8Array(0))),
      new ConsoleStdout((data) => output.push(data.slice())),
      stderr,
      new PreopenDirectory("/", files),
    ],
  );
  // The library sizes the arguments by their length in UTF-16 units instead of UTF-8
  // bytes, which is too little for a folder named in, say, Hindi or Japanese.
  const encoder = new TextEncoder();
  wasi.wasiImport.args_sizes_get = (count: number, bytes: number) => {
    const view = new DataView(wasi.inst.exports.memory.buffer);
    view.setUint32(count, argv.length, true);
    view.setUint32(
      bytes,
      argv.reduce((sum, arg) => sum + encoder.encode(arg).byteLength + 1, 0),
      true,
    );
    return 0;
  };
  const instance = await WebAssembly.instantiate(module, {
    wasi_snapshot_preview1: wasi.wasiImport,
  });
  let code: number;
  try {
    code = wasi.start(instance as unknown as Parameters<WASI["start"]>[0]);
  } catch (error) {
    const detail = notes.join(" ") || (error instanceof Error ? error.message : String(error));
    throw new Error(`RepoDNA stopped unexpectedly: ${detail}`);
  }
  if (code !== 0) {
    const message = failure ?? (notes.join(" ") || `RepoDNA stopped with exit code ${code}.`);
    // Names files as the user knows them, without the directory they were put in here.
    throw new Error(message.replaceAll(`${WORK}/`, ""));
  }
  return concat(output);
}
