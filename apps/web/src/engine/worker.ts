// The worker that runs RepoDNA's WebAssembly program, so that an analysis does not stop the
// page from responding. The page sends one request at a time.

import { RepoDNA, type LazyFile } from "@repodna/wasm";
import type { EngineReply, EngineRequest } from "./protocol";

declare const FileReaderSync: new () => { readAsArrayBuffer(blob: Blob): ArrayBuffer };

const reader = new FileReaderSync();
const encoder = new TextEncoder();

/** A file the page gave, read when the program opens it. */
function lazy(blob: Blob): LazyFile {
  return { size: blob.size, read: () => new Uint8Array(reader.readAsArrayBuffer(blob)) };
}

/** The program, downloaded and compiled once for every request that follows. */
let program: { url: string; ready: Promise<RepoDNA> } | null = null;

function load(url: string): Promise<RepoDNA> {
  if (program?.url !== url) {
    const ready = RepoDNA.load(fetch(url));
    program = { url, ready };
    // Try again on the next request after a failure, such as a lost connection.
    ready.catch(() => {
      if (program?.ready === ready) {
        program = null;
      }
    });
  }
  return program.ready;
}

/** What the program makes for a request: text, or the bytes of an image. */
async function perform(
  repodna: RepoDNA,
  request: EngineRequest,
  onProgress: Parameters<RepoDNA["analyzeFolder"]>[2],
): Promise<string | Uint8Array<ArrayBuffer>> {
  switch (request.kind) {
    case "analyze-folder":
      return repodna.analyzeFolder(
        request.name,
        request.files.map(({ path, file }) => ({ path, ...lazy(file) })),
        { profile: request.profile, ...onProgress },
      );
    case "analyze-archive":
      return repodna.analyzeArchive(request.archive.name, lazy(request.archive), {
        profile: request.profile,
        ...onProgress,
      });
    case "report":
      return repodna.report(request.artifact, request);
    case "card":
      return request.png
        ? repodna.cardPng(request.artifact, request)
        : repodna.card(request.artifact, request);
  }
}

function reply(message: EngineReply, transfer: Transferable[] = []): void {
  self.postMessage(message, { transfer });
}

self.onmessage = async (event: MessageEvent<EngineRequest>) => {
  const request = event.data;
  const { id } = request;
  try {
    const repodna = await load(request.wasm);
    const output = await perform(repodna, request, {
      onProgress: (progress) => reply({ id, kind: "progress", progress }),
    });
    const size = typeof output === "string" ? encoder.encode(output).byteLength : output.byteLength;
    reply({ id, kind: "done", output, size }, typeof output === "string" ? [] : [output.buffer]);
  } catch (error) {
    reply({ id, kind: "failed", message: error instanceof Error ? error.message : String(error) });
  }
};
