// The browser analysis session is shared between the home view and drop targets.
import { useSyncExternalStore } from "react";
import { analyzeArchive, analyzeFolder, type Profile } from "./client";
import { folderFromEntry, type AnalysisInput } from "./input";
import { BrowserSession, type FinishedAnalysis } from "./browserSession";

export { analysisFileName, normalizeFileProgress, type BrowserAnalysis } from "./browserSession";

const session = new BrowserSession({
  start(input, profile, progress) {
    return input.kind === "folder"
      ? analyzeFolder(input.name, input.files, profile, progress)
      : analyzeArchive(input.file, profile, progress);
  },
  list: folderFromEntry,
  now: Date.now,
});
const subscribe = (listener: () => void) => session.subscribe(listener);
const snapshot = () => session.state;

export function useBrowserAnalysis() {
  return useSyncExternalStore(subscribe, snapshot);
}
export function profile(): Profile { return session.profile; }
export function setProfile(next: Profile): void { session.setProfile(next); }
export function busy(): boolean { return session.busy; }
export function analyze(input: AnalysisInput, profile: Profile): void { session.analyze(input, profile); }
export function analyzeDroppedFolder(folder: FileSystemDirectoryEntry, profile: Profile): Promise<void> {
  return session.analyzeDroppedFolder(folder, profile);
}
export function cancel(): void { session.cancel(); }
export function fail(name: string, message: string): void { session.fail(name, message); }
export function dismiss(): void { session.dismiss(); }
export function consumeFinished(expected: FinishedAnalysis, open: (file: FinishedAnalysis) => void): boolean {
  return session.consumeFinished(expected, open);
}
