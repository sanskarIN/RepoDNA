// Messages between the page and the worker that runs RepoDNA's WebAssembly program.

/** A file to analyze, at a path relative to the folder that was chosen. */
export interface InputFile {
  path: string;
  file: Blob;
}

/** Report options, named as `repodna report` names them. */
export interface ReportRequest {
  format: "html" | "markdown" | "json";
  theme: "professional" | "minimal" | "technical" | "dark";
  privacy: "local" | "share" | "public";
}

/** What the worker is asked to do, with the address of the program to run. */
export type EngineRequest = { id: number; wasm: string } & (
  | { kind: "analyze-folder"; name: string; files: InputFile[]; profile: string }
  | { kind: "analyze-archive"; archive: File; profile: string }
  | ({ kind: "report"; artifact: string } & ReportRequest)
  | { kind: "card"; artifact: string; dark: boolean; png: boolean }
);

/** Progress of an analysis, as the program writes it. */
export type ProgressEvent =
  | { event: "stage"; stage: string; status: string; milliseconds?: number }
  | { event: "files"; done: number; total: number }
  | { event: "message"; text: string };

/** What the worker answers: progress, then the output or why there is none. */
export type EngineReply = { id: number } & (
  | { kind: "progress"; progress: ProgressEvent }
  | { kind: "done"; output: Uint8Array<ArrayBuffer> }
  | { kind: "failed"; message: string }
);
