import { describe, expect, it } from "vitest";
import { Clock, flush } from "../lib/controllerTestSupport";
import {
  addFileBytes,
  DEFAULT_FOLDER_LIMITS,
  enumerateFolder,
  validEntryName,
  type FolderLimits,
} from "./folderTraversal";

type Read = (resolve: (entries: FileSystemEntry[]) => void, reject: (error: Error) => void) => void;
type Load = (resolve: (file: File) => void, reject: (error: Error) => void) => void;

function file(name: string, content = "x", load?: Load): FileSystemFileEntry {
  return {
    name,
    isFile: true,
    isDirectory: false,
    file: load ?? ((resolve: (file: File) => void) => resolve(new File([content], name))),
  } as unknown as FileSystemFileEntry;
}

function directory(name: string, batches: FileSystemEntry[][], custom?: Read): FileSystemDirectoryEntry {
  return {
    name,
    isFile: false,
    isDirectory: true,
    createReader() {
      let index = 0;
      return {
        readEntries: custom ?? ((resolve: (entries: FileSystemEntry[]) => void) => resolve(batches[index++] ?? [])),
      };
    },
  } as unknown as FileSystemDirectoryEntry;
}

const include = () => true;
const limits = (overrides: Partial<FolderLimits>) => ({ ...DEFAULT_FOLDER_LIMITS, ...overrides });

async function error<T>(promise: Promise<T>): Promise<Error> {
  try {
    await promise;
  } catch (reason) {
    if (reason instanceof Error) return reason;
    throw reason;
  }
  throw new Error("Expected rejection");
}

describe("bounded dropped-folder enumeration", () => {
  it("preserves deterministic depth-first order and exact byte totals", async () => {
    const root = directory("root", [[file("first", "aa"), directory("src", [[file("lib", "bbb")], [file("main", "cccc")]])], [file("last", "ddddd")]]);
    const counts: number[] = [];
    const clock = new Clock();
    const result = await enumerateFolder(root, include, (count) => counts.push(count), { clock });
    expect(result.files.map((entry) => entry.path)).toEqual(["first", "src/lib", "src/main", "last"]);
    expect(result.bytes).toBe(14);
    expect(counts).toEqual([3, 4]);
    expect(clock.pending).toBe(0);
  });

  it("does not read excluded directories or files", async () => {
    let reads = 0;
    const ignored = directory("vendor", [], () => { reads++; });
    const ignoredFile = file("ignored", "", () => { reads++; });
    const root = directory("root", [[ignored, ignoredFile, file("main")]]);
    const result = await enumerateFolder(root, (path) => path === "main", undefined, { clock: new Clock() });
    expect(reads).toBe(0);
    expect(result.files.map((item) => item.path)).toEqual(["main"]);
  });

  it("does not open more than one callback operation at once", async () => {
    const operations: Array<() => void> = [];
    const first = file("first", "", (resolve) => operations.push(() => resolve(new File(["a"], "first"))));
    const second = file("second", "", (resolve) => operations.push(() => resolve(new File(["b"], "second"))));
    const clock = new Clock();
    const result = enumerateFolder(directory("root", [[first, second]]), include, undefined, { clock });
    await flush();
    expect(operations.length).toBe(1);
    operations.shift()?.();
    await flush();
    expect(operations.length).toBe(1);
    operations.shift()?.();
    expect((await result).bytes).toBe(2);
    expect(clock.pending).toBe(0);
  });

  it("does not access a root when cancellation was already requested", async () => {
    const controller = new AbortController();
    controller.abort();
    let reads = 0;
    const root = directory("root", [], () => { reads++; });
    const result = await error(enumerateFolder(root, include, undefined, { signal: controller.signal, clock: new Clock() }));
    expect(result.name).toBe("AbortError");
    expect(reads).toBe(0);
  });

  it("cancels an unanswered directory callback and ignores late delivery", async () => {
    let deliver: ((entries: FileSystemEntry[]) => void) | undefined;
    let reads = 0;
    const root = directory("root", [], (resolve) => { reads++; deliver = resolve; });
    const controller = new AbortController();
    const clock = new Clock();
    const counts: number[] = [];
    const result = error(enumerateFolder(root, include, (count) => counts.push(count), { signal: controller.signal, clock }));
    controller.abort();
    expect((await result).name).toBe("AbortError");
    deliver?.([file("late")]);
    await flush();
    expect(reads).toBe(1);
    expect(counts).toEqual([]);
    expect(clock.pending).toBe(0);
  });

  it("cancels a pending file read without opening the next file", async () => {
    let deliver: ((file: File) => void) | undefined;
    let secondReads = 0;
    const root = directory("root", [[file("first", "", (resolve) => { deliver = resolve; }), file("second", "", () => { secondReads++; })]]);
    const controller = new AbortController();
    const clock = new Clock();
    const result = error(enumerateFolder(root, include, undefined, { clock, signal: controller.signal }));
    await flush();
    controller.abort();
    expect((await result).name).toBe("AbortError");
    deliver?.(new File(["late"], "first"));
    await flush();
    expect(secondReads).toBe(0);
    expect(clock.pending).toBe(0);
  });

  it("honors cancellation from a progress subscriber", async () => {
    const controller = new AbortController();
    let later = 0;
    const first = directory("child", [[file("inside")]]);
    const root = directory("root", [[first, file("later", "", () => { later++; })]]);
    const result = await error(enumerateFolder(root, include, () => controller.abort(), { clock: new Clock(), signal: controller.signal }));
    expect(result.name).toBe("AbortError");
    expect(later).toBe(0);
  });

  it("times out a directory callback and clears its timer", async () => {
    const clock = new Clock();
    const result = error(enumerateFolder(directory("root", [], () => undefined), include, undefined, { clock, limits: limits({ callbackMs: 25 }) }));
    await clock.advance(25);
    expect((await result).message).toContain("directory reader did not respond");
    expect(clock.pending).toBe(0);
  });

  it("times out a file callback and ignores duplicate or late callbacks", async () => {
    const clock = new Clock();
    let deliver: ((file: File) => void) | undefined;
    const result = error(enumerateFolder(directory("root", [[file("main", "", (resolve) => { deliver = resolve; })]]), include, undefined, { clock, limits: limits({ callbackMs: 20 }) }));
    await flush();
    await clock.advance(20);
    expect((await result).message).toContain("selected file did not respond");
    deliver?.(new File(["late"], "main"));
    deliver?.(new File(["later"], "main"));
    expect(clock.pending).toBe(0);
  });

  it("consumes synchronous reader and file failures", async () => {
    const clock = new Clock();
    const root = directory("root", [], () => { throw new Error("reader failed"); });
    expect((await error(enumerateFolder(root, include, undefined, { clock }))).message).toBe("reader failed");
    const another = directory("root", [[file("main", "", () => { throw new Error("file failed"); })]]);
    expect((await error(enumerateFolder(another, include, undefined, { clock }))).message).toBe("file failed");
    expect(clock.pending).toBe(0);
  });

  it("propagates asynchronous failure without returning partial results", async () => {
    const root = directory("root", [[file("first"), file("second", "", (_resolve, reject) => reject(new Error("permission denied")))]]);
    expect((await error(enumerateFolder(root, include, undefined, { clock: new Clock() }))).message).toBe("permission denied");
  });

  it("bounds counts even when every entry would be excluded", async () => {
    const root = directory("root", [[file("a"), file("b"), file("c")]]);
    const result = await error(enumerateFolder(root, () => false, undefined, { clock: new Clock(), limits: limits({ entries: 2 }) }));
    expect(result.message).toContain("exceeds 2 directory entries");
  });

  it("bounds depth before opening the disallowed directory", async () => {
    let reads = 0;
    const deep = directory("deep", [], () => { reads++; });
    const root = directory("root", [[directory("nested", [[deep]])]]);
    const result = await error(enumerateFolder(root, include, undefined, { clock: new Clock(), limits: limits({ depth: 1 }) }));
    expect(result.message).toContain("directory depth");
    expect(reads).toBe(0);
  });

  it("counts UTF-8 bytes, not JavaScript code units, in path budgets", async () => {
    const root = directory("root", [[directory("日本", [[file("語")]])]]);
    const result = await error(enumerateFolder(root, include, undefined, { clock: new Clock(), limits: limits({ pathBytes: 9 }) }));
    expect(result.message).toContain("long path");
  });

  it("rejects repeated directory names before walking the duplicate", async () => {
    let reads = 0;
    const first = directory("src", [[file("a")]]);
    const second = directory("src", [], () => { reads++; });
    const root = directory("root", [[first], [second]]);
    const result = await error(enumerateFolder(root, include, undefined, { clock: new Clock() }));
    expect(result.message).toContain("duplicate file paths");
    expect(reads).toBe(0);
  });

  it("rejects duplicate names across file and directory entry types", async () => {
    const root = directory("root", [[file("same"), directory("same", [])]]);
    expect((await error(enumerateFolder(root, include, undefined, { clock: new Clock() }))).message).toContain("duplicate file paths");
  });

  it("rejects slash-delimited entry names rather than interpreting them as paths", async () => {
    const root = directory("root", [[file("src/main")]]);
    expect((await error(enumerateFolder(root, include, undefined, { clock: new Clock() }))).message).toContain("unsafe path");
    for (const name of ["", ".", "..", "a/b", "a\\b", "a:b", "a\0b"]) expect(validEntryName(name)).toBe(false);
    for (const name of ["main.rs", ".gitignore", "name with spaces", "日本語"]) expect(validEntryName(name)).toBe(true);
  });

  it("rejects unknown and contradictory entry types", async () => {
    for (const flags of [{ isFile: false, isDirectory: false }, { isFile: true, isDirectory: true }]) {
      const unknown = { name: "special", ...flags } as FileSystemEntry;
      const result = await error(enumerateFolder(directory("root", [[unknown]]), include, undefined, { clock: new Clock() }));
      expect(result.message).toContain("unknown entry type");
    }
  });

  it("reports an empty folder without inventing file counts", async () => {
    const counts: number[] = [];
    const result = await enumerateFolder(directory("root", []), include, (count) => counts.push(count), { clock: new Clock() });
    expect(result).toEqual({ files: [], bytes: 0 });
    expect(counts).toEqual([0]);
  });

  it("rejects negative, fractional and overflowing byte totals", () => {
    expect(addFileBytes(10, 5)).toBe(15);
    for (const size of [-1, Infinity, NaN, 1.5, Number.MAX_SAFE_INTEGER]) {
      let failed = false;
      try { addFileBytes(1, size); } catch { failed = true; }
      expect(failed).toBe(true);
    }
  });
});
