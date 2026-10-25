import { describe, expect, it } from "vitest";
import {
  droppedFolder,
  folderFromEntry,
  folderFromList,
  isArchive,
  keep,
  safeRelativePath,
} from "./input";

/** A file as a folder chooser gives it: named by its path from the chosen folder. */
function chosen(path: string, text = "x"): File {
  const file = new File([text], path.split("/").pop() ?? path);
  Object.defineProperty(file, "webkitRelativePath", { value: path });
  return file;
}

/** A dropped folder's entry, which lists its children two at a time. */
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

function entry(name: string, text = "x"): FileSystemEntry {
  return {
    name,
    isDirectory: false,
    isFile: true,
    file: (resolve: (file: File) => void) => resolve(new File([text], name)),
  } as unknown as FileSystemEntry;
}

describe("input", () => {
  it("recognizes archives and skips version control", () => {
    expect(["a.zip", "B.TAR.GZ", "c.tgz", "d.tar"].every(isArchive)).toBe(true);
    expect(isArchive("e.repodna")).toBe(false);
    expect(keep("src/main.rs")).toBe(true);
    expect(keep(".git/HEAD")).toBe(false);
    expect(keep("vendor/lib/.svn/entries")).toBe(false);
    expect(keep(".github/workflows/ci.yml")).toBe(true);
  });

  it("takes a chosen folder's name and paths", () => {
    const input = folderFromList([
      chosen("widget/src/lib.rs", "pub fn a() {}"),
      chosen("widget/.git/config"),
      chosen("widget/README.md", "# W"),
    ]);
    expect(input).toMatchObject({ kind: "folder", name: "widget", bytes: 16 });
    expect(input?.kind === "folder" && input.files.map((file) => file.path)).toEqual([
      "src/lib.rs",
      "README.md",
    ]);
    expect(folderFromList([chosen("empty/.git/HEAD")])).toBeNull();
  });

  it("lists a dropped folder in batches, and counts as it goes", async () => {
    const tree = folder("widget", [
      entry("README.md"),
      folder("src", [entry("a.rs"), entry("b.rs"), entry("c.rs")]),
      folder(".git", [entry("HEAD")]),
    ]);
    const counts: number[] = [];
    const input = await folderFromEntry(tree, (count) => counts.push(count));
    expect(input?.kind === "folder" && input.files.map((file) => file.path).sort()).toEqual([
      "README.md",
      "src/a.rs",
      "src/b.rs",
      "src/c.rs",
    ]);
    expect(input).toMatchObject({ name: "widget", bytes: 4 });
    expect(counts.at(-1)).toBe(4);
  });

  it("accepts normal relative paths and rejects traversal, absolute and malformed paths", () => {
    expect(safeRelativePath("src/core/module.rs")).toBe(true);
    expect(safeRelativePath(".gitignore")).toBe(true);
    for (const path of [
      "",
      "/etc/passwd",
      "../secret",
      "src/../secret",
      "src/./module.rs",
      "src//module.rs",
      "src/",
      "C:/private",
      "src/file:alternate",
      "src\\\\windows",
      "src/unsafe\0name",
    ]) {
      expect(safeRelativePath(path), path).toBe(false);
    }
  });

  it("does not combine separate folder roots or rooted and unrooted paths", () => {
    expect(folderFromList([chosen("first/src/main.rs"), chosen("other/README.md")])).toBeNull();
    expect(folderFromList([chosen("first/src/main.rs"), chosen("readme.md")])).toBeNull();
    expect(folderFromList([chosen("first/src/../secret.rs")])).toBeNull();
    const good = folderFromList([chosen("first/src/main.rs"), chosen("first/README.md")]);
    expect(good?.kind).toBe("folder");
    expect(good?.name).toBe("first");
  });

  it("rejects duplicate chosen file paths without double-counting files", () => {
    const chosenFiles = [chosen("widget/src/main.rs"), chosen("widget/src/main.rs")];
    expect(folderFromList(chosenFiles)).toBeNull();
  });

  it("rejects unsafe dropped paths before passing any files to the engine", async () => {
    const traversal = folder("widget", [entry("../outside.rs")]);
    await expect(folderFromEntry(traversal)).rejects.toThrow(/unsafe path/);
    const backslash = folder("widget", [folder("src", [entry("bad\\\\file.rs")])]);
    await expect(folderFromEntry(backslash)).rejects.toThrow(/unsafe path/);
  });

  it("rejects repeated file paths across separate dropped-folder reader batches", async () => {
    const files = [entry("main.rs"), entry("helper.rs"), entry("main.rs")];
    await expect(folderFromEntry(folder("widget", files))).rejects.toThrow(
      /duplicate file paths/,
    );
  });

  it("finds a dropped folder among the dropped items", () => {
    const directory = folder("widget", []);
    const transfer = {
      items: [
        { kind: "string", webkitGetAsEntry: () => null },
        { kind: "file", webkitGetAsEntry: () => directory },
      ],
    } as unknown as DataTransfer;
    expect(droppedFolder(transfer)).toBe(directory);
    // A file (or an unreadable entry) before the directory must not hide the folder.
    const mixed = {
      items: [
        { kind: "file", webkitGetAsEntry: () => entry("README.md") },
        { kind: "file", webkitGetAsEntry: () => null },
        { kind: "file", webkitGetAsEntry: () => directory },
      ],
    } as unknown as DataTransfer;
    expect(droppedFolder(mixed)).toBe(directory);
    const file = { items: [{ kind: "file", webkitGetAsEntry: () => entry("a.zip") }] };
    expect(droppedFolder(file as unknown as DataTransfer)).toBeNull();
    expect(droppedFolder(null)).toBeNull();
  });
});
