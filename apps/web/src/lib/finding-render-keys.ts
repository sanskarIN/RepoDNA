import type { Finding } from "@repodna/schema";

/**
 * Preserve distinct React keys for imported findings with repeated IDs.
 * Keys follow original analysis positions, not the current filtered/sorted
 * position, so opening or closing a search filter cannot swap a card's UI
 * state with another finding. No fields are added to the artifact.
 */
export function findingRenderKeys(findings: readonly Finding[]): WeakMap<Finding, number> {
  const keys = new WeakMap<Finding, number>();
  findings.forEach((finding, index) => keys.set(finding, index));
  return keys;
}
