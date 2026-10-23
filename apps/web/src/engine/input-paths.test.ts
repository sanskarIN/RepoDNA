import { describe, expect, it } from "vitest";
import { safeRelativePath } from "./input";

describe("folder import path validation", () => {
  it("accepts ordinary nested source files", () => {
    expect(safeRelativePath("src/lib.rs")).toBe(true);
    expect(safeRelativePath("project/.gitignore")).toBe(true);
  });

  it("rejects paths that are absolute or escape the selected folder", () => {
    for (const name of [
      "", "/etc/passwd", "../secret", "src/../secret", "src//file",
      "./file", "C:\\\\private", "src\\\\other", "unsafe\0path",
    ]) {
      expect(safeRelativePath(name)).toBe(false);
    }
  });
});
