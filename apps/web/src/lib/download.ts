// Saving generated text from the browser without a server round trip.

import { useCallback, useState } from "react";
import type { RepositoryDna } from "@repodna/schema";
import { isDesktop } from "./backend";

/**
 * Offers `text` as a file: a download in a browser, and a save dialog in the desktop app,
 * whose window does not download files. Resolves once the file is saved or the dialog is
 * cancelled.
 */
export async function downloadText(name: string, text: string, type: string): Promise<void> {
  if (isDesktop()) {
    const { saveFile } = await import("./desktop");
    await saveFile(name, text);
    return;
  }
  const url = URL.createObjectURL(new Blob([text], { type }));
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.rel = "noopener";
  document.body.append(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** The file name `repodna export` uses: `repodna-<name>-<date>.repodna`. */
export function artifactFileName(dna: RepositoryDna): string {
  const name = dna.identity.name
    .split("")
    .map((c) => (/[A-Za-z0-9._-]/.test(c) ? c.toLowerCase() : "-"))
    .join("")
    .replace(/^[-.]+|[-.]+$/g, "");
  return `repodna-${name || "repository"}-${dna.analysisMetadata.generatedAt.slice(0, 10)}.repodna`;
}

/** `downloadText`, and why the last file could not be saved, for the page to show. */
export function useDownload(): [
  download: (name: string, text: string, type: string) => void,
  failure: string | null,
] {
  const [failure, setFailure] = useState<string | null>(null);
  const download = useCallback((name: string, text: string, type: string) => {
    setFailure(null);
    downloadText(name, text, type).catch((reason: unknown) =>
      setFailure(
        `${name} could not be saved: ${reason instanceof Error ? reason.message : String(reason)}`,
      ),
    );
  }, []);
  return [download, failure];
}
