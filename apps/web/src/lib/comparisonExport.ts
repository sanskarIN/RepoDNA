import {
  CHANGE_LABELS,
  filterAnalysisChanges,
  type AnalysisChange,
  type AnalysisComparison,
  type ChangeGroup,
  type ChangeKind,
} from "./analysisComparison";

export type ComparisonSection = keyof AnalysisComparison;
export type ExportMode = "counts" | "details";
export type ExportFormat = "json" | "csv";
export const COMPARISON_SECTIONS: readonly [ComparisonSection, string][] = [
  ["files", "Files"],
  ["dependencies", "Dependencies"],
  ["languages", "Languages"],
  ["modules", "Architecture modules"],
  ["moduleEdges", "Module dependencies"],
  ["cycles", "Dependency cycles"],
  ["findings", "Findings"],
];
export const MAX_EXPORTED_CHANGES = 20_000;
export const MAX_EXPORT_BYTES = 16 * 1024 * 1024;
export const EXPORT_FORMAT_VERSION = "repodna-comparison-1";

export interface ExportSelection {
  /** Export all inventories or only the section and its active filters. */
  scope: "all" | "filtered";
  section?: ComparisonSection;
  kind?: ChangeKind | "all";
  query?: string;
  mode: ExportMode;
}
export interface ExportCounts {
  total: number;
  added: number;
  removed: number;
  modified: number;
  metadata: number;
  "possible-move": number;
  "only-current": number;
  "only-baseline": number;
  suppressed: number;
  unsuppressed: number;
}
export interface ExportCoverage {
  current: ChangeGroup["coverage"]["current"];
  baseline: ChangeGroup["coverage"]["baseline"];
  absenceComparable: boolean;
  ambiguousKeys: number;
  unverified: number;
}
export interface ExportDetail {
  /** The change identifier is deliberately omitted: labels and evidence are
   * enough for review, and keys may contain a raw internal identity. */
  kind: ChangeKind;
  label: string;
  before: string;
  after: string;
  evidence: string[];
}
export interface ExportGroup {
  section: ComparisonSection;
  coverage: ExportCoverage;
  /** Full section count, even when exporting a selected filter. */
  inventoryChanges: number;
  selected: ExportCounts;
  exportedDetails: number;
  omittedDetails: number;
  details?: ExportDetail[];
}
export interface ComparisonExport {
  format: typeof EXPORT_FORMAT_VERSION;
  scope: "all" | "filtered";
  mode: ExportMode;
  direction: "baseline-to-current";
  groups: ExportGroup[];
  selectedChanges: number;
  exportedDetails: number;
  omittedDetails: number;
  /** False means the export contains no free-form source-derived strings.
   * Counts still disclose project characteristics and need user review. */
  sourceDerivedText: boolean;
  notice: string;
}
export interface ExportLimits {
  maxRows?: number;
  maxBytes?: number;
}
export interface SerializedComparison {
  text: string;
  filename: string;
  mime: string;
  bytes: number;
  exportedDetails: number;
  omittedDetails: number;
}

function emptyCounts(): ExportCounts {
  return { total: 0, added: 0, removed: 0, modified: 0, metadata: 0, "possible-move": 0, "only-current": 0, "only-baseline": 0, suppressed: 0, unsuppressed: 0 };
}
function summarize(changes: readonly AnalysisChange[]): ExportCounts {
  const counts = emptyCounts();
  for (const change of changes) {
    counts.total += 1;
    counts[change.kind] += 1;
  }
  return counts;
}
function bounded(value: number | undefined, fallback: number, maximum: number): number {
  if (value === undefined) return fallback;
  if (!Number.isSafeInteger(value) || value < 0 || value > maximum) throw new RangeError("Invalid comparison export limit.");
  return value;
}
function toDetail(change: AnalysisChange): ExportDetail {
  return { kind: change.kind, label: change.label, before: change.before, after: change.after, evidence: [...change.details] };
}
function selectGroups(comparison: AnalysisComparison, selection: ExportSelection): { section: ComparisonSection; group: ChangeGroup; changes: readonly AnalysisChange[] }[] {
  if (selection.scope === "all") return COMPARISON_SECTIONS.map(([section]) => ({ section, group: comparison[section], changes: comparison[section].changes }));
  if (!selection.section || !COMPARISON_SECTIONS.some(([section]) => section === selection.section)) throw new Error("Select a valid comparison section before exporting a filter.");
  const group = comparison[selection.section];
  return [{ section: selection.section, group, changes: filterAnalysisChanges(group.changes, selection.query ?? "", selection.kind ?? "all") }];
}
function coverage(group: ChangeGroup): ExportCoverage {
  return {
    current: group.coverage.current,
    baseline: group.coverage.baseline,
    absenceComparable: group.coverage.absenceComparable,
    ambiguousKeys: group.ambiguousKeys,
    unverified: group.unverified,
  };
}
const NOTICE = "Observed differences between saved analyses, not a source-code diff, proof of Git ancestry, or a measure of contribution. Coverage and configuration differences may affect these counts.";

/** Build a deterministic report. Counts-only does not include IDs, paths,
 * names, search text, coverage free text, raw hashes or revision identifiers. */
export function buildComparisonExport(comparison: AnalysisComparison, selection: ExportSelection, maxRows = MAX_EXPORTED_CHANGES): ComparisonExport {
  let remaining = bounded(maxRows, MAX_EXPORTED_CHANGES, MAX_EXPORTED_CHANGES);
  const groups: ExportGroup[] = [];
  for (const { section, group, changes } of selectGroups(comparison, selection)) {
    const selected = summarize(changes);
    const limit = selection.mode === "details" ? Math.min(remaining, selected.total) : 0;
    const report: ExportGroup = {
      section,
      coverage: coverage(group),
      inventoryChanges: group.changes.length,
      selected,
      exportedDetails: limit,
      omittedDetails: selection.mode === "details" ? selected.total - limit : 0,
    };
    if (selection.mode === "details") {
      report.details = changes.slice(0, limit).map(toDetail);
      remaining -= limit;
    }
    groups.push(report);
  }
  return {
    format: EXPORT_FORMAT_VERSION,
    scope: selection.scope,
    mode: selection.mode,
    direction: "baseline-to-current",
    groups,
    selectedChanges: groups.reduce((total, group) => total + group.selected.total, 0),
    exportedDetails: groups.reduce((total, group) => total + group.exportedDetails, 0),
    omittedDetails: groups.reduce((total, group) => total + group.omittedDetails, 0),
    sourceDerivedText: selection.mode === "details",
    notice: NOTICE,
  };
}

function csvCell(value: string | number | boolean | null): string {
  if (value === null) return "";
  if (typeof value !== "string") return String(value);
  const safe = /^[\s\uFEFF]*[=+\-@]/.test(value) || /^[\t\r\n]/.test(value) ? "'" + value : value;
  return `"${safe.replaceAll('"', '""')}"`;
}
const CSV_HEADER = [
  "record_type", "section", "scope", "mode", "direction", "current_coverage", "baseline_coverage",
  "absence_comparable", "ambiguous_keys", "unverified_matches", "inventory_changes", "selected_changes",
  "exported_details", "omitted_details", "kind", "count", "label", "baseline", "current", "evidence",
];
function csvGroupPrefix(report: ComparisonExport, group: ExportGroup, record: "count" | "detail"): (string | boolean | number)[] {
  return [
    record, group.section, report.scope, report.mode, report.direction,
    group.coverage.current, group.coverage.baseline, group.coverage.absenceComparable,
    group.coverage.ambiguousKeys, group.coverage.unverified, group.inventoryChanges,
    group.selected.total, group.exportedDetails, group.omittedDetails,
  ];
}
function csvRows(report: ComparisonExport): string[] {
  const rows = [CSV_HEADER.join(",")];
  for (const group of report.groups) {
    const countPrefix = csvGroupPrefix(report, group, "count");
    // Retain even zero-valued count rows: downstream readers can distinguish a
    // measured empty selection from a missing category in the export.
    for (const kind of Object.keys(CHANGE_LABELS) as ChangeKind[]) {
      rows.push([...countPrefix, kind, group.selected[kind], null, null, null, null].map(csvCell).join(","));
    }
    const detailPrefix = csvGroupPrefix(report, group, "detail");
    for (const item of group.details ?? []) {
      rows.push([...detailPrefix, item.kind, null, item.label, item.before, item.after, item.evidence.join("; ")].map(csvCell).join(","));
    }
  }
  return rows;
}

/** Serialize without a giant optimistic detail allocation. First estimate
 * detail byte costs individually, then create a report using the retained
 * prefix. Exact final bytes are verified; summaries are never silently lost. */
export function serializeComparisonExport(comparison: AnalysisComparison, selection: ExportSelection, format: ExportFormat, limits: ExportLimits = {}): SerializedComparison {
  const maxRows = bounded(limits.maxRows, MAX_EXPORTED_CHANGES, MAX_EXPORTED_CHANGES);
  const maxBytes = bounded(limits.maxBytes, MAX_EXPORT_BYTES, MAX_EXPORT_BYTES);
  const encoder = new TextEncoder();
  const render = (report: ComparisonExport) => format === "json"
    ? JSON.stringify(report, null, 2) + "\n"
    : csvRows(report).join("\r\n") + "\r\n";
  const summary = buildComparisonExport(comparison, selection, 0);
  const summaryBytes = encoder.encode(render(summary)).byteLength;
  if (summaryBytes > maxBytes) throw new Error("The comparison summary exceeds the export size limit.");
  let retained = 0;
  if (selection.mode === "details") {
    let budget = maxBytes - summaryBytes;
    const groups = selectGroups(comparison, selection);
    outer: for (const { section, changes } of groups) {
      const group = summary.groups.find((item) => item.section === section)!;
      for (const change of changes) {
        if (retained >= maxRows) break outer;
        const item = toDetail(change);
        // Conservative overhead includes JSON indentation, separators, or a
        // repeated CSV coverage prefix, as well as count-digit adjustments.
        const cost = format === "json"
          ? encoder.encode(JSON.stringify(item, null, 2)).byteLength + 128 + item.evidence.length * 12
          : encoder.encode([...csvGroupPrefix(summary, group, "detail"), item.kind, null, item.label, item.before, item.after, item.evidence.join("; ")].map(csvCell).join(",")).byteLength + 64;
        if (cost > budget) break outer;
        budget -= cost;
        retained += 1;
      }
    }
  }
  let report = buildComparisonExport(comparison, selection, retained);
  let text = render(report);
  let bytes = encoder.encode(text).byteLength;
  // The conservative estimate should already fit. Verify actual bytes and
  // reduce only the detail prefix if unusual quoting/indentation exceeds it.
  while (bytes > maxBytes && retained > 0) {
    retained = Math.floor(retained / 2);
    report = buildComparisonExport(comparison, selection, retained);
    text = render(report);
    bytes = encoder.encode(text).byteLength;
  }
  if (bytes > maxBytes) throw new Error("The comparison summary exceeds the export size limit.");
  return {
    text,
    filename: `repodna-comparison-${selection.mode}.${format}`,
    mime: format === "json" ? "application/json" : "text/csv;charset=utf-8",
    bytes,
    exportedDetails: report.exportedDetails,
    omittedDetails: report.omittedDetails,
  };
}
