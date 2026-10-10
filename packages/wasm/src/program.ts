// Running RepoDNA's WebAssembly program once, on a file system made in memory.

import {
  ConsoleStdout,
  File as WasiFile,
  OpenFile,
  PreopenDirectory,
  WASI,
  type Inode,
} from "@bjorn3/browser_wasi_shim";
import { WORK } from "./files";

/** Progress of an analysis, as the program reports it. */
export type ProgressEvent =
  | { event: "stage"; stage: string; status: string; milliseconds?: number }
  | { event: "files"; done: number; total: number }
  | { event: "message"; text: string };

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
export async function run(
  module: WebAssembly.Module,
  args: readonly string[],
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
    // The library logs every file lookup unless told not to.
    { debug: false },
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
    // Names files as the caller knows them, without the directory they were put in here.
    throw new Error(message.replaceAll(`${WORK}/`, ""));
  }
  return concat(output);
}
