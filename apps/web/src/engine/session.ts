// The analysis running in this browser, if any: one at a time, started from the start page
// or by dropping a folder or an archive anywhere on the page, and shown on the start page.

import { useSyncExternalStore } from "react";
import {
  analyzeArchive,
  analyzeFolder,
  CancelledError,
  type AnalysisFile,
  type EngineTask,
  type Profile,
} from "./client";
import { folderFromEntry, type AnalysisInput } from "./input";
import type { ProgressEvent } from "./protocol";

export type BrowserAnalysis =
  | { phase: "idle" }
  /** Listing the files of a dropped folder. */
  | { phase: "listing"; name: string; files: number }
  | {
      phase: "running";
      name: string;
      /** Files and bytes given to the analysis; unknown for an archive. */
      files: number | null;
      bytes: number;
      startedAt: number;
      /** Finished stages with how they ended, in order. */
      stages: [string, string][];
      stage: string | null;
      done: number;
      total: number;
    }
  | { phase: "finished"; name: string; text: string; size: number }
  | { phase: "failed"; name: string; message: string }
  | { phase: "cancelled"; name: string };

let state: BrowserAnalysis = { phase: "idle" };
let chosenProfile: Profile = "standard";
let task: EngineTask<AnalysisFile> | null = null;
// Raised by `cancel`, so that a folder being listed is not analyzed afterwards.
let generation = 0;
const listeners = new Set<() => void>();

function set(next: BrowserAnalysis): void {
  state = next;
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** The analysis running in this browser, kept up to date. */
export function useBrowserAnalysis(): BrowserAnalysis {
  return useSyncExternalStore(subscribe, () => state);
}

/** The analysis profile chosen on the start page, used for folders dropped anywhere. */
export function profile(): Profile {
  return chosenProfile;
}

export function setProfile(next: Profile): void {
  chosenProfile = next;
}

/** Whether an analysis is being prepared or is running. */
export function busy(): boolean {
  return state.phase === "listing" || state.phase === "running";
}

function progress(event: ProgressEvent): void {
  if (state.phase !== "running") {
    return;
  }
  if (event.event === "stage") {
    set(
      event.status === "started"
        ? { ...state, stage: event.stage, done: 0, total: 0 }
        : { ...state, stage: null, stages: [...state.stages, [event.stage, event.status]] },
    );
  } else if (event.event === "files") {
    set({ ...state, done: event.done, total: event.total });
  }
}

/** The name of the file an analysis of `name` is kept as. */
function fileName(name: string): string {
  return `${name.replace(/\.(zip|tar|tar\.gz|tgz)$/i, "")}.repodna`;
}

/** Analyzes a folder's files or an archive with the analysis profile `profile`. */
export function analyze(input: AnalysisInput, profile: Profile): void {
  if (busy()) {
    return;
  }
  const name = input.kind === "folder" ? input.name : input.file.name;
  set({
    phase: "running",
    name,
    files: input.kind === "folder" ? input.files.length : null,
    bytes: input.kind === "folder" ? input.bytes : input.file.size,
    startedAt: Date.now(),
    stages: [],
    stage: null,
    done: 0,
    total: 0,
  });
  const running =
    input.kind === "folder"
      ? analyzeFolder(input.name, input.files, profile, progress)
      : analyzeArchive(input.file, profile, progress);
  task = running;
  running.result.then(
    ({ text, size }) => {
      if (task === running) {
        task = null;
        set({ phase: "finished", name: fileName(name), text, size });
      }
    },
    (error: unknown) => {
      if (task !== running) {
        return;
      }
      task = null;
      set(
        error instanceof CancelledError
          ? { phase: "cancelled", name }
          : {
              phase: "failed",
              name,
              message: error instanceof Error ? error.message : String(error),
            },
      );
    },
  );
}

/** Lists a dropped folder, then analyzes it. */
export async function analyzeDroppedFolder(
  folder: FileSystemDirectoryEntry,
  profile: Profile,
): Promise<void> {
  if (busy()) {
    return;
  }
  const started = ++generation;
  set({ phase: "listing", name: folder.name, files: 0 });
  try {
    const input = await folderFromEntry(folder, (files) => {
      if (started === generation) {
        set({ phase: "listing", name: folder.name, files });
      }
    });
    if (started !== generation) {
      return;
    }
    if (!input) {
      set({ phase: "failed", name: folder.name, message: "The folder has no files to analyze." });
      return;
    }
    set({ phase: "idle" });
    analyze(input, profile);
  } catch (error) {
    if (started === generation) {
      set({
        phase: "failed",
        name: folder.name,
        message: `The folder could not be read: ${error instanceof Error ? error.message : String(error)}`,
      });
    }
  }
}

/** Stops the analysis, or the listing of a folder before it. */
export function cancel(): void {
  if (state.phase === "listing") {
    generation += 1;
    set({ phase: "cancelled", name: state.name });
  } else if (state.phase === "running") {
    task?.cancel();
  }
}

/** Records that a finished analysis could not be opened. */
export function fail(name: string, message: string): void {
  set({ phase: "failed", name, message });
}

/** Forgets a finished, failed, or cancelled analysis. */
export function dismiss(): void {
  if (!busy()) {
    set({ phase: "idle" });
  }
}
