// RepoDNA's WebAssembly program, run by a worker on this page: analyses of a folder or an
// archive the user chooses, and reports and cards of an analysis, without a server. The
// files are read in the browser and never uploaded.

import type { Profile, InputFile, ProgressEvent, ReportRequest } from "./protocol";
import { WorkerQueue, type WorkerBody, type WorkerOutput, type WorkerTask } from "./workerQueue";

export { CancelledError } from "./workerQueue";
export type { InputFile, Profile, ProgressEvent, ReportRequest } from "./protocol";

import { EngineCatalog, type EngineInfo } from "./engineCatalog";
export { isInfo, type EngineInfo } from "./engineCatalog";

const catalog = new EngineCatalog();

/** The program this interface comes with, or null when unavailable. */
export function engineInfo(): Promise<EngineInfo | null> {
  return catalog.load(document.baseURI);
}

/** A request in progress, and the way to stop it. */
export type EngineTask<T> = WorkerTask<T>;
type Output = WorkerOutput;

const queue = new WorkerQueue({
  create: () => new Worker(new URL("./worker.ts", import.meta.url), {
    type: "module",
    name: "RepoDNA analysis",
  }),
});

function request(body: WorkerBody, onProgress?: (event: ProgressEvent) => void): EngineTask<Output> {
  const wasm = catalog.wasmUrl(document.baseURI);
  return queue.request(body, wasm, onProgress);
}

/** A finished analysis: the `.repodna` file's text, and its size in bytes. */
export interface AnalysisFile {
  text: string;
  size: number;
}

function text(output: Output): string {
  if (typeof output.output !== "string") {
    throw new Error("RepoDNA's analysis program answered with an image instead of text.");
  }
  return output.output;
}

function analysisFile(task: EngineTask<Output>): EngineTask<AnalysisFile> {
  return {
    ...task,
    result: task.result.then((output) => ({ text: text(output), size: output.size })),
  };
}

/** Analyzes the files of the folder `name`. */
export function analyzeFolder(
  name: string,
  files: InputFile[],
  profile: Profile,
  onProgress?: (event: ProgressEvent) => void,
): EngineTask<AnalysisFile> {
  return analysisFile(request({ kind: "analyze-folder", name, files, profile }, onProgress));
}

/** Analyzes a ZIP or TAR archive. */
export function analyzeArchive(
  archive: File,
  profile: Profile,
  onProgress?: (event: ProgressEvent) => void,
): EngineTask<AnalysisFile> {
  return analysisFile(request({ kind: "analyze-archive", archive, profile }, onProgress));
}

/** A report of the analysis `artifact` (its JSON text), as `repodna report` makes it. */
export function renderReportTask(artifact: string, options: ReportRequest): EngineTask<string> {
  const task = request({ kind: "report", artifact, ...options });
  return { result: task.result.then(text), cancel: task.cancel };
}
export function renderReport(artifact: string, options: ReportRequest): Promise<string> {
  return renderReportTask(artifact, options).result;
}

/** The Project DNA card of the analysis `artifact`, as SVG markup or PNG bytes. */
export function renderCardTask(artifact: string, dark: boolean, png: true): EngineTask<Uint8Array<ArrayBuffer>>;
export function renderCardTask(artifact: string, dark: boolean, png?: false): EngineTask<string>;
export function renderCardTask(artifact: string, dark: boolean, png = false): EngineTask<string | Uint8Array<ArrayBuffer>> {
  const task = request({ kind: "card", artifact, dark, png });
  return { result: task.result.then(({ output }) => output), cancel: task.cancel };
}
export function renderCard(artifact: string, dark: boolean, png: true): Promise<Uint8Array<ArrayBuffer>>;
export function renderCard(artifact: string, dark: boolean, png?: false): Promise<string>;
export function renderCard(artifact: string, dark: boolean, png = false): Promise<string | Uint8Array<ArrayBuffer>> {
  return png ? renderCardTask(artifact, dark, true).result : renderCardTask(artifact, dark, false).result;
}
