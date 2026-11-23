import {
  describeEvidence,
  isSuppressed,
  severityRank,
  type Confidence,
  type Finding,
  type FindingCategory,
  type Severity,
} from "@repodna/schema";

/** Filters shared by the interactive findings list and its CSV export. */
export interface FindingFilters {
  query: string;
  severity: "all" | Severity;
  category: "all" | FindingCategory;
  confidence: "all" | Confidence;
  showSuppressed: boolean;
}

function searchable(finding: Finding): string {
  return [
    finding.title,
    finding.summary,
    finding.rule,
    finding.category,
    ...(finding.paths ?? []),
    ...(finding.evidence ?? []).map(describeEvidence),
  ]
    .join(" ")
    .toLowerCase();
}

/** Match all query terms against a single normalized text string per candidate. */
function matchesTerms(finding: Finding, terms: readonly string[]): boolean {
  if (terms.length === 0) return true;
  const text = searchable(finding);
  return terms.every((term) => text.includes(term));
}

/**
 * Filter each record directly, not by its ID: older or imported artifacts can have
 * repeated identifiers that must not cause unrelated findings to match a query.
 */
export function filterFindings(findings: readonly Finding[], filters: FindingFilters): Finding[] {
  const terms = filters.query.toLowerCase().split(/\s+/).filter(Boolean);
  return findings
    .filter(
      (finding) =>
        (filters.showSuppressed || !isSuppressed(finding)) &&
        (filters.severity === "all" || finding.severity === filters.severity) &&
        (filters.category === "all" || finding.category === filters.category) &&
        (filters.confidence === "all" || finding.confidence === filters.confidence) &&
        matchesTerms(finding, terms),
    )
    .sort((a, b) => severityRank(b.severity) - severityRank(a.severity));
}
