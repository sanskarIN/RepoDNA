import {
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

/** Available formats for a local, user-reviewed findings export. */
export type FindingsExportFormat = "csv" | "json" | "markdown";

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
    format === "csv"
      ? findingsCsv(sample)
      : format === "json"
        ? findingsJson(sample)
        : findingsMarkdown(sample);
  const clipped = text.length > PREVIEW_CHARACTERS;
  return {
    preview: clipped ? text.slice(0, PREVIEW_CHARACTERS) + "\n[Sample shortened]" : text,
    sampled: sample.length,
    shortened: clipped || sample.length !== findings.length,
    paths: new Set(findings.flatMap((finding) => finding.paths ?? [])).size,
    evidenceItems: findings.reduce((count, finding) => count + (finding.evidence?.length ?? 0), 0),
    suppressed: findings.filter(isSuppressed).length,
  };
}
