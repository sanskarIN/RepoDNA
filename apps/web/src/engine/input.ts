// What to analyze in the browser: the files of a folder the user chose or dropped, or an
// archive.

import { keep } from "@repodna/wasm";
import type { InputFile } from "./protocol";
import { addFileBytes, enumerateFolder, type FolderTraversalOptions } from "./folderTraversal";

export { keep };

/** A folder's files, or an archive, ready to analyze. */
export type AnalysisInput =
  | { kind: "folder"; name: string; files: InputFile[]; bytes: number }
  | { kind: "archive"; file: File };

/**
 * The browser usually supplies relative paths, but drag-and-drop entries and synthetic
 * FileLists can contain unsafe names. Never give absolute or escaping paths to WASM.
 */
export function safeRelativePath(path: string): boolean {
  return (
    path.length > 0 &&
    !path.includes("\\") &&
    !path.includes(":") &&
    !path.includes("\0") &&
    path.split("/").every((part) => part !== "" && part !== "." && part !== "..")
  );
}

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
  const seen = new Set<string>();
  let name = "";
  let hasRoot: boolean | null = null;
  let bytes = 0;
  for (let index = 0; index < list.length; index += 1) {
    const file = list[index];
    if (!file) return null;
    const relative = file.webkitRelativePath || file.name;
    if (!safeRelativePath(relative)) return null;
    const slash = relative.indexOf("/");
    const rooted = slash > 0;
    // A FileList from a folder chooser must not silently combine two repositories
    // or mix rooted entries with unrelated top-level file names.
    if (hasRoot !== null && hasRoot !== rooted) return null;
    hasRoot = rooted;
    if (rooted) {
      const root = relative.slice(0, slash);
      if (name && root !== name) return null;
      name = root;
    }
    const path = rooted ? relative.slice(slash + 1) : relative;
    if (keep(path)) {
      if (seen.has(path)) return null;
      seen.add(path);
      files.push({ path, file });
      try {
        bytes = addFileBytes(bytes, file.size);
      } catch {
        return null;
      }
    }
  }
  return files.length > 0 ? { kind: "folder", name: name || "repository", files, bytes } : null;
}

/** Lists a dropped folder without unbounded parallel reads. The optional abort
 * signal stops enumeration itself, rather than merely ignoring its final result. */
export async function folderFromEntry(
  folder: FileSystemDirectoryEntry,
  onCount?: (files: number) => void,
  options?: FolderTraversalOptions,
): Promise<AnalysisInput | null> {
  const { files, bytes } = await enumerateFolder(folder, keep, onCount, options);
  return files.length > 0 ? { kind: "folder", name: folder.name, files, bytes } : null;
}

/**
 * The folder dropped with a drag, if a folder was dropped. It must be asked for while the
 * drop is handled: the browser forgets what was dropped afterwards.
 */
export function droppedFolder(transfer: DataTransfer | null): FileSystemDirectoryEntry | null {
  for (const item of Array.from(transfer?.items ?? [])) {
    if (item.kind !== "file") {
      continue;
    }
    const entry = item.webkitGetAsEntry?.();
    if (entry?.isDirectory) {
      return entry as FileSystemDirectoryEntry;
    }
  }
  return null;
}
