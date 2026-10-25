import {
  categoryLabel,
  confidenceLabel,
  isSuppressed,
  severityLabel,
  type Finding,
} from "@repodna/schema";
import { toCsv, type Column } from "../components/DataTable";

/** Keep exported values plain text, using DataTable's spreadsheet-safe CSV escaping. */
const columns: Column<Finding>[] = [
  { key: "severity", header: "Severity", cell: (finding) => severityLabel(finding.severity) },
  { key: "category", header: "Category", cell: (finding) => categoryLabel(finding.category) },
  { key: "rule", header: "Rule", cell: (finding) => finding.rule },
  { key: "title", header: "Title", cell: (finding) => finding.title },
  { key: "summary", header: "Summary", cell: (finding) => finding.summary },
  {
    key: "confidence",
    header: "Confidence",
    cell: (finding) => confidenceLabel(finding.confidence),
  },
  { key: "paths", header: "Paths", cell: (finding) => (finding.paths ?? []).join("; ") },
  {
    key: "suppressed",
    header: "Suppressed",
    cell: (finding) => (isSuppressed(finding) ? "Yes" : "No"),
  },
];

/** Exports exactly the matching findings in their current display order. */
export function findingsCsv(findings: readonly Finding[]): string {
  return toCsv(findings, columns);
}
/**
 * Export the selected findings with all of their original, structured evidence.
 * This is a JSON array of findings, not a full RepositoryDNA artifact. The
 * ordering and suppression selection are owned by the Findings view.
 */
export function findingsJson(findings: readonly Finding[]): string {
  return `${JSON.stringify(findings, null, 2)}\n`;
}
