import {
  SEVERITIES,
  categoryLabel,
  confidenceLabel,
  isSuppressed,
  severityLabel,
  type Finding,
} from "@repodna/schema";
import { toCsv, type Column } from "../components/DataTable";
import { findingsMarkdown } from "./findings-markdown";

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

/**
 * Aggregate findings without copying repository-controlled text into the CSV.
 * Only fixed severity labels and numeric counts are emitted. Imported artifacts
 * with an unfamiliar severity are counted under Unknown instead of discarded.
 * The result is lower-disclosure, not a guarantee that counts are shareable.
 */
export function findingsCountsCsv(findings: readonly Finding[]): string {
  type CountRow = { severity: string; active: number; suppressed: number; total: number };
  const rows: CountRow[] = SEVERITIES.map((severity) => ({
    severity: severityLabel(severity),
    active: 0,
    suppressed: 0,
    total: 0,
  }));
  const unknown: CountRow = { severity: "Unknown", active: 0, suppressed: 0, total: 0 };
  let active = 0;
  let suppressed = 0;

  for (const finding of findings) {
    const index = SEVERITIES.indexOf(finding.severity);
    const row = index < 0 ? unknown : rows[index]!;
    row.total += 1;
    if (isSuppressed(finding)) {
      row.suppressed += 1;
      suppressed += 1;
    } else {
      row.active += 1;
      active += 1;
    }
  }

  if (unknown.total > 0) rows.push(unknown);
  rows.push({ severity: "Total", active, suppressed, total: findings.length });
  const summaryColumns: Column<CountRow>[] = [
    { key: "severity", header: "Severity", cell: (row) => row.severity },
    { key: "active", header: "Active", cell: (row) => row.active },
    { key: "suppressed", header: "Suppressed", cell: (row) => row.suppressed },
    { key: "total", header: "Total", cell: (row) => row.total },
  ];
  return toCsv(rows, summaryColumns);
}

/** Available formats for a local, user-reviewed findings export. */
export type FindingsExportFormat = "csv" | "json" | "markdown" | "counts";

/** A bounded sample; not a privacy or secret-detection scan. */
export interface FindingsExportReview {
  preview: string;
  sampled: number;
  shortened: boolean;
  paths: number;
  evidenceItems: number;
  suppressed: number;
}

const PREVIEW_FINDINGS = 3;
const PREVIEW_CHARACTERS = 4_000;

/**
 * Describe the complete filtered selection without rendering an unbounded
 * preview. The full export is generated only after user confirmation.
 */
export function findingsExportReview(
  findings: readonly Finding[],
  format: FindingsExportFormat,
): FindingsExportReview {
  const sample = findings.slice(0, PREVIEW_FINDINGS);
  const text =
    format === "counts"
      ? findingsCountsCsv(findings)
      : format === "csv"
        ? findingsCsv(sample)
        : format === "json"
          ? findingsJson(sample)
          : findingsMarkdown(sample);
  const clipped = text.length > PREVIEW_CHARACTERS;
  return {
    preview: clipped ? text.slice(0, PREVIEW_CHARACTERS) + "\n[Sample shortened]" : text,
    sampled: format === "counts" ? findings.length : sample.length,
    shortened: clipped || (format !== "counts" && sample.length !== findings.length),
    paths: new Set(findings.flatMap((finding) => finding.paths ?? [])).size,
    evidenceItems: findings.reduce((count, finding) => count + (finding.evidence?.length ?? 0), 0),
    suppressed: findings.filter(isSuppressed).length,
  };
}
