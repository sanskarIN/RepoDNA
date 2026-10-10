import { Directory, File as WasiFile } from "@bjorn3/browser_wasi_shim";
import { describe, expect, it, vi } from "vitest";
import { droppedFolder, folderFromEntry, folderFromList, isArchive, keep } from "./input";
import { fileRoot, folderRoot } from "./run";

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

/** The entry at `path` in a file system root, or undefined. */
function at(root: Map<string, unknown>, path: string): unknown {
  let current: unknown = root;
  for (const part of path.split("/")) {
    const contents: Map<string, unknown> | undefined =
      current instanceof Map ? current : (current as Directory | undefined)?.contents;
    current = contents?.get(part);
  }
  return current;
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

  it("finds a dropped folder among the dropped items", () => {
    const directory = folder("widget", []);
    const transfer = {
      items: [
        { kind: "string", webkitGetAsEntry: () => null },
        { kind: "file", webkitGetAsEntry: () => directory },
      ],
    } as unknown as DataTransfer;
    expect(droppedFolder(transfer)).toBe(directory);
    const file = { items: [{ kind: "file", webkitGetAsEntry: () => entry("a.zip") }] };
    expect(droppedFolder(file as unknown as DataTransfer)).toBeNull();
    expect(droppedFolder(null)).toBeNull();
  });
});

describe("file system", () => {
  it("puts a folder under /work and reads each file once, when opened", () => {
    const read = vi.fn((blob: Blob) => new Uint8Array(blob.size));
    const root = folderRoot(
      "widget",
      [
        { path: "src/lib.rs", file: new Blob(["pub fn a() {}"]) },
        { path: "./README.md", file: new Blob(["# W"]) },
        { path: "../escape.txt", file: new Blob(["no"]) },
        { path: "src/../../escape.txt", file: new Blob(["no"]) },
        { path: ".git/HEAD", file: new Blob(["ref"]) },
      ],
      read,
    );
    expect(at(root, "tmp")).toBeInstanceOf(Directory);
    const lib = at(root, "work/widget/src/lib.rs") as WasiFile;
    expect(lib).toBeInstanceOf(WasiFile);
    expect(at(root, "work/widget/README.md")).toBeInstanceOf(WasiFile);
    expect(at(root, "work/widget/.git")).toBeUndefined();
    expect(at(root, "work/escape.txt")).toBeUndefined();
    expect(at(root, "work/widget/escape.txt")).toBeUndefined();

    // Listing asks for sizes, which do not need the contents.
    expect(lib.size).toBe(13n);
    expect(lib.stat().size).toBe(13n);
    expect(read).not.toHaveBeenCalled();
    expect(lib.data.byteLength).toBe(13);
    expect(lib.data.byteLength).toBe(13);
    expect(read).toHaveBeenCalledTimes(1);
    expect(lib.readonly).toBe(true);
  });

  it("puts a single file under /work", () => {
    const root = fileRoot("analysis.repodna", new Blob(["{}"]), () => new Uint8Array(2));
    expect((at(root, "work/analysis.repodna") as WasiFile).size).toBe(2n);
  });
});
