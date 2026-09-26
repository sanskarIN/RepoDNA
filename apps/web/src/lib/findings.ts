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
  const firsts: Finding[] = [];
  const rest: Finding[] = [];
  for (const finding of active) {
    (rules.has(finding.rule) ? rest : firsts).push(finding);
    rules.add(finding.rule);
  }
  const picked = firsts.slice(0, limit);
  picked.push(...rest.slice(0, limit - picked.length));
  const position = new Map(active.map((finding, index) => [finding.id, index]));
  return picked.sort((a, b) => (position.get(a.id) ?? 0) - (position.get(b.id) ?? 0));
}
