// Saving generated text from the browser without a server round trip.

import type { RepositoryDna } from "@repodna/schema";

/** Offers `text` as a file download. */
export function downloadText(name: string, text: string, type: string): void {
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
