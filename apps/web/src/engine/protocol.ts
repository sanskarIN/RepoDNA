// Messages between the page and the worker that runs RepoDNA's WebAssembly program.

import type { Profile, ProgressEvent } from "@repodna/wasm";

export type { Profile, ProgressEvent } from "@repodna/wasm";

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
  | { kind: "analyze-folder"; name: string; files: InputFile[]; profile: Profile }
  | { kind: "analyze-archive"; archive: File; profile: Profile }
  | ({ kind: "report"; artifact: string } & ReportRequest)
  | { kind: "card"; artifact: string; dark: boolean; png: boolean }
);

/** What the worker answers: progress, then the output or why there is none. */
export type EngineReply = { id: number } & (
  | { kind: "progress"; progress: ProgressEvent }
  /** Text, or the bytes of an image, and their size in bytes. */
  | { kind: "done"; output: string | Uint8Array<ArrayBuffer>; size: number }
  | { kind: "failed"; message: string }
);
