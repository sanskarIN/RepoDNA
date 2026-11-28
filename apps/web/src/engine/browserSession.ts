import type { Profile, ProgressEvent } from "./protocol";
import type { AnalysisInput } from "./input";
import type { FolderTraversalOptions } from "./folderTraversal";
import { CancelledError, type WorkerTask } from "./workerQueue";

export interface BrowserAnalysisFile {
  text: string;
  size: number;
}
export type BrowserAnalysis =
  | { phase: "idle" }
  | { phase: "listing"; name: string; files: number }
  | {
      phase: "running";
      name: string;
      files: number | null;
      bytes: number;
      startedAt: number;
      stages: [string, string][];
      stage: string | null;
      done: number;
      total: number;
    }
  | { phase: "finished"; name: string; text: string; size: number }
  | { phase: "failed"; name: string; message: string }
  | { phase: "cancelled"; name: string };
export type FinishedAnalysis = Extract<BrowserAnalysis, { phase: "finished" }>;
export type RunningAnalysis = Extract<BrowserAnalysis, { phase: "running" }>;

export interface BrowserSessionDependencies {
  start(input: AnalysisInput, profile: Profile, onProgress: (event: ProgressEvent) => void): WorkerTask<BrowserAnalysisFile>;
  list(folder: FileSystemDirectoryEntry, onCount: (files: number) => void, options: FolderTraversalOptions): Promise<AnalysisInput | null>;
  now(): number;
}
interface Operation {
  readonly name: string;
  readonly profile: Profile;
  listing: AbortController | null;
  task: WorkerTask<BrowserAnalysisFile> | null;
}
const MAX_PROGRESS_STAGES = 128;
const IDLE: BrowserAnalysis = { phase: "idle" };

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
function counter(value: number): number {
  return Number.isFinite(value) ? Math.max(0, Math.min(Number.MAX_SAFE_INTEGER, Math.trunc(value))) : 0;
}
/** Preserve the prior public helper and bound unsafe progress counter values. */
export function normalizeFileProgress(done: number, total: number): { done: number; total: number } {
  const limit = counter(total);
  return { done: Math.min(counter(done), limit), total: limit };
}

/** Portable analysis filename. Truncate by Unicode code point and UTF-8 bytes,
 * not UTF-16 units: never produce a broken surrogate pair or overlong name. */
export function analysisFileName(name: string): string {
  let stem = name.replace(/\.(zip|tar|tar\.gz|tgz)$/i, "")
    .replace(/[\\/\x00-\x1f\x7f<>:"|?*]/g, "-")
    .replace(/[. ]+$/g, "");
  const encoder = new TextEncoder();
  let retained = "";
  let bytes = 0;
  let characters = 0;
  for (const character of stem) {
    const size = encoder.encode(character).byteLength;
    if (characters === 100 || bytes + size > 220) break;
    retained += character;
    bytes += size;
    characters += 1;
  }
  stem = retained.replace(/[. ]+$/g, "");
  if (/^(con|prn|aux|nul|com[1-9¹²³]|lpt[1-9¹²³])(?:\.|$)/i.test(stem)) stem = "repodna-" + stem;
  return `${stem || "repository"}.repodna`;
}

/** Update one scan's progress without duplicate stage rows or regressing a
 * count within a single stage. A new stage intentionally resets its counts. */
export function applyProgress(state: RunningAnalysis, event: ProgressEvent): RunningAnalysis {
  if (event.event === "message") return state;
  if (event.event === "files") {
    const next = normalizeFileProgress(event.done, event.total);
    // The engine can revise the total as discovery progresses. Use the latest
    // valid total but never display a completed count above that total.
    if (next.total === 0 && state.total > 0) return state;
    const done = Math.min(next.total, Math.max(state.done, next.done));
    if (state.total === next.total && state.done === done) return state;
    return { ...state, total: next.total, done };
  }
  if (!event.stage || !event.status) return state;
  const existing = state.stages.findIndex(([stage]) => stage === event.stage);
  if (event.status === "started") {
    // A delayed duplicate start must not reset the currently visible counts
    // or make a completed stage appear to have started a second time.
    if (state.stage === event.stage || existing !== -1) return state;
    return { ...state, stage: event.stage, done: 0, total: 0 };
  }
  if (existing !== -1) return state;
  const stages: [string, string][] = state.stages.length < MAX_PROGRESS_STAGES
    ? [...state.stages, [event.stage, event.status]] : state.stages;
  if (state.stage === event.stage) return { ...state, stage: null, stages, done: 0, total: 0 };
  // Some stages are skipped without a preceding start. Do not clear a
  // different active stage merely because this status arrived out of order.
  return stages === state.stages ? state : { ...state, stages };
}

/** One browser-owned analysis, independent of React and worker implementation.
 * Operation identity guards every callback, including callbacks made during
 * synchronous startup or cancellation. Subscribers never own task lifetime. */
export class BrowserSession {
  private snapshot: BrowserAnalysis = IDLE;
  private selectedProfile: Profile = "standard";
  private active: Operation | null = null;
  private readonly listeners = new Set<() => void>();
  private consuming = false;

  constructor(private readonly dependencies: BrowserSessionDependencies) {}

  get state(): BrowserAnalysis { return this.snapshot; }
  get profile(): Profile { return this.selectedProfile; }
  setProfile(profile: Profile): void { this.selectedProfile = profile; }
  get busy(): boolean { return this.active !== null || this.consuming; }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => { this.listeners.delete(listener); };
  }
  private publish(next: BrowserAnalysis): void {
    if (this.snapshot === next) return;
    this.snapshot = next;
    // Copy before iterating so that a subscription change inside notification
    // does not unexpectedly invoke a new listener within the same update.
    for (const listener of [...this.listeners]) {
      if (!this.listeners.has(listener)) continue;
      try { listener(); } catch { /* A view subscriber cannot strand the worker. */ }
    }
  }
  private current(operation: Operation): boolean { return this.active === operation; }
  private reserve(name: string, profile: Profile): Operation | null {
    if (this.busy) return null;
    const operation: Operation = { name, profile, listing: null, task: null };
    this.active = operation;
    return operation;
  }
  private finish(operation: Operation, next: BrowserAnalysis): void {
    if (!this.current(operation)) return;
    this.active = null;
    operation.listing = null;
    operation.task = null;
    this.publish(next);
  }
  private failed(operation: Operation, reason: unknown): void {
    this.finish(operation, reason instanceof CancelledError
      ? { phase: "cancelled", name: operation.name }
      : { phase: "failed", name: operation.name, message: message(reason) });
  }
  private progress(operation: Operation, event: ProgressEvent): void {
    if (this.current(operation) && this.snapshot.phase === "running") {
      this.publish(applyProgress(this.snapshot, event));
    }
  }
  private start(operation: Operation, input: AnalysisInput): void {
    if (!this.current(operation)) return;
    const name = input.kind === "folder" ? input.name : input.file.name;
    const next: RunningAnalysis = {
      phase: "running", name,
      files: input.kind === "folder" ? input.files.length : null,
      bytes: input.kind === "folder" ? input.bytes : input.file.size,
      startedAt: this.dependencies.now(), stages: [], stage: null, done: 0, total: 0,
    };
    operation.listing = null;
    this.publish(next);
    if (!this.current(operation)) return;
    let task: WorkerTask<BrowserAnalysisFile>;
    try {
      task = this.dependencies.start(input, operation.profile, (event) => this.progress(operation, event));
    } catch (reason) {
      this.failed(operation, reason);
      return;
    }
    // A synchronous startup progress callback can cancel the scan. Attach a
    // rejection handler before cancelling a task we can no longer own.
    if (!this.current(operation)) {
      void task.result.catch(() => undefined);
      try { task.cancel(); } catch { /* Already abandoned. */ }
      return;
    }
    operation.task = task;
    void task.result.then(
      (file) => {
        if (!this.current(operation) || operation.task !== task) return;
        if (typeof file.text !== "string" || !Number.isSafeInteger(file.size) || file.size < 0) {
          this.failed(operation, new Error("The browser analysis returned an invalid result."));
          return;
        }
        this.finish(operation, { phase: "finished", name: analysisFileName(name), text: file.text, size: file.size });
      },
      (reason: unknown) => {
        if (this.current(operation) && operation.task === task) this.failed(operation, reason);
      },
    );
  }

  analyze(input: AnalysisInput, profile: Profile): void {
    const operation = this.reserve(input.kind === "folder" ? input.name : input.file.name, profile);
    if (operation) this.start(operation, input);
  }

  async analyzeDroppedFolder(folder: FileSystemDirectoryEntry, profile: Profile): Promise<void> {
    const operation = this.reserve(folder.name, profile);
    if (!operation) return;
    const controller = new AbortController();
    operation.listing = controller;
    this.publish({ phase: "listing", name: folder.name, files: 0 });
    if (!this.current(operation)) return;
    try {
      const input = await this.dependencies.list(folder, (files) => {
        if (!this.current(operation) || this.snapshot.phase !== "listing") return;
        const count = Math.max(this.snapshot.files, counter(files));
        if (count !== this.snapshot.files) this.publish({ phase: "listing", name: folder.name, files: count });
      }, { signal: controller.signal });
      if (!this.current(operation)) return;
      if (!input) {
        this.failed(operation, new Error("The folder has no files to analyze."));
        return;
      }
      this.start(operation, input);
    } catch (reason) {
      if (this.current(operation)) this.failed(operation, new Error(`The folder could not be read: ${message(reason)}`));
    }
  }

  cancel(): void {
    const operation = this.active;
    if (!operation) return;
    // Invalidate before aborting; dependencies may immediately call back.
    this.active = null;
    const listing = operation.listing;
    const task = operation.task;
    operation.listing = null;
    operation.task = null;
    listing?.abort();
    try { task?.cancel(); } catch { /* The abandoned task no longer owns state. */ }
    // If cancellation synchronously caused a new operation, preserve it.
    if (!this.active) this.publish({ phase: "cancelled", name: operation.name });
  }

  fail(name: string, reason: string): void {
    if (!this.busy) this.publish({ phase: "failed", name, message: reason });
  }
  dismiss(): void {
    if (!this.busy) this.publish(IDLE);
  }

  /** Claim a particular finished snapshot once, including React Strict Mode's
   * repeated effects. A thrown opener leaves an explicit failed state. */
  consumeFinished(expected: FinishedAnalysis, open: (file: FinishedAnalysis) => void): boolean {
    if (this.snapshot !== expected || this.busy) return false;
    this.consuming = true;
    let error: string | null = null;
    try { open(expected); }
    catch (reason) { error = message(reason); }
    finally { this.consuming = false; }
    if (error !== null) {
      this.publish({ phase: "failed", name: expected.name, message: error });
      return false;
    }
    this.publish(IDLE);
    return true;
  }
}
