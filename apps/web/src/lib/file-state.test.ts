import { describe, expect, it } from "vitest";
import type { FileRecord } from "@repodna/schema";
import { matchesFileState, type FileStateFilter } from "./file-state";

function file(flags: Record<string, unknown> = {}): FileRecord {
  return {
    generated: false,
    vendored: false,
    binary: false,
    skipped: null,
    ...flags,
  } as unknown as FileRecord;
}

describe("Files explorer state filters", () => {
  it("includes all files by default", () => {
    expect(matchesFileState(file({ generated: true }), "all")).toBe(true);
  });

  it("matches the flag requested without excluding overlapping classifications", () => {
    const special = file({ generated: true, vendored: true, binary: true, skipped: "too-large" });
    const flags: FileStateFilter[] = ["generated", "vendored", "binary", "skipped"];
    for (const flag of flags) {
      expect(matchesFileState(special, flag)).toBe(true);
    }
    expect(matchesFileState(special, "regular")).toBe(false);
  });

  it("identifies ordinary files and excludes special states from that subset", () => {
    const ordinary = file();
    expect(matchesFileState(ordinary, "regular")).toBe(true);
    for (const flag of ["generated", "vendored", "binary", "skipped"] as const) {
      expect(matchesFileState(ordinary, flag)).toBe(false);
    }
  });

  it("matches a skipped file even when it is not binary", () => {
    const unreadable = file({ skipped: "unreadable" });
    expect(matchesFileState(unreadable, "skipped")).toBe(true);
    expect(matchesFileState(unreadable, "binary")).toBe(false);
  });
});
