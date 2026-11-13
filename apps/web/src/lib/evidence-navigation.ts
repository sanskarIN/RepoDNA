import { evidencePath, type Evidence, type FileRecord } from "@repodna/schema";

// An analysis's file array is immutable while it is displayed. Index by that
// array identity so every evidence list shares one lookup table; old indices
// can be collected when their analysis is no longer in use.
const indices = new WeakMap<readonly FileRecord[], ReadonlySet<string>>();

function filePathIndex(files: readonly FileRecord[]): ReadonlySet<string> {
  let index = indices.get(files);
  if (!index) {
    index = new Set(files.map((file) => file.path));
    indices.set(files, index);
  }
  return index;
}

/**
 * Return a destination only for file-backed evidence actually present in the
 * loaded artifact. Directory and non-path evidence must not claim to open a
 * file; missing/ignored/truncated files remain unlinked.
 */
export function evidenceFilePath(
  evidence: Evidence,
  files: readonly FileRecord[] | undefined,
): string | undefined {
  if (!files || evidence.kind === "directory") return undefined;
  const path = evidencePath(evidence);
  return path && filePathIndex(files).has(path) ? path : undefined;
}
