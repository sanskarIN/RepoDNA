import type { InputFile } from "./protocol";
import { browserClock, type RequestClock } from "../lib/requestDeadline";

/** Bound the work performed before the analyzer receives the selected files.
 * Counts include ignored entries so an ignored tree cannot keep listing forever.
 */
export interface FolderLimits {
  entries: number;
  depth: number;
  pathBytes: number;
  callbackMs: number;
}

export const DEFAULT_FOLDER_LIMITS: Readonly<FolderLimits> = {
  entries: 500_000,
  depth: 256,
  pathBytes: 16_384,
  callbackMs: 30_000,
};

export interface FolderTraversalOptions {
  signal?: AbortSignal;
  limits?: Readonly<FolderLimits>;
  clock?: RequestClock;
}

export class FolderReadError extends Error {
  override name = "FolderReadError";
}

function checkAbort(signal?: AbortSignal): void {
  if (signal?.aborted) throw new DOMException("Folder listing cancelled.", "AbortError");
}

const encoder = new TextEncoder();

/** FileSystemEntry.name is one component, not an already joined path. */
export function validEntryName(name: string): boolean {
  return name.length > 0 && name !== "." && name !== ".." && !/[\\/:\0]/.test(name);
}

function validateLimits(limits: Readonly<FolderLimits>): void {
  for (const value of Object.values(limits)) {
    if (!Number.isSafeInteger(value) || value < 1) {
      throw new RangeError("Folder enumeration limits must be positive safe integers.");
    }
  }
}

/** Convert callback-only browser APIs to cancellable observations. Late callbacks
 * are harmless, even when a browser cannot cancel the native directory request.
 */
function read<T>(
  label: string,
  call: (resolve: (value: T) => void, reject: (reason: unknown) => void) => void,
  signal: AbortSignal | undefined,
  clock: RequestClock,
  timeout: number,
): Promise<T> {
  return new Promise((resolve, reject) => {
    checkAbort(signal);
    let settled = false;
    let timer: number | null = null;
    const complete = (finish: () => void) => {
      if (settled) return;
      settled = true;
      if (timer !== null) clock.clear(timer);
      signal?.removeEventListener("abort", abort);
      finish();
    };
    const abort = () => complete(() => reject(new DOMException("Folder listing cancelled.", "AbortError")));
    signal?.addEventListener("abort", abort, { once: true });
    timer = clock.set(
      () => complete(() => reject(new FolderReadError(`${label} did not respond in time.`))),
      timeout,
    );
    try {
      call(
        (value) => complete(() => resolve(value)),
        (reason) => complete(() => reject(reason)),
      );
    } catch (reason) {
      complete(() => reject(reason));
    }
  });
}

interface Frame {
  reader: FileSystemDirectoryReader;
  prefix: string;
  depth: number;
  batch: FileSystemEntry[];
  offset: number;
}

export interface EnumeratedFolder {
  files: InputFile[];
  bytes: number;
}

/** Validate size arithmetic without silently losing precision in a large selection. */
export function addFileBytes(total: number, size: number): number {
  if (!Number.isSafeInteger(size) || size < 0 || !Number.isSafeInteger(total + size)) {
    throw new FolderReadError("The selected file sizes exceed the supported range.");
  }
  return total + size;
}

/**
 * Depth-first enumeration with one outstanding callback, a bounded stack and
 * explicit cancellation. Never starts an unbounded Promise.all of directory
 * or file operations. A duplicate directory is rejected just like a duplicate
 * file; a broken reader cannot repeatedly return the same path forever.
 */
export async function enumerateFolder(
  root: FileSystemDirectoryEntry,
  include: (path: string) => boolean,
  onCount?: (files: number) => void,
  options: FolderTraversalOptions = {},
): Promise<EnumeratedFolder> {
  const { signal, limits = DEFAULT_FOLDER_LIMITS, clock = browserClock } = options;
  validateLimits(limits);
  checkAbort(signal);
  const frame = (entry: FileSystemDirectoryEntry, prefix: string, depth: number): Frame => ({
    reader: entry.createReader(),
    prefix,
    depth,
    batch: [],
    offset: 0,
  });
  const stack: Frame[] = [frame(root, "", 0)];
  const seen = new Set<string>();
  const files: InputFile[] = [];
  let bytes = 0;
  let visited = 0;
  let lastReported = -1;
  const report = () => {
    if (lastReported !== files.length) {
      lastReported = files.length;
      onCount?.(files.length);
    }
    checkAbort(signal);
  };
  while (stack.length) {
    checkAbort(signal);
    const current = stack[stack.length - 1]!;
    if (current.offset === current.batch.length) {
      current.batch = await read<FileSystemEntry[]>(
        "The directory reader",
        (resolve, reject) => current.reader.readEntries(resolve, reject),
        signal,
        clock,
        limits.callbackMs,
      );
      checkAbort(signal);
      current.offset = 0;
      if (!Array.isArray(current.batch)) throw new FolderReadError("Invalid directory reader response.");
      if (!current.batch.length) {
        stack.pop();
        report();
        continue;
      }
      // Count the entire batch before scheduling any reads from it.
      visited += current.batch.length;
      if (visited > limits.entries) {
        throw new FolderReadError(`The selected folder exceeds ${limits.entries} directory entries.`);
      }
    }
    const entry = current.batch[current.offset++];
    if (!entry || !validEntryName(entry.name)) {
      throw new FolderReadError("The dropped folder contains an unsafe path.");
    }
    const path = current.prefix + entry.name;
    if (encoder.encode(path).length > limits.pathBytes) {
      throw new FolderReadError("The dropped folder contains an excessively long path.");
    }
    if (seen.has(path)) {
      throw new FolderReadError("The dropped folder contains duplicate file paths or directories.");
    }
    seen.add(path);
    if (!include(path)) continue;
    if (entry.isDirectory && !entry.isFile) {
      if (current.depth + 1 > limits.depth) {
        throw new FolderReadError("The selected folder exceeds the supported directory depth.");
      }
      stack.push(frame(entry as FileSystemDirectoryEntry, `${path}/`, current.depth + 1));
    } else if (entry.isFile && !entry.isDirectory) {
      const file = await read<File>(
        "A selected file",
        (resolve, reject) => (entry as FileSystemFileEntry).file(resolve, reject),
        signal,
        clock,
        limits.callbackMs,
      );
      checkAbort(signal);
      bytes = addFileBytes(bytes, file.size);
      files.push({ path, file });
      // Avoid one React render for every file in large directories.
      if (files.length % 128 === 0) report();
    } else {
      throw new FolderReadError("The dropped folder contains an unknown entry type.");
    }
  }
  report();
  return { files, bytes };
}
