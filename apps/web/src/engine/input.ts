// What to analyze in the browser: the files of a folder the user chose or dropped, or an
// archive.

import { keep } from "@repodna/wasm";
import type { InputFile } from "./protocol";

export { keep };

/** A folder's files, or an archive, ready to analyze. */
export type AnalysisInput =
  | { kind: "folder"; name: string; files: InputFile[]; bytes: number }
  | { kind: "archive"; file: File };

/** Whether a file name is one of the archives RepoDNA analyzes. */
export function isArchive(name: string): boolean {
  return /\.(zip|tar|tar\.gz|tgz)$/i.test(name);
}

/**
 * The folder chosen with `<input type="file" webkitdirectory>`, whose files are named by
 * their path from the folder, or null when it has no files to analyze.
 */
export function folderFromList(list: ArrayLike<File>): AnalysisInput | null {
  const files: InputFile[] = [];
  let name = "";
  let bytes = 0;
  for (const file of Array.from(list)) {
    const relative = file.webkitRelativePath || file.name;
    const slash = relative.indexOf("/");
    name ||= slash > 0 ? relative.slice(0, slash) : "";
    const path = slash > 0 ? relative.slice(slash + 1) : relative;
    if (keep(path)) {
      files.push({ path, file });
      bytes += file.size;
    }
  }
  return files.length > 0 ? { kind: "folder", name: name || "repository", files, bytes } : null;
}

function entries(reader: FileSystemDirectoryReader): Promise<FileSystemEntry[]> {
  return new Promise((resolve, reject) => reader.readEntries(resolve, reject));
}

function file(entry: FileSystemFileEntry): Promise<File> {
  return new Promise((resolve, reject) => entry.file(resolve, reject));
}

/**
 * The files of a dropped folder. `onCount` hears how many files were found so far, since a
 * large folder takes a moment to list.
 */
export async function folderFromEntry(
  folder: FileSystemDirectoryEntry,
  onCount?: (files: number) => void,
): Promise<AnalysisInput | null> {
  const files: InputFile[] = [];
  let bytes = 0;
  const walk = async (directory: FileSystemDirectoryEntry, prefix: string): Promise<void> => {
    const reader = directory.createReader();
    // A reader gives a directory's entries in batches until it gives none.
    for (let batch = await entries(reader); batch.length > 0; batch = await entries(reader)) {
      const found = await Promise.all(
        batch.map(async (entry) => {
          const path = prefix + entry.name;
          if (!keep(path)) {
            return null;
          }
          if (entry.isDirectory) {
            await walk(entry as FileSystemDirectoryEntry, `${path}/`);
            return null;
          }
          return entry.isFile ? { path, file: await file(entry as FileSystemFileEntry) } : null;
        }),
      );
      for (const input of found) {
        if (input) {
          files.push(input);
          bytes += input.file.size;
        }
      }
      onCount?.(files.length);
    }
  };
  await walk(folder, "");
  return files.length > 0 ? { kind: "folder", name: folder.name, files, bytes } : null;
}

/**
 * The folder dropped with a drag, if a folder was dropped. It must be asked for while the
 * drop is handled: the browser forgets what was dropped afterwards.
 */
export function droppedFolder(transfer: DataTransfer | null): FileSystemDirectoryEntry | null {
  const item = Array.from(transfer?.items ?? []).find((candidate) => candidate.kind === "file");
  const entry = item?.webkitGetAsEntry?.() ?? null;
  return entry?.isDirectory ? (entry as FileSystemDirectoryEntry) : null;
}
