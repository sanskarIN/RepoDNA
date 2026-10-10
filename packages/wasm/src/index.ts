// RepoDNA's analysis, reports, and Project DNA cards as WebAssembly. The same program as
// the web version runs, for browsers and Node.js: give it the files of a folder or an
// archive, and it returns the analysis as RepoDNA's JSON artifact. It runs where the code
// is; nothing is uploaded. Without Git, an analysis has no history.

import { bytesFile, fileFileSystem, folderFileSystem, WORK } from "./files";
import type { FolderFile, LazyFile } from "./files";
import { run, type ProgressEvent } from "./program";

export { keep, SKIPPED_DIRECTORIES } from "./paths";
export type { FolderFile, LazyFile } from "./files";
export type { ProgressEvent } from "./program";

/** Analysis profiles, as `repodna analyze --profile` names them. */
export type Profile =
  "quick" | "standard" | "deep" | "history-only" | "architecture-only" | "dependencies-only";

export interface AnalyzeOptions {
  /** Defaults to the repository's own configuration, or `standard`. */
  profile?: Profile;
  /** Replace contributor names with pseudonyms. */
  anonymizeContributors?: boolean;
  /** Hears each stage as it starts and ends, and the files done. */
  onProgress?: (event: ProgressEvent) => void;
}

export interface ReportOptions {
  /** Defaults to `html`. */
  format?: "html" | "markdown" | "json";
  /** The look of an HTML report; defaults to `professional`. */
  theme?: "professional" | "minimal" | "technical" | "dark";
  /** What to leave out before rendering; defaults to `local`, the complete analysis. */
  privacy?: "local" | "share" | "public";
}

export interface CardOptions {
  /** Use the dark palette. */
  dark?: boolean;
}

const ANALYSIS = "analysis.repodna";
const decoder = new TextDecoder();
const encoder = new TextEncoder();

function analyzeArguments(path: string, options: AnalyzeOptions): string[] {
  const args = ["analyze", path];
  if (options.profile) {
    args.push("--profile", options.profile);
  }
  if (options.anonymizeContributors) {
    args.push("--anonymize-contributors");
  }
  return args;
}

/** The analysis `artifact`, as JSON text or as an object, in the file system. */
function analysisFile(artifact: string | object) {
  const text = typeof artifact === "string" ? artifact : JSON.stringify(artifact);
  return fileFileSystem(ANALYSIS, bytesFile(encoder.encode(text)));
}

/** RepoDNA's WebAssembly program, compiled and ready to run. */
export class RepoDNA {
  private constructor(readonly module: WebAssembly.Module) {}

  /** Compiles the program from its bytes, or from a response that delivers them. */
  static async load(program: BufferSource | Response | PromiseLike<Response>): Promise<RepoDNA> {
    const source = await program;
    if (source instanceof Response) {
      if (!source.ok) {
        throw new Error(`RepoDNA's program could not be loaded (HTTP ${source.status}).`);
      }
      return new RepoDNA(await WebAssembly.compile(await source.arrayBuffer()));
    }
    return new RepoDNA(await WebAssembly.compile(source));
  }

  /** Wraps a program compiled elsewhere, such as one sent to a worker. */
  static from(module: WebAssembly.Module): RepoDNA {
    return new RepoDNA(module);
  }

  /** The program's version, such as `1.3.1`. */
  async version(): Promise<string> {
    const output = decoder.decode(await run(this.module, ["version"], new Map()));
    return output.trim().replace(/^repodna-wasm\s+/, "");
  }

  /**
   * Analyzes the files of the folder `name`, given with their paths from the folder.
   * Resolves to the analysis as JSON text, the contents of a `.repodna` file.
   */
  async analyzeFolder(
    name: string,
    files: Iterable<FolderFile>,
    options: AnalyzeOptions = {},
  ): Promise<string> {
    const folder = name.replace(/[/\\]/g, "-") || "repository";
    const output = await run(
      this.module,
      analyzeArguments(`${WORK}/${folder}`, options),
      folderFileSystem(folder, files),
      options.onProgress,
    );
    return decoder.decode(output);
  }

  /**
   * Analyzes a ZIP or TAR archive (`.zip`, `.tar`, `.tar.gz`, or `.tgz`, as `name` says).
   * Resolves to the analysis as JSON text.
   */
  async analyzeArchive(
    name: string,
    archive: LazyFile,
    options: AnalyzeOptions = {},
  ): Promise<string> {
    const file = name.replace(/[/\\]/g, "-");
    const output = await run(
      this.module,
      analyzeArguments(`${WORK}/${file}`, options),
      fileFileSystem(file, archive),
      options.onProgress,
    );
    return decoder.decode(output);
  }

  /** A report of an analysis, as `repodna report` makes it. */
  async report(artifact: string | object, options: ReportOptions = {}): Promise<string> {
    const args = ["report", `${WORK}/${ANALYSIS}`, "--format", options.format ?? "html"];
    args.push("--theme", options.theme ?? "professional");
    args.push("--privacy", options.privacy ?? "local");
    return decoder.decode(await run(this.module, args, analysisFile(artifact)));
  }

  /** The Project DNA card of an analysis as SVG markup, as `repodna card` makes it. */
  async card(artifact: string | object, options: CardOptions = {}): Promise<string> {
    const args = ["card", `${WORK}/${ANALYSIS}`, ...(options.dark ? ["--dark"] : [])];
    return decoder.decode(await run(this.module, args, analysisFile(artifact)));
  }

  /** The Project DNA card of an analysis as a PNG image. */
  async cardPng(
    artifact: string | object,
    options: CardOptions = {},
  ): Promise<Uint8Array<ArrayBuffer>> {
    const args = ["card", `${WORK}/${ANALYSIS}`, "--png", ...(options.dark ? ["--dark"] : [])];
    return run(this.module, args, analysisFile(artifact));
  }
}
