import type { Finding } from "@repodna/schema";

/**
 * Stable React keys for a particular analysis, including imported findings
 * with duplicate or empty IDs. Use original array positions, never positions
 * in a filtered or sorted list, to preserve the correct expanded card.
 * A new analysis scope prevents an expanded card leaking to another dataset.
 * Nothing is written back into a RepositoryDNA artifact.
 */
export function findingRenderKeys(
  findings: readonly Finding[],
  scope: string,
): WeakMap<Finding, string> {
  const keys = new WeakMap<Finding, string>();
  findings.forEach((finding, index) => keys.set(finding, `${scope}:${index}`));
  return keys;
}
