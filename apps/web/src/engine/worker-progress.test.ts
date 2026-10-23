import { afterEach, expect, it, vi } from "vitest";
import { analyzeFolder } from "./client";

afterEach(() => vi.unstubAllGlobals());

it("still resolves an analysis after the progress subscriber throws", async () => {
  let worker: { onmessage: ((event: { data: unknown }) => void) | null } | null = null;
  class WorkerStub {
    onmessage: ((event: { data: unknown }) => void) | null = null;
    onerror = null;
    postMessage() {}
    terminate() {}
    constructor() {
      worker = this;
    }
  }
  vi.stubGlobal("Worker", WorkerStub);
  const task = analyzeFolder("project", [], "quick", () => {
    throw new Error("UI callback failed");
  });
  const deliver = (data: unknown) => {
    if (!worker) throw new Error("Worker was not constructed");
    (worker as WorkerStub).onmessage?.({ data });
  };
  expect(() => deliver({ id: 1, kind: "progress", progress: { event: "files", done: 1, total: 2 } })).not.toThrow();
  deliver({ id: 1, kind: "done", output: "{}", size: 2 });
  await expect(task.result).resolves.toEqual({ text: "{}", size: 2 });
});
