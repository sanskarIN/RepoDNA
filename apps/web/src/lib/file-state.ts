import type { FileRecord } from "@repodna/schema";

/** The provenance and processing flags available in the Files explorer. */
export type FileStateFilter = "all" | "regular" | "generated" | "vendored" | "binary" | "skipped";

type FileState = Pick<FileRecord, "generated" | "vendored" | "binary" | "skipped">;

/** Multiple flags may overlap; regular means none of the special flags applies. */
export function matchesFileState(file: FileState, filter: FileStateFilter): boolean {
  switch (filter) {
    case "all":
      return true;
    case "regular":
      return !file.generated && !file.vendored && !file.binary && !file.skipped;
    case "generated":
      return Boolean(file.generated);
    case "vendored":
      return Boolean(file.vendored);
    case "binary":
      return Boolean(file.binary);
    case "skipped":
      return Boolean(file.skipped);
  }
}
