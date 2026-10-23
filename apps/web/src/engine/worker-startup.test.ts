import { afterEach, expect, it, vi } from "vitest";
import { analyzeFolder } from "./client";

afterEach(() => vi.unstubAllGlobals());

it("rejects a task when a worker refuses to clone its input", async () => {
  const terminate = vi.fn();
  const worker = class {
    onmessage = null;
    onerror = null;
    postMessage() {
      throw new DOMException("Cannot clone file", "DataCloneError");
    }
    terminate = terminate;
  };
  vi.stubGlobal("Worker", worker);
  const task = analyzeFolder("sample", [], "quick");
  await expect(task.result).rejects.toThrow("Cannot clone file");
});
