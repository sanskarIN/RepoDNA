// Opening an analysis file that the user chose or dropped on the page.

import { useApp } from "../state";
import { readFile } from "./demo";

/** Opens an analysis file in place of the open analysis, if any. */
export function useOpenFile(): (file: File) => Promise<void> {
  const { open } = useApp();
  return async (file) => {
    const loaded = await readFile(file);
    open(
      {
        dna: loaded.artifact,
        origin: { kind: "file", name: file.name },
        warnings: loaded.warnings,
      },
      { text: loaded.text, size: file.size },
    );
  };
}

/** Whether a drag carries files, rather than text or a link from the page. */
export function carriesFiles(event: DragEvent): boolean {
  return Array.from(event.dataTransfer?.types ?? []).includes("Files");
}
