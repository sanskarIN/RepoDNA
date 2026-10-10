// RepoDNA's WebAssembly program, run by a worker on this page: analyses of a folder or an
// archive the user chooses, and reports and cards of an analysis, without a server. The
// files are read in the browser and never uploaded.

import type {
  EngineReply,
  EngineRequest,
  InputFile,
  ProgressEvent,
  ReportRequest,
} from "./protocol";

export type { InputFile, ProgressEvent, ReportRequest } from "./protocol";

/** Where the program and its description are, next to the page. */
const DIRECTORY = "engine/";

/** What `engine/engine.json` says about the program. */
export interface EngineInfo {
  version: string;
  bytes: number;
}

function isInfo(value: unknown): value is EngineInfo {
  const info = value as Partial<EngineInfo> | null;
  return typeof info?.version === "string" && typeof info.bytes === "number";
}

let info: Promise<EngineInfo | null> | null = null;
// The version from `engine.json`, which keeps a cached program of another version from
// being used.
let version = "";

/**
 * The program this build of the web interface comes with, or null when it has none: the
 * interface that `repodna serve` and the desktop app show analyzes on the machine instead.
 */
export function engineInfo(): Promise<EngineInfo | null> {
  info ??= fetch(new URL(`${DIRECTORY}engine.json`, document.baseURI), { cache: "no-cache" })
    .then((response) => (response.ok ? response.json() : null))
    .then((value: unknown) => {
      if (!isInfo(value)) {
        return null;
      }
      version = value.version;
      return value;
    })
    .catch(() => null);
  return info;
}

/** The error of a request that was cancelled. */
export class CancelledError extends Error {
  override name = "CancelledError";
  constructor() {
    super("Cancelled.");
  }
}

/** A request in progress, and the way to stop it. */
export interface EngineTask<T> {
  result: Promise<T>;
  cancel(): void;
}

interface Pending {
  resolve(output: Uint8Array<ArrayBuffer>): void;
  reject(error: Error): void;
  onProgress?: (event: ProgressEvent) => void;
}

let worker: Worker | null = null;
let nextId = 1;
const pending = new Map<number, Pending>();

/** Stops the worker and every request it was working on. */
function stop(error: Error): void {
  worker?.terminate();
  worker = null;
  for (const request of pending.values()) {
    request.reject(error);
  }
  pending.clear();
}

function started(): Worker {
  if (worker) {
    return worker;
  }
  const created = new Worker(new URL("./worker.ts", import.meta.url), {
    type: "module",
    name: "RepoDNA analysis",
  });
  created.onmessage = (event: MessageEvent<EngineReply>) => {
    const reply = event.data;
    const request = pending.get(reply.id);
    if (!request) {
      return;
    }
    if (reply.kind === "progress") {
      request.onProgress?.(reply.progress);
      return;
    }
    pending.delete(reply.id);
    if (reply.kind === "done") {
      request.resolve(reply.output);
    } else {
      request.reject(new Error(reply.message));
    }
  };
  created.onerror = (event) => {
    event.preventDefault();
    stop(new Error(event.message || "RepoDNA's analysis program stopped unexpectedly."));
  };
  worker = created;
  return created;
}

type Body = EngineRequest extends infer R
  ? R extends unknown
    ? Omit<R, "id" | "wasm">
    : never
  : never;

function request(
  body: Body,
  onProgress?: (event: ProgressEvent) => void,
): EngineTask<Uint8Array<ArrayBuffer>> {
  const id = nextId++;
  const result = new Promise<Uint8Array<ArrayBuffer>>((resolve, reject) => {
    pending.set(id, { resolve, reject, onProgress });
  });
  const wasm = new URL(
    `${DIRECTORY}repodna.wasm?v=${encodeURIComponent(version)}`,
    document.baseURI,
  ).href;
  started().postMessage({ ...body, id, wasm } as EngineRequest);
  return {
    result,
    cancel: () => {
      if (pending.has(id)) {
        stop(new CancelledError());
      }
    },
  };
}

const decoder = new TextDecoder();

/** A finished analysis: the `.repodna` file's text, and its size in bytes. */
export interface AnalysisFile {
  text: string;
  size: number;
}

function analysisFile(task: EngineTask<Uint8Array<ArrayBuffer>>): EngineTask<AnalysisFile> {
  return {
    ...task,
    result: task.result.then((output) => ({
      text: decoder.decode(output),
      size: output.byteLength,
    })),
  };
}

/** Analyzes the files of the folder `name`. */
export function analyzeFolder(
  name: string,
  files: InputFile[],
  profile: string,
  onProgress?: (event: ProgressEvent) => void,
): EngineTask<AnalysisFile> {
  return analysisFile(request({ kind: "analyze-folder", name, files, profile }, onProgress));
}

/** Analyzes a ZIP or TAR archive. */
export function analyzeArchive(
  archive: File,
  profile: string,
  onProgress?: (event: ProgressEvent) => void,
): EngineTask<AnalysisFile> {
  return analysisFile(request({ kind: "analyze-archive", archive, profile }, onProgress));
}

/** A report of the analysis `artifact` (its JSON text), as `repodna report` makes it. */
export async function renderReport(artifact: string, options: ReportRequest): Promise<string> {
  const output = await request({ kind: "report", artifact, ...options }).result;
  return decoder.decode(output);
}

/** The Project DNA card of the analysis `artifact`, as SVG markup or PNG bytes. */
export function renderCard(
  artifact: string,
  dark: boolean,
  png: true,
): Promise<Uint8Array<ArrayBuffer>>;
export function renderCard(artifact: string, dark: boolean, png?: false): Promise<string>;
export async function renderCard(
  artifact: string,
  dark: boolean,
  png = false,
): Promise<string | Uint8Array<ArrayBuffer>> {
  const output = await request({ kind: "card", artifact, dark, png }).result;
  return png ? output : decoder.decode(output);
}
