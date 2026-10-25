import { describe, expect, it } from "vitest";
import { folderFromEntry, folderFromList, safeRelativePath } from "./input";

function chosen(path: string): File {
  const file = new File(["x"], path.split("/").pop() ?? path);
  Object.defineProperty(file, "webkitRelativePath", { value: path });
  return file;
}

function entry(name: string): FileSystemEntry {
  return {
    name,
    isDirectory: false,
    isFile: true,
    file: (resolve: (file: File) => void) => resolve(new File(["x"], name)),
  } as unknown as FileSystemEntry;
}

function folder(name: string, children: FileSystemEntry[]): FileSystemDirectoryEntry {
  return {
    name,
    isDirectory: true,
    isFile: false,
    createReader: () => {
      let offset = 0;
      return {
        readEntries: (resolve: (entries: FileSystemEntry[]) => void) => {
          resolve(children.slice(offset, offset + 2));
          offset += 2;
        },
      };
    },
  } as unknown as FileSystemDirectoryEntry;
}

describe("browser folder import validation", () => {
  it("accepts ordinary nested paths", () => {
    expect(safeRelativePath("src/lib.rs")).toBe(true);
    expect(safeRelativePath("src/.gitignore")).toBe(true);
  });

  it("rejects malformed, absolute and escaping paths", () => {
    const invalid = [
      "",
      "/etc/passwd",
      "../secret",
      "src/../secret",
      "src/./file",
      "src//file",
      "src/",
      "C:/private",
      "src/file:stream",
      "src\\\\other",
      "unsafe\0path",
    ];
    for (const path of invalid) {
      expect(safeRelativePath(path)).toBe(false);
    }
  });

  it("rejects mixed roots in selected folder files", () => {
    const files = [chosen("alpha/src/main.rs"), chosen("beta/README.md")];
    expect(folderFromList(files)).toBeNull();
  });

  it("rejects rooted and rootless entries in a single selection", () => {
    const files = [chosen("alpha/src/main.rs"), chosen("readme.md")];
    expect(folderFromList(files)).toBeNull();
  });

  it("rejects duplicate paths from a selected folder", () => {
    const files = [chosen("alpha/src/main.rs"), chosen("alpha/src/main.rs")];
    expect(folderFromList(files)).toBeNull();
  });

  it("accepts unique files within a single selected root", () => {
    const files = [chosen("alpha/src/main.rs"), chosen("alpha/README.md")];
    const result = folderFromList(files);
    expect(result?.kind).toBe("folder");
    if (result?.kind !== "folder") throw new Error("Expected folder");
    expect(result.name).toBe("alpha");
    expect(result.files.map((file) => file.path)).toEqual(["src/main.rs", "README.md"]);
  });

  it("rejects traversal from a dropped folder", async () => {
    const root = folder("alpha", [entry("../secret.rs")]);
    await expect(folderFromEntry(root)).rejects.toThrow(/unsafe path/);
  });

  it("rejects backslashes in nested dropped entries", async () => {
    const nested = folder("src", [entry("bad\\\\name.rs")]);
    const root = folder("alpha", [nested]);
    await expect(folderFromEntry(root)).rejects.toThrow(/unsafe path/);
  });

  it("rejects duplicate names across dropped reader batches", async () => {
    const root = folder("alpha", [entry("main.rs"), entry("lib.rs"), entry("main.rs")]);
    await expect(folderFromEntry(root)).rejects.toThrow(/duplicate file paths/);
  });
});
