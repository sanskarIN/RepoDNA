import { type Finding, isSuppressed } from "@repodna/schema";

/**
 * Up to `limit` findings worth looking at first, from findings in display order (as stored
 * in the artifact): the first of each rule, so one kind of finding does not crowd out the
 * others, then the remaining ones in order. Suppressed findings are skipped. The same
 * selection as the command line and reports.
 */
export function highlights(findings: readonly Finding[], limit: number): Finding[] {
  const active = findings.filter((finding) => !isSuppressed(finding));
  const rules = new Set<string>();
  // Keep original positions, not IDs: imported artifacts may contain duplicate IDs
  // and a map keyed by ID would silently overwrite earlier positions.
  const firsts: Array<{ finding: Finding; index: number }> = [];
  const rest: Array<{ finding: Finding; index: number }> = [];
  for (const [index, finding] of active.entries()) {
    (rules.has(finding.rule) ? rest : firsts).push({ finding, index });
    rules.add(finding.rule);
  }
  const count = Math.max(0, limit);
  const picked = firsts.slice(0, count);
  picked.push(...rest.slice(0, count - picked.length));
  return picked.sort((a, b) => a.index - b.index).map(({ finding }) => finding);
}
