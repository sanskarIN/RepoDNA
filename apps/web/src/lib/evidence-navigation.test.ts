import { describe, expect, it } from "vitest";
import type { FileRecord } from "@repodna/schema";
import { evidenceFilePath } from "./evidence-navigation";

const files = [
  { path: "src/a.ts" },
  { path: "config/package.json" },
] as FileRecord[];

describe("file-backed evidence navigation", () => {
  it("links only evidence with an exact path in the artifact", () => {
    expect(evidenceFilePath({ kind: "file", path: "src/a.ts", line: 42 }, files)).toBe("src/a.ts");
    expect(evidenceFilePath({ kind: "symbol", path: "src/a.ts", name: "f" }, files)).toBe(
      "src/a.ts",
    );
    expect(
      evidenceFilePath(
        { kind: "dependency-edge", from: "a", to: "b", path: "src/a.ts" },
        files,
      ),
    ).toBe("src/a.ts");
    expect(
      evidenceFilePath(
        { kind: "package", ecosystem: "npm", name: "foo", manifest: "config/package.json" },
        files,
      ),
    ).toBe("config/package.json");

    expect(evidenceFilePath({ kind: "file", path: "src/missing.ts" }, files)).toBeUndefined();
    expect(evidenceFilePath({ kind: "directory", path: "src/a.ts" }, files)).toBeUndefined();
    expect(evidenceFilePath({ kind: "observation", text: "No README" }, files)).toBeUndefined();
    expect(evidenceFilePath({ kind: "file", path: "src/a.ts" }, undefined)).toBeUndefined();
    expect(evidenceFilePath({ kind: "file", path: "" }, files)).toBeUndefined();
  });

  it("builds its path index once per file-array identity", () => {
    let reads = 0;
    const records = [
      {
        get path() {
          reads += 1;
          return "src/a.ts";
        },
      },
    ] as FileRecord[];
    const target = { kind: "file" as const, path: "src/a.ts" };
    expect(evidenceFilePath(target, records)).toBe("src/a.ts");
    expect(evidenceFilePath(target, records)).toBe("src/a.ts");
    expect(reads).toBe(1);
    expect(evidenceFilePath(target, [{ path: "src/other.ts" }] as FileRecord[])).toBeUndefined();
  });
});
