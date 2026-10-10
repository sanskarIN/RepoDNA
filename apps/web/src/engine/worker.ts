// The worker that runs RepoDNA's WebAssembly program, so that an analysis does not stop the
// page from responding. The page sends one request at a time.

import type { Inode } from "@bjorn3/browser_wasi_shim";
import type { EngineReply, EngineRequest } from "./protocol";
import { fileRoot, folderRoot, runProgram, WORK, type ReadBlob } from "./run";

declare const FileReaderSync: new () => { readAsArrayBuffer(blob: Blob): ArrayBuffer };

const reader = new FileReaderSync();
const read: ReadBlob = (blob) => new Uint8Array(reader.readAsArrayBuffer(blob));

/** The program, downloaded and compiled once for every request that follows. */
let program: { url: string; module: Promise<WebAssembly.Module> } | null = null;

async function download(url: string): Promise<WebAssembly.Module> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`RepoDNA's analysis program could not be loaded (HTTP ${response.status}).`);
  }
  return WebAssembly.compile(await response.arrayBuffer());
}

function compile(url: string): Promise<WebAssembly.Module> {
  if (program?.url !== url) {
    const module = download(url);
    program = { url, module };
    // Try again on the next request after a failure, such as a lost connection.
    module.catch(() => {
      if (program?.module === module) {
        program = null;
      }
    });
  }
  return program.module;
}

/** The arguments and the files for a request. */
function task(request: EngineRequest): [string[], Map<string, Inode>] {
  switch (request.kind) {
    case "analyze-folder":
      return [
        ["analyze", `${WORK}/${request.name}`, "--profile", request.profile],
        folderRoot(request.name, request.files, read),
      ];
    case "analyze-archive":
      return [
        ["analyze", `${WORK}/${request.archive.name}`, "--profile", request.profile],
        fileRoot(request.archive.name, request.archive, read),
      ];
    case "report": {
      const args = ["report", `${WORK}/analysis.repodna`, "--format", request.format];
      args.push("--theme", request.theme, "--privacy", request.privacy);
      return [args, fileRoot("analysis.repodna", new Blob([request.artifact]), read)];
    }
    case "card": {
      const args = ["card", `${WORK}/analysis.repodna`];
      if (request.dark) {
        args.push("--dark");
      }
      if (request.png) {
        args.push("--png");
      }
      return [args, fileRoot("analysis.repodna", new Blob([request.artifact]), read)];
    }
  }
}

function reply(message: EngineReply, transfer: Transferable[] = []): void {
  self.postMessage(message, { transfer });
}

self.onmessage = async (event: MessageEvent<EngineRequest>) => {
  const request = event.data;
  const { id } = request;
  try {
    const module = await compile(request.wasm);
    const [args, files] = task(request);
    const output = await runProgram(module, args, files, (progress) =>
      reply({ id, kind: "progress", progress }),
    );
    reply({ id, kind: "done", output }, [output.buffer]);
  } catch (error) {
    reply({ id, kind: "failed", message: error instanceof Error ? error.message : String(error) });
  }
};
