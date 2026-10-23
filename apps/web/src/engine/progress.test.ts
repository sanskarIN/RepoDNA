import { describe, expect, it } from "vitest";
import { normalizeFileProgress } from "./session";

describe("browser file progress normalization", () => {
  it("preserves valid worker progress values", () => {
    expect(normalizeFileProgress(2, 5)).toEqual({ done: 2, total: 5 });
  });

  it("clamps overflow, negative and nonfinite values", () => {
    expect(normalizeFileProgress(100, 4)).toEqual({ done: 4, total: 4 });
    expect(normalizeFileProgress(-1, 8)).toEqual({ done: 0, total: 8 });
    expect(normalizeFileProgress(Infinity, 10)).toEqual({ done: 0, total: 10 });
    expect(normalizeFileProgress(7, NaN)).toEqual({ done: 0, total: 0 });
    expect(normalizeFileProgress(1.9, 5.8)).toEqual({ done: 1, total: 5 });
  });
});
