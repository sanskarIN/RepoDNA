import type { ScanSummary } from "./backend";

export type HistoryMetric = "files" | "codeLines" | "commits" | "contributors" | "activeFindings" | "critical" | "warning" | "attention" | "info" | "suppressed";
export interface MetricDefinition {
  id: HistoryMetric;
  label: string;
  description: string;
  group: "structure" | "history" | "findings";
}
export const HISTORY_METRICS: readonly MetricDefinition[] = [
  { id: "files", label: "Discovered files", description: "Reported file count after the scan's ignore rules.", group: "structure" },
  { id: "codeLines", label: "Code lines", description: "Code lines in the saved scan, not Git additions or deletions.", group: "structure" },
  { id: "commits", label: "Analyzed commits", description: "Commits included by the scan's history limits, not necessarily the entire repository history.", group: "history" },
  { id: "contributors", label: "Contributors", description: "Contributors in the analyzed Git history.", group: "history" },
  { id: "activeFindings", label: "Active findings", description: "Sum of critical, warning, attention and informational findings, excluding suppressed findings.", group: "findings" },
  { id: "critical", label: "Critical findings", description: "Critical findings reported by the saved scan, not confirmed vulnerabilities.", group: "findings" },
  { id: "warning", label: "Warnings", description: "Warning-level findings reported by the saved scan.", group: "findings" },
  { id: "attention", label: "Attention findings", description: "Attention-level findings reported by the saved scan.", group: "findings" },
  { id: "info", label: "Informational findings", description: "Informational findings reported by the saved scan.", group: "findings" },
  { id: "suppressed", label: "Suppressed findings", description: "Findings that matched configured suppression rules.", group: "findings" },
];
export const MAX_HISTORY_TREND_SCANS = 10_000;
export const MAX_CHART_POINTS = 120;

export interface TrendFilter {
  profile?: string;
  toolVersion?: string;
  /** UTC calendar-day boundaries, inclusive. */
  from?: string;
  through?: string;
  revision?: string;
}
export type GapReason = "unavailable" | "different-profile" | "different-version" | "same-time" | "unknown-profile";
export interface TrendPoint {
  id: string;
  timestamp: number;
  generatedAt: string;
  profile: string;
  toolVersion: string;
  revision: string | null;
  value: number | null;
  /** Difference from the preceding visible scan only when basic scope matches. */
  delta: number | null;
  percentChange: number | null;
  gap: GapReason | null;
  sameRevision: boolean;
  sameFingerprint: boolean;
}
export interface TrendSummary {
  metric: HistoryMetric;
  points: TrendPoint[];
  knownValues: number;
  missingValues: number;
  invalidScans: number;
  duplicateScans: number;
  filteredOut: number;
  truncated: boolean;
  partialHistory: boolean;
  min: number | null;
  max: number | null;
  first: TrendPoint | null;
  last: TrendPoint | null;
  /** No total change is displayed across incompatible or missing intervals. */
  netChange: number | null;
  netPercent: number | null;
  comparableIntervals: number;
  interruptedIntervals: number;
  distinctProfiles: string[];
  distinctVersions: string[];
}

const STANDARD_PROFILES = new Set(["quick", "standard", "deep", "history-only", "architecture-only", "dependencies-only", "security-only"]);
// Quick and focused non-history profiles explicitly omit Git in the native
// pipeline. A zero in such a summary is not evidence of zero Git commits.
const NO_GIT_PROFILES = new Set(["quick", "architecture-only", "dependencies-only", "security-only"]);

function count(value: number): number | null {
  return Number.isSafeInteger(value) && value >= 0 ? value : null;
}
function safeTotal(values: readonly number[]): number | null {
  let result = 0;
  for (const value of values) {
    if (count(value) === null) return null;
    result += value;
    if (count(result) === null) return null;
  }
  return result;
}

export function metricDefinition(metric: HistoryMetric): MetricDefinition {
  const definition = HISTORY_METRICS.find((item) => item.id === metric);
  if (!definition) throw new Error("Unknown saved-history metric.");
  return definition;
}

export function summaryValue(scan: ScanSummary, metric: HistoryMetric): number | null {
  if (metricDefinition(metric).group === "history" && NO_GIT_PROFILES.has(scan.profile)) return null;
  switch (metric) {
    case "files": return count(scan.files);
    case "codeLines": return count(scan.codeLines);
    case "commits": return count(scan.commits);
    case "contributors": return count(scan.contributors);
    case "activeFindings": return safeTotal([scan.findings.critical, scan.findings.warning, scan.findings.attention, scan.findings.info]);
    default: return count(scan.findings[metric]);
  }
}

/** Date.parse accepts normalized impossible dates such as February 30. Reject
 * those before interpreting the filter as an inclusive UTC day boundary. */
export function utcDay(value: string): number | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return null;
  const time = Date.parse(value + "T00:00:00.000Z");
  return Number.isFinite(time) && new Date(time).toISOString().slice(0, 10) === value ? time : null;
}
export interface TrendBounds {
  from: number | null;
  before: number | null;
}
export function trendBounds(filter: TrendFilter): TrendBounds {
  const from = filter.from ? utcDay(filter.from) : null;
  const through = filter.through ? utcDay(filter.through) : null;
  if (filter.from && from === null || filter.through && through === null) throw new Error("Enter valid UTC calendar dates.");
  if (from !== null && through !== null && from > through) throw new Error("The start date must not be after the end date.");
  return { from, before: through === null ? null : through + 86_400_000 };
}

function timestamp(value: string): number | null {
  // Stored summaries are RFC 3339 timestamps. Require an explicit zone and a
  // real calendar date; do not let a host's timezone affect ordering.
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/i.test(value) || utcDay(value.slice(0, 10)) === null) return null;
  const time = Date.parse(value);
  return Number.isFinite(time) ? time : null;
}
function compare(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}
function percentage(delta: number, previous: number): number | null {
  if (previous === 0) return delta === 0 ? 0 : null;
  const result = delta / previous * 100;
  return Number.isFinite(result) ? result : null;
}
function gapBetween(a: TrendPoint, b: TrendPoint): GapReason | null {
  if (a.value === null || b.value === null) return "unavailable";
  if (!STANDARD_PROFILES.has(a.profile) || !STANDARD_PROFILES.has(b.profile) || !a.toolVersion || !b.toolVersion) return "unknown-profile";
  if (a.profile !== b.profile) return "different-profile";
  if (a.toolVersion !== b.toolVersion) return "different-version";
  if (a.timestamp === b.timestamp) return "same-time";
  return null;
}
export function gapLabel(reason: GapReason): string {
  switch (reason) {
    case "unavailable": return "A recorded metric is unavailable.";
    case "different-profile": return "The analysis profile changed.";
    case "different-version": return "The analyzer version changed.";
    case "same-time": return "The scans have the same recorded time; their order is not established.";
    case "unknown-profile": return "The analysis profile or tool version is unknown.";
  }
}

interface Candidate {
  scan: ScanSummary;
  time: number;
}
export function trendOptions(scans: readonly ScanSummary[]): { profiles: string[]; toolVersions: string[] } {
  const profiles = new Set<string>();
  const versions = new Set<string>();
  for (const scan of scans.slice(0, MAX_HISTORY_TREND_SCANS)) {
    if (scan.profile) profiles.add(scan.profile);
    if (scan.toolVersion) versions.add(scan.toolVersion);
  }
  return { profiles: [...profiles].sort(compare), toolVersions: [...versions].sort(compare) };
}

/** Aggregate bounded saved summaries, not source-code churn. The server
 * summary does not include complete configuration, branch, or analyzer status,
 * so even same-profile/version deltas are observations only. */
export function buildHistoryTrend(scans: readonly ScanSummary[], metric: HistoryMetric, filter: TrendFilter = {}, partialHistory = false): TrendSummary {
  const bounds = trendBounds(filter);
  const ids = new Set<string>();
  const candidates: Candidate[] = [];
  let invalidScans = 0;
  let duplicateScans = 0;
  let filteredOut = 0;
  const revision = filter.revision?.trim().toLowerCase();
  for (const scan of scans.slice(0, MAX_HISTORY_TREND_SCANS)) {
    const time = timestamp(scan.generatedAt);
    if (!scan.id || time === null) { invalidScans += 1; continue; }
    if (ids.has(scan.id)) { duplicateScans += 1; continue; }
    ids.add(scan.id);
    if ((filter.profile && scan.profile !== filter.profile) ||
        (filter.toolVersion && scan.toolVersion !== filter.toolVersion) ||
        (bounds.from !== null && time < bounds.from) ||
        (bounds.before !== null && time >= bounds.before) ||
        (revision && !scan.revision?.toLowerCase().startsWith(revision))) {
      filteredOut += 1;
      continue;
    }
    candidates.push({ scan, time });
  }
  candidates.sort((a, b) => a.time - b.time || compare(a.scan.id, b.scan.id));
  const points: TrendPoint[] = [];
  let min: number | null = null;
  let max: number | null = null;
  let knownValues = 0;
  let comparableIntervals = 0;
  let interruptedIntervals = 0;
  const profiles = new Set<string>();
  const versions = new Set<string>();
  let previousScan: ScanSummary | null = null;
  for (const { scan, time } of candidates) {
    const value = summaryValue(scan, metric);
    const point: TrendPoint = {
      id: scan.id,
      timestamp: time,
      generatedAt: scan.generatedAt,
      profile: scan.profile,
      toolVersion: scan.toolVersion,
      revision: scan.revision || null,
      value,
      delta: null,
      percentChange: null,
      gap: null,
      sameRevision: false,
      sameFingerprint: false,
    };
    const previous = points.at(-1);
    if (previous) {
      point.gap = gapBetween(previous, point);
      point.sameRevision = Boolean(previous.revision && point.revision && previous.revision.toLowerCase() === point.revision.toLowerCase());
      point.sameFingerprint = Boolean(previousScan?.dnaHash && scan.dnaHash && previousScan.dnaHash === scan.dnaHash);
      if (point.gap === null && previous.value !== null && value !== null) {
        point.delta = value - previous.value;
        point.percentChange = percentage(point.delta, previous.value);
        comparableIntervals += 1;
      } else interruptedIntervals += 1;
    }
    if (value !== null) {
      min = min === null ? value : Math.min(min, value);
      max = max === null ? value : Math.max(max, value);
      knownValues += 1;
    }
    profiles.add(scan.profile);
    versions.add(scan.toolVersion);
    points.push(point);
    previousScan = scan;
  }
  const first = points[0] ?? null;
  const last = points.at(-1) ?? null;
  const netChange = first?.value != null && last?.value != null && comparableIntervals > 0 && interruptedIntervals === 0 ? last.value - first.value : null;
  return {
    metric, points, knownValues, missingValues: points.length - knownValues,
    invalidScans, duplicateScans, filteredOut, truncated: scans.length > MAX_HISTORY_TREND_SCANS,
    partialHistory,
    min, max, first, last, netChange,
    netPercent: netChange !== null && first?.value != null ? percentage(netChange, first.value) : null,
    comparableIntervals, interruptedIntervals,
    distinctProfiles: [...profiles].sort(compare), distinctVersions: [...versions].sort(compare),
  };
}

export interface TrendChartPoint {
  point: TrendPoint;
  index: number;
  x: number;
  y: number | null;
  /** Lines never bridge missing or incompatible observations. */
  connected: boolean;
}
export interface TrendChart {
  points: TrendChartPoint[];
  sampled: boolean;
  minimum: number;
  maximum: number;
  firstTime: number | null;
  lastTime: number | null;
}

/** Bounded visual sampling preserves endpoints and local extrema. The table
 * and CSV retain all points. Skipped intervals cannot be joined in the chart. */
export function trendChart(summary: TrendSummary, limit = MAX_CHART_POINTS): TrendChart {
  if (!Number.isInteger(limit) || limit < 4 || limit > 2_000) throw new RangeError("Invalid saved-history chart point limit.");
  const points = summary.points;
  const indices = new Set<number>();
  const first = points[0];
  const last = points.at(-1);
  if (points.length <= limit) {
    for (let i = 0; i < points.length; i += 1) indices.add(i);
  } else {
    indices.add(0);
    indices.add(points.length - 1);
    // Each bucket contributes min and max observed values. If no value is
    // available, retain its first missing point rather than inventing one.
    const buckets = Math.floor((limit - 2) / 2);
    for (let bucket = 0; bucket < buckets; bucket += 1) {
      const start = 1 + Math.floor(bucket * (points.length - 2) / buckets);
      const end = 1 + Math.floor((bucket + 1) * (points.length - 2) / buckets);
      let min: number | null = null;
      let max: number | null = null;
      for (let i = start; i < end; i += 1) {
        const value = points[i]?.value;
        if (value == null) continue;
        if (min === null || value < points[min]!.value!) min = i;
        if (max === null || value > points[max]!.value!) max = i;
      }
      indices.add(min ?? start);
      if (max !== null) indices.add(max);
    }
  }
  const minimum = 0;
  const maximum = Math.max(1, summary.max ?? 0);
  const duration = first && last ? last.timestamp - first.timestamp : 0;
  const result: TrendChartPoint[] = [];
  // Build a prefix count of interrupted links once; this makes the chart's
  // no-gap-crossing guarantee independent of how aggressively it samples.
  const gaps: number[] = [0];
  for (const [i, point] of points.entries()) gaps.push(gaps[i]! + (point.gap !== null || point.value === null ? 1 : 0));
  for (const index of [...indices].sort((a, b) => a - b)) {
    const point = points[index];
    if (!point) continue;
    const previous = result.at(-1);
    const connected = Boolean(previous && previous.y !== null && point.value !== null && gaps[index + 1] === gaps[previous.index + 1]);
    result.push({
      point, index,
      x: duration > 0 && first ? (point.timestamp - first.timestamp) / duration : 0.5,
      y: point.value === null ? null : 1 - (point.value - minimum) / (maximum - minimum),
      connected,
    });
  }
  return { points: result, sampled: result.length < points.length, minimum, maximum, firstTime: first?.timestamp ?? null, lastTime: last?.timestamp ?? null };
}

/** Escape untrusted spreadsheet formula prefixes before CSV quoting. Preserve
 * actual numeric values as numbers, including negative differences. */
function csvCell(value: string | number | null): string {
  if (value === null) return "";
  if (typeof value === "number") return Number.isFinite(value) ? String(value) : "";
  const safe = /^[\s\uFEFF]*[=+\-@]/.test(value) || /^[\t\r\n]/.test(value) ? "'" + value : value;
  return `"${safe.replaceAll('"', '""')}"`;
}
export function historyTrendCsv(summary: TrendSummary): string {
  const columns = ["scan_id", "generated_at_utc", "profile", "tool_version", "revision", "metric", "recorded_value", "observed_delta", "observed_percent_change", "gap_reason", "same_revision_as_previous", "same_fingerprint_as_previous", "partial_history"];
  const rows = [columns.join(",")];
  for (const point of summary.points) {
    rows.push([
      point.id, new Date(point.timestamp).toISOString(), point.profile, point.toolVersion,
      point.revision, summary.metric, point.value, point.delta, point.percentChange,
      point.gap, point.sameRevision ? "true" : "false", point.sameFingerprint ? "true" : "false",
      summary.partialHistory || summary.truncated ? "true" : "false",
    ].map(csvCell).join(","));
  }
  return rows.join("\r\n") + "\r\n";
}

export type TrendSort = "newest" | "oldest" | "largest" | "smallest";
export interface TrendPage {
  rows: TrendPoint[];
  page: number;
  pages: number;
  total: number;
}
export function trendPage(points: readonly TrendPoint[], requested: number, size: number, sort: TrendSort): TrendPage {
  if (!Number.isInteger(size) || size < 1 || size > 500) throw new RangeError("Invalid saved-history table page size.");
  const ordered = [...points];
  if (sort === "newest") ordered.reverse();
  else if (sort === "largest" || sort === "smallest") ordered.sort((a, b) => {
    if (a.value === null) return b.value === null ? b.timestamp - a.timestamp || compare(a.id, b.id) : 1;
    if (b.value === null) return -1;
    return (sort === "largest" ? b.value - a.value : a.value - b.value) || b.timestamp - a.timestamp || compare(a.id, b.id);
  });
  const pages = Math.max(1, Math.ceil(ordered.length / size));
  const page = Math.max(0, Math.min(pages - 1, Number.isFinite(requested) ? Math.trunc(requested) : 0));
  return { rows: ordered.slice(page * size, (page + 1) * size), page, pages, total: ordered.length };
}
