import { describe, expect, it } from "vitest";
import { BrowserSession, analysisFileName, applyProgress, normalizeFileProgress, type BrowserAnalysisFile, type BrowserSessionDependencies, type FinishedAnalysis, type RunningAnalysis } from "./browserSession";
import type { AnalysisInput } from "./input";
import type { ProgressEvent, Profile } from "./protocol";
import { CancelledError } from "./workerQueue";
import { deferred, flush } from "../lib/controllerTestSupport";

function folder(name = "repo"): AnalysisInput {
  return { kind: "folder", name, files: [{ path: "README.md", file: new Blob(["hello"]) }], bytes: 5 };
}
function dropped(name = "repo"): FileSystemDirectoryEntry {
  return { name, isDirectory: true, isFile: false } as FileSystemDirectoryEntry;
}
function running(): RunningAnalysis {
  return { phase: "running", name: "repo", files: 10, bytes: 100, startedAt: 42, stages: [], stage: null, done: 0, total: 0 };
}
function harness() {
  const starts: { input: AnalysisInput; profile: Profile; emit: (event: ProgressEvent) => void; result: ReturnType<typeof deferred<BrowserAnalysisFile>>; cancelled: number }[] = [];
  const listings: { folder: FileSystemDirectoryEntry; count: (files: number) => void; signal: AbortSignal | undefined; result: ReturnType<typeof deferred<AnalysisInput | null>> }[] = [];
  const dependencies: BrowserSessionDependencies = {
    now: () => 42,
    start(input, profile, emit) {
      const entry = { input, profile, emit, result: deferred<BrowserAnalysisFile>(), cancelled: 0 };
      starts.push(entry);
      return { result: entry.result.promise, cancel() { entry.cancelled += 1; entry.result.reject(new CancelledError()); } };
    },
    list(folder, count, options) {
      const entry = { folder, count, signal: options.signal, result: deferred<AnalysisInput | null>() };
      listings.push(entry);
      return entry.result.promise;
    },
  };
  return { starts, listings, dependencies, session: new BrowserSession(dependencies) };
}

function finish(session: BrowserSession): FinishedAnalysis {
  if (session.state.phase !== "finished") throw new Error("Expected a finished analysis");
  return session.state;
}

describe("portable analysis filenames and progress", () => {
  it("strips supported archive suffixes case-insensitively", () => {
    for (const name of ["repo.zip", "repo.TAR", "repo.tar.gz", "repo.TGZ"]) expect(analysisFileName(name)).toBe("repo.repodna");
    expect(analysisFileName("repo.txt")).toBe("repo.txt.repodna");
  });
  it("neutralizes path separators, controls and Windows filename punctuation", () => {
    expect(analysisFileName('a/b\\c:\0<d>?*.zip')).toBe("a-b-c---d---.repodna");
    expect(analysisFileName("... ")).toBe("repository.repodna");
  });
  it("handles Windows reserved names, including superscript device digits", () => {
    for (const name of ["CON", "prn.txt", "AUX.zip", "NUL", "COM1", "LPT9", "COM²"]) expect(analysisFileName(name).startsWith("repodna-")).toBe(true);
    expect(analysisFileName("console")).toBe("console.repodna");
  });
  it("limits multibyte filenames without breaking Unicode code points", () => {
    const result = analysisFileName("😀".repeat(200) + ".zip");
    expect(new TextEncoder().encode(result).length <= 255).toBe(true);
    expect(result.includes("\uFFFD")).toBe(false);
    expect(result).toBe("😀".repeat(55) + ".repodna");
  });
  it("keeps counters finite, safe and bounded by total", () => {
    expect(normalizeFileProgress(100, 4)).toEqual({ done: 4, total: 4 });
    expect(normalizeFileProgress(-5, Infinity)).toEqual({ done: 0, total: 0 });
    expect(normalizeFileProgress(1.9, 5.8)).toEqual({ done: 1, total: 5 });
    expect(normalizeFileProgress(Number.MAX_VALUE, Number.MAX_VALUE)).toEqual({ done: Number.MAX_SAFE_INTEGER, total: Number.MAX_SAFE_INTEGER });
  });
  it("ignores duplicate stage starts and regressing progress", () => {
    const started = applyProgress(running(), { event: "stage", stage: "discovery", status: "started" });
    const progress = applyProgress(started, { event: "files", done: 5, total: 10 });
    expect(applyProgress(progress, { event: "files", done: 3, total: 10 })).toBe(progress);
    expect(applyProgress(progress, { event: "stage", stage: "discovery", status: "started" })).toBe(progress);
  });
  it("resets file counters when the next stage begins", () => {
    const state = { ...running(), stage: "discovery", done: 10, total: 10 };
    const completed = applyProgress(state, { event: "stage", stage: "discovery", status: "completed" });
    const next = applyProgress(completed, { event: "stage", stage: "quality", status: "started" });
    expect(next.stage).toBe("quality");
    expect(next.done).toBe(0);
    expect(next.total).toBe(0);
    expect(next.stages).toEqual([["discovery", "completed"]]);
  });
  it("retains skipped stages without clearing another active stage", () => {
    const state = { ...running(), stage: "quality", done: 3, total: 10 };
    const next = applyProgress(state, { event: "stage", stage: "git", status: "skipped" });
    expect(next.stage).toBe("quality");
    expect(next.done).toBe(3);
    expect(next.stages).toEqual([["git", "skipped"]]);
  });
  it("deduplicates completion rows and delayed starts", () => {
    const first = applyProgress(running(), { event: "stage", stage: "git", status: "skipped" });
    expect(applyProgress(first, { event: "stage", stage: "git", status: "skipped" })).toBe(first);
    expect(applyProgress(first, { event: "stage", stage: "git", status: "started" })).toBe(first);
  });
  it("bounds retained stage diagnostics", () => {
    let state = running();
    for (let index = 0; index < 200; index += 1) state = applyProgress(state, { event: "stage", stage: `stage-${index}`, status: "completed" });
    expect(state.stages.length).toBe(128);
  });
});

describe("browser analysis session lifecycle", () => {
  it("starts one task and exposes its immutable state to subscribers", async () => {
    const { session, starts } = harness();
    const phases: string[] = [];
    const unsubscribe = session.subscribe(() => phases.push(session.state.phase));
    session.analyze(folder(), "deep");
    session.analyze(folder("other"), "quick");
    expect(starts.length).toBe(1);
    expect(starts[0]?.profile).toBe("deep");
    expect(session.state.phase).toBe("running");
    expect(session.busy).toBe(true);
    starts[0]!.result.resolve({ text: "{}", size: 2 });
    await flush();
    expect(session.state).toEqual({ phase: "finished", name: "repo.repodna", text: "{}", size: 2 });
    expect(session.busy).toBe(false);
    unsubscribe();
    session.dismiss();
    expect(phases).toEqual(["running", "finished"]);
  });
  it("keeps the chosen profile separate from an already-running scan", () => {
    const { session, starts } = harness();
    expect(session.profile).toBe("standard");
    session.setProfile("quick");
    session.analyze(folder(), session.profile);
    session.setProfile("deep");
    expect(starts[0]?.profile).toBe("quick");
    expect(session.profile).toBe("deep");
  });
  it("cancels the active worker and ignores its later progress or rejection", async () => {
    const { session, starts } = harness();
    session.analyze(folder("first"), "standard");
    session.cancel();
    expect(starts[0]?.cancelled).toBe(1);
    expect(session.state).toEqual({ phase: "cancelled", name: "first" });
    session.analyze(folder("second"), "standard");
    starts[0]?.emit({ event: "stage", stage: "stale", status: "started" });
    await flush();
    expect(session.state.phase === "running" && session.state.name).toBe("second");
    expect(session.state.phase === "running" && session.state.stage).toBeNull();
  });
  it("does not start a worker if a subscriber cancels the initial notification", () => {
    const { session, starts } = harness();
    session.subscribe(() => { if (session.state.phase === "running") session.cancel(); });
    session.analyze(folder(), "standard");
    expect(starts.length).toBe(0);
    expect(session.state.phase).toBe("cancelled");
  });
  it("cancels a task returned after a synchronous progress callback cancelled startup", async () => {
    const result = deferred<BrowserAnalysisFile>();
    let cancelled = 0;
    const { dependencies } = harness();
    dependencies.start = (_input, _profile, emit) => {
      emit({ event: "stage", stage: "discovery", status: "started" });
      return { result: result.promise, cancel() { cancelled += 1; result.reject(new CancelledError()); } };
    };
    const session = new BrowserSession(dependencies);
    session.subscribe(() => { if (session.state.phase === "running" && session.state.stage) session.cancel(); });
    session.analyze(folder(), "standard");
    await flush();
    expect(cancelled).toBe(1);
    expect(session.state.phase).toBe("cancelled");
  });
  it("recovers from a synchronous worker-start exception", () => {
    const { dependencies } = harness();
    dependencies.start = () => { throw new Error("worker unavailable"); };
    const session = new BrowserSession(dependencies);
    session.analyze(folder(), "standard");
    expect(session.state).toEqual({ phase: "failed", name: "repo", message: "worker unavailable" });
    expect(session.busy).toBe(false);
  });
  it("reports worker failures and separate cancellation outcomes", async () => {
    const { session, starts } = harness();
    session.analyze(folder(), "standard");
    starts[0]?.result.reject(new Error("bad archive"));
    await flush();
    expect(session.state).toEqual({ phase: "failed", name: "repo", message: "bad archive" });
    session.analyze(folder(), "standard");
    starts[1]?.result.reject(new CancelledError());
    await flush();
    expect(session.state.phase).toBe("cancelled");
  });
  it("rejects invalid worker result metadata", async () => {
    const { session, starts } = harness();
    session.analyze(folder(), "standard");
    starts[0]?.result.resolve({ text: "{}", size: -1 });
    await flush();
    expect(session.state.phase).toBe("failed");
  });
  it("isolates subscriber exceptions from worker ownership", async () => {
    const { session, starts } = harness();
    session.subscribe(() => { throw new Error("view error"); });
    let received = 0;
    session.subscribe(() => { received += 1; });
    session.analyze(folder(), "standard");
    starts[0]?.result.resolve({ text: "{}", size: 2 });
    await flush();
    expect(received).toBe(2);
    expect(session.state.phase).toBe("finished");
  });
  it("does not dismiss or overwrite a running analysis", () => {
    const { session } = harness();
    session.analyze(folder(), "standard");
    session.dismiss();
    session.fail("other", "error");
    expect(session.state.phase).toBe("running");
  });
});

describe("folder preparation and result ownership", () => {
  it("transitions directly from listing to running without an idle gap", async () => {
    const { session, listings, starts } = harness();
    const phases: string[] = [];
    session.subscribe(() => phases.push(session.state.phase));
    const pending = session.analyzeDroppedFolder(dropped(), "quick");
    listings[0]?.count(3);
    listings[0]?.count(2);
    expect(session.state).toEqual({ phase: "listing", name: "repo", files: 3 });
    listings[0]?.result.resolve(folder());
    await pending;
    expect(starts[0]?.profile).toBe("quick");
    expect(phases).toEqual(["listing", "listing", "running"]);
  });
  it("aborts listing and ignores stale callbacks when a new scan begins", async () => {
    const { session, listings, starts } = harness();
    const pending = session.analyzeDroppedFolder(dropped("old"), "standard");
    session.cancel();
    expect(listings[0]?.signal?.aborted).toBe(true);
    session.analyze(folder("new"), "quick");
    listings[0]?.count(999);
    listings[0]?.result.resolve(folder("old"));
    await pending;
    expect(starts.length).toBe(1);
    expect(session.state.phase === "running" && session.state.name).toBe("new");
  });
  it("does not call the folder reader after initial subscriber cancellation", async () => {
    const { session, listings } = harness();
    session.subscribe(() => { if (session.state.phase === "listing") session.cancel(); });
    await session.analyzeDroppedFolder(dropped(), "standard");
    expect(listings.length).toBe(0);
    expect(session.state.phase).toBe("cancelled");
  });
  it("reports empty folders without starting an analysis", async () => {
    const { session, listings, starts } = harness();
    const pending = session.analyzeDroppedFolder(dropped(), "standard");
    listings[0]?.result.resolve(null);
    await pending;
    expect(starts.length).toBe(0);
    expect(session.state).toEqual({ phase: "failed", name: "repo", message: "The folder has no files to analyze." });
  });
  it("reports folder enumeration failures", async () => {
    const { session, listings } = harness();
    const pending = session.analyzeDroppedFolder(dropped(), "standard");
    listings[0]?.result.reject(new Error("permission denied"));
    await pending;
    expect(session.state).toEqual({ phase: "failed", name: "repo", message: "The folder could not be read: permission denied" });
  });
  it("claims a finished artifact exactly once even if a view effect runs twice", async () => {
    const { session, starts } = harness();
    session.analyze(folder(), "standard");
    starts[0]?.result.resolve({ text: "{}", size: 2 });
    await flush();
    const completed = finish(session);
    let opened = 0;
    expect(session.consumeFinished(completed, () => { opened += 1; })).toBe(true);
    expect(session.consumeFinished(completed, () => { opened += 1; })).toBe(false);
    expect(opened).toBe(1);
    expect(session.state.phase).toBe("idle");
  });
  it("does not allow a reentrant open or analysis during consumption", async () => {
    const { session, starts } = harness();
    session.analyze(folder(), "standard");
    starts[0]?.result.resolve({ text: "{}", size: 2 });
    await flush();
    const completed = finish(session);
    expect(session.consumeFinished(completed, () => {
      expect(session.busy).toBe(true);
      expect(session.consumeFinished(completed, () => { throw new Error("duplicate"); })).toBe(false);
      session.analyze(folder("new"), "standard");
    })).toBe(true);
    expect(starts.length).toBe(1);
  });
  it("leaves a recoverable failure if parsing or opening the finished artifact throws", async () => {
    const { session, starts } = harness();
    session.analyze(folder(), "standard");
    starts[0]?.result.resolve({ text: "bad", size: 3 });
    await flush();
    expect(session.consumeFinished(finish(session), () => { throw new Error("invalid artifact"); })).toBe(false);
    expect(session.state).toEqual({ phase: "failed", name: "repo.repodna", message: "invalid artifact" });
    session.dismiss();
    expect(session.state.phase).toBe("idle");
  });
  it("does not let an earlier finished artifact overwrite a newer analysis", async () => {
    const { session, starts } = harness();
    session.analyze(folder("old"), "standard");
    starts[0]?.result.resolve({ text: "{}", size: 2 });
    await flush();
    const old = finish(session);
    session.analyze(folder("new"), "standard");
    expect(session.consumeFinished(old, () => { throw new Error("stale opener"); })).toBe(false);
    expect(session.state.phase === "running" && session.state.name).toBe("new");
  });
});
