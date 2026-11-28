import { describe, expect, it } from "vitest";
import type { ScanSummary } from "./backend";
import {
  buildHistoryTrend, gapLabel, HISTORY_METRICS, historyTrendCsv, metricDefinition,
  summaryValue, trendBounds, trendChart, trendOptions, trendPage, utcDay,
  MAX_HISTORY_TREND_SCANS,
} from "./historyTrend";

function scan(id: string, day: number, value: number, options: Partial<ScanSummary> = {}): ScanSummary {
  return {
    id,
    generatedAt: new Date(Date.UTC(2026, 0, 1 + day)).toISOString(),
    profile: "standard",
    toolVersion: "1.5.0",
    revision: `revision-${id}`,
    dnaHash: `hash-${id}`,
    files: value * 2,
    codeLines: value,
    commits: 10,
    contributors: 3,
    findings: { critical: 1, warning: 2, attention: 3, info: 4, suppressed: 5 },
    ...options,
  };
}
function error(operation: () => unknown): unknown {
  try { operation(); return null; } catch (reason) { return reason; }
}

describe("saved history metric selection", () => {
  it("offers unique definitions for all recorded metrics", () => {
    expect(HISTORY_METRICS.length).toBe(10);
    expect(new Set(HISTORY_METRICS.map((metric) => metric.id)).size).toBe(10);
    for (const metric of HISTORY_METRICS) expect(metricDefinition(metric.id)).toBe(metric);
  });
  it("reports each recorded structure and Git metric", () => {
    const record = scan("a", 0, 15);
    expect(summaryValue(record, "files")).toBe(30);
    expect(summaryValue(record, "codeLines")).toBe(15);
    expect(summaryValue(record, "commits")).toBe(10);
    expect(summaryValue(record, "contributors")).toBe(3);
  });
  it("does not treat omitted Git stages as zero history", () => {
    for (const profile of ["quick", "architecture-only", "dependencies-only", "security-only"]) {
      const record = scan("a", 0, 10, { profile, commits: 0, contributors: 0 });
      expect(summaryValue(record, "commits")).toBeNull();
      expect(summaryValue(record, "contributors")).toBeNull();
      expect(summaryValue(record, "codeLines")).toBe(10);
    }
  });
  it("retains a recorded zero for a history-capable profile", () => {
    expect(summaryValue(scan("a", 0, 10, { commits: 0 }), "commits")).toBe(0);
  });
  it("adds active findings without including suppressed findings", () => {
    const record = scan("a", 0, 10);
    expect(summaryValue(record, "activeFindings")).toBe(10);
    expect(summaryValue(record, "critical")).toBe(1);
    expect(summaryValue(record, "warning")).toBe(2);
    expect(summaryValue(record, "attention")).toBe(3);
    expect(summaryValue(record, "info")).toBe(4);
    expect(summaryValue(record, "suppressed")).toBe(5);
  });
  it("rejects unsafe, negative and non-integral measurements", () => {
    for (const value of [-1, Infinity, NaN, 0.5, Number.MAX_SAFE_INTEGER + 1]) {
      expect(summaryValue(scan("a", 0, value), "codeLines")).toBeNull();
    }
  });
  it("does not round a finding total above safe integer precision", () => {
    const record = scan("a", 0, 1);
    record.findings.critical = Number.MAX_SAFE_INTEGER;
    expect(summaryValue(record, "activeFindings")).toBeNull();
  });
});

describe("saved history chronology and filtering", () => {
  it("orders independent input pages by timestamp then exact ID", () => {
    const rows = [scan("c", 3, 3), scan("b", 1, 2), scan("a", 1, 1)];
    const result = buildHistoryTrend(rows, "codeLines");
    expect(result.points.map((point) => point.id)).toEqual(["a", "b", "c"]);
    expect(result.points[1]?.gap).toBe("same-time");
    expect(rows.map((row) => row.id)).toEqual(["c", "b", "a"]);
  });
  it("interprets offset timestamps as the same UTC instant", () => {
    const result = buildHistoryTrend([
      scan("a", 0, 1, { generatedAt: "2026-01-01T05:30:00+05:30" }),
      scan("b", 0, 2, { generatedAt: "2026-01-01T00:00:00Z" }),
    ], "codeLines");
    expect(result.points[0]?.timestamp).toBe(result.points[1]?.timestamp);
    expect(result.points[1]?.gap).toBe("same-time");
  });
  it("uses strict real UTC calendar-day boundaries", () => {
    expect(utcDay("2024-02-29")).toBe(Date.UTC(2024, 1, 29));
    for (const day of ["2026-02-29", "2026-04-31", "2026-13-01", "2026-01-1", "today", "2026-01-01T00:00:00Z"]) expect(utcDay(day)).toBeNull();
  });
  it("validates reversed date filters before scanning history", () => {
    expect(error(() => trendBounds({ from: "2026-02-01", through: "2026-01-01" }))).toBeInstanceOf(Error);
    expect(error(() => trendBounds({ from: "tomorrow" }))).toBeInstanceOf(Error);
  });
  it("includes the entire end date in UTC", () => {
    const result = buildHistoryTrend([
      scan("before", 0, 1),
      scan("first", 1, 2),
      scan("last", 1, 3, { generatedAt: "2026-01-02T23:59:59.999Z" }),
      scan("after", 2, 4),
    ], "codeLines", { from: "2026-01-02", through: "2026-01-02" });
    expect(result.points.map((point) => point.id)).toEqual(["first", "last"]);
    expect(result.filteredOut).toBe(2);
  });
  it("omits invalid timestamps and missing identities without inventing dates", () => {
    const result = buildHistoryTrend([
      scan("a", 0, 1, { generatedAt: "bad" }),
      scan("b", 1, 2, { generatedAt: "2026-02-30T00:00:00Z" }),
      scan("c", 1, 2, { generatedAt: "2026-01-02T00:00:00" }),
      scan("", 2, 3), scan("valid", 3, 4),
    ], "codeLines");
    expect(result.invalidScans).toBe(4);
    expect(result.points.map((point) => point.id)).toEqual(["valid"]);
  });
  it("deduplicates exact scan identities, not equal timestamps or revisions", () => {
    const result = buildHistoryTrend([scan("a", 0, 1), scan("a", 1, 5), scan("b", 0, 1)], "codeLines");
    expect(result.duplicateScans).toBe(1);
    expect(result.points.map((point) => point.id)).toEqual(["a", "b"]);
    expect(result.points[0]?.value).toBe(1);
  });
  it("filters exact profile and analyzer version separately", () => {
    const rows = [scan("a", 0, 1), scan("b", 1, 2, { profile: "deep" }), scan("c", 2, 3, { toolVersion: "1.6.0" })];
    expect(buildHistoryTrend(rows, "files", { profile: "standard", toolVersion: "1.5.0" }).points.map((point) => point.id)).toEqual(["a"]);
    expect(trendOptions(rows)).toEqual({ profiles: ["deep", "standard"], toolVersions: ["1.5.0", "1.6.0"] });
  });
  it("matches a revision prefix case-insensitively and excludes unknown revisions", () => {
    const rows = [scan("a", 0, 1, { revision: "ABCDEF" }), scan("b", 1, 2, { revision: "abxyz" }), scan("c", 2, 3, { revision: null })];
    expect(buildHistoryTrend(rows, "files", { revision: " abc " }).points.map((point) => point.id)).toEqual(["a"]);
  });
  it("preserves a partial-history flag and bounds work", () => {
    const rows = Array.from({ length: MAX_HISTORY_TREND_SCANS + 1 }, (_, index) => scan(String(index), index, index));
    const result = buildHistoryTrend(rows, "codeLines", {}, true);
    expect(result.partialHistory).toBe(true);
    expect(result.truncated).toBe(true);
    expect(result.points.length).toBe(MAX_HISTORY_TREND_SCANS);
  });
});

describe("saved history interval observations", () => {
  it("computes signed absolute and percentage changes within a matching scope", () => {
    const result = buildHistoryTrend([scan("a", 0, 100), scan("b", 1, 150), scan("c", 2, 120)], "codeLines");
    expect(result.points.map((point) => point.delta)).toEqual([null, 50, -30]);
    expect(result.points.map((point) => point.percentChange)).toEqual([null, 50, -20]);
    expect(result.netChange).toBe(20);
    expect(result.netPercent).toBe(20);
    expect(result.min).toBe(100);
    expect(result.max).toBe(150);
    expect(result.comparableIntervals).toBe(2);
  });
  it("does not invent a relative increase from a zero baseline", () => {
    const result = buildHistoryTrend([scan("a", 0, 0), scan("b", 1, 10)], "codeLines");
    expect(result.points[1]?.delta).toBe(10);
    expect(result.points[1]?.percentChange).toBeNull();
    expect(result.netPercent).toBeNull();
  });
  it("handles unchanged zero measurements", () => {
    const result = buildHistoryTrend([scan("a", 0, 0), scan("b", 1, 0)], "codeLines");
    expect(result.netChange).toBe(0);
    expect(result.netPercent).toBe(0);
  });
  it("interrupts intervals on profile changes", () => {
    const result = buildHistoryTrend([scan("a", 0, 10), scan("b", 1, 20, { profile: "deep" }), scan("c", 2, 30, { profile: "deep" })], "files");
    expect(result.points[1]?.gap).toBe("different-profile");
    expect(result.points[1]?.delta).toBeNull();
    expect(result.points[2]?.delta).toBe(20);
    expect(result.netChange).toBeNull();
  });
  it("interrupts intervals on analyzer version changes", () => {
    const result = buildHistoryTrend([scan("a", 0, 10), scan("b", 1, 20, { toolVersion: "1.6.0" })], "files");
    expect(result.points[1]?.gap).toBe("different-version");
    expect(result.netChange).toBeNull();
  });
  it("interrupts intervals with unknown analysis scope", () => {
    const result = buildHistoryTrend([scan("a", 0, 10, { profile: "custom" }), scan("b", 1, 20, { profile: "custom" })], "files");
    expect(result.points[1]?.gap).toBe("unknown-profile");
  });
  it("does not bridge unavailable observations", () => {
    const result = buildHistoryTrend([scan("a", 0, 10), scan("b", 1, -1), scan("c", 2, 30)], "codeLines");
    expect(result.points.map((point) => point.gap)).toEqual([null, "unavailable", "unavailable"]);
    expect(result.knownValues).toBe(2);
    expect(result.missingValues).toBe(1);
    expect(result.netChange).toBeNull();
  });
  it("marks same revisions and fingerprints without assuming identical measurements", () => {
    const options = { revision: "abcdef", dnaHash: "same" };
    const result = buildHistoryTrend([scan("a", 0, 10, options), scan("b", 1, 15, { ...options, revision: "ABCDEF" })], "codeLines");
    expect(result.points[1]?.sameRevision).toBe(true);
    expect(result.points[1]?.sameFingerprint).toBe(true);
    expect(result.points[1]?.delta).toBe(5);
  });
  it("does not treat absent identities as equal fingerprints or revisions", () => {
    const options = { revision: null, dnaHash: "" };
    const result = buildHistoryTrend([scan("a", 0, 10, options), scan("b", 1, 10, options)], "files");
    expect(result.points[1]?.sameRevision).toBe(false);
    expect(result.points[1]?.sameFingerprint).toBe(false);
  });
  it("returns explicit empty and single-snapshot results", () => {
    const empty = buildHistoryTrend([], "files");
    expect(empty.first).toBeNull();
    expect(empty.last).toBeNull();
    expect(empty.min).toBeNull();
    expect(empty.netChange).toBeNull();
    const single = buildHistoryTrend([scan("a", 0, 1)], "files");
    expect(single.netChange).toBeNull();
    expect(single.comparableIntervals).toBe(0);
  });
  it("provides meaningful gap explanations", () => {
    for (const reason of ["unavailable", "different-profile", "different-version", "same-time", "unknown-profile"] as const) expect(gapLabel(reason).length > 10).toBe(true);
  });
});

describe("saved history chart, table and export", () => {
  it("positions irregularly spaced timestamps on a real time axis", () => {
    const chart = trendChart(buildHistoryTrend([scan("a", 0, 0), scan("b", 1, 5), scan("c", 10, 10)], "codeLines"));
    expect(chart.points.map((point) => point.x)).toEqual([0, 0.1, 1]);
    expect(chart.points.map((point) => point.y)).toEqual([1, 0.5, 0]);
    expect(chart.points.map((point) => point.connected)).toEqual([false, true, true]);
  });
  it("handles a flat zero chart and equal timestamps without non-finite coordinates", () => {
    const chart = trendChart(buildHistoryTrend([scan("a", 0, 0), scan("b", 0, 0)], "codeLines"));
    expect(chart.points.map((point) => point.x)).toEqual([0.5, 0.5]);
    expect(chart.points.map((point) => point.y)).toEqual([1, 1]);
    expect(chart.points[1]?.connected).toBe(false);
  });
  it("preserves a missing point as a gap rather than a zero", () => {
    const chart = trendChart(buildHistoryTrend([scan("a", 0, 1), scan("b", 1, -1), scan("c", 2, 2)], "codeLines"));
    expect(chart.points[1]?.y).toBeNull();
    expect(chart.points[2]?.connected).toBe(false);
  });
  it("bounds sampled points while retaining endpoints and local extremes", () => {
    const rows = Array.from({ length: 200 }, (_, index) => scan(String(index), index, index === 80 ? 1000 : index === 85 ? 0 : 50));
    const chart = trendChart(buildHistoryTrend(rows, "codeLines"), 20);
    expect(chart.sampled).toBe(true);
    expect(chart.points.length <= 20).toBe(true);
    expect(chart.points[0]?.index).toBe(0);
    expect(chart.points.at(-1)?.index).toBe(199);
    expect(chart.points.some((point) => point.point.value === 1000)).toBe(true);
    expect(chart.points.some((point) => point.point.value === 0)).toBe(true);
  });
  it("never connects across a sampled-out missing or incompatible interval", () => {
    const rows = Array.from({ length: 200 }, (_, index) => scan(String(index), index, index === 100 ? -1 : index, index === 125 ? { profile: "deep" } : {}));
    const chart = trendChart(buildHistoryTrend(rows, "codeLines"), 12);
    for (const [index, point] of chart.points.entries()) {
      const previous = chart.points[index - 1];
      if (previous && [100, 125, 126].some((gap) => previous.index < gap && point.index >= gap)) expect(point.connected).toBe(false);
    }
  });
  it("rejects invalid chart point limits", () => {
    expect(error(() => trendChart(buildHistoryTrend([], "codeLines"), 0))).toBeInstanceOf(RangeError);
  });
  it("sorts paged records without changing underlying chronological deltas", () => {
    const result = buildHistoryTrend([scan("a", 0, 3), scan("b", 1, 1), scan("c", 2, 4)], "codeLines");
    expect(trendPage(result.points, 0, 2, "newest").rows.map((point) => point.id)).toEqual(["c", "b"]);
    expect(trendPage(result.points, 0, 2, "oldest").rows.map((point) => point.id)).toEqual(["a", "b"]);
    expect(trendPage(result.points, 0, 2, "largest").rows.map((point) => point.id)).toEqual(["c", "a"]);
    expect(trendPage(result.points, 0, 2, "smallest").rows.map((point) => point.id)).toEqual(["b", "a"]);
    expect(result.points[1]?.delta).toBe(-2);
  });
  it("puts unavailable values last for both numeric sorts", () => {
    const result = buildHistoryTrend([scan("a", 0, -1), scan("b", 1, 2), scan("c", 2, 1)], "codeLines");
    for (const sort of ["largest", "smallest"] as const) expect(trendPage(result.points, 0, 10, sort).rows.at(-1)?.id).toBe("a");
  });
  it("clamps a page after filters reduce the result set", () => {
    const result = buildHistoryTrend([scan("a", 0, 1), scan("b", 1, 2)], "files");
    expect(trendPage(result.points, 99, 1, "newest").page).toBe(1);
    expect(trendPage([], 99, 1, "newest").page).toBe(0);
    expect(trendPage(result.points, -2, 1, "newest").page).toBe(0);
    expect(error(() => trendPage(result.points, 0, 0, "newest"))).toBeInstanceOf(RangeError);
  });
  it("exports a UTC timestamp, measurement and signed difference in chronological order", () => {
    const result = buildHistoryTrend([scan("b", 1, 1), scan("a", 0, 2)], "codeLines");
    const lines = historyTrendCsv(result).split("\r\n");
    expect(lines[0]).toContain("observed_delta");
    expect(lines[1]).toContain('"a","2026-01-01T00:00:00.000Z"');
    expect(lines[2]).toContain('"codeLines",1,-1,-50');
  });
  it("escapes spreadsheet formula prefixes and embedded CSV quotes", () => {
    const result = buildHistoryTrend([scan('=HYPERLINK("x")', 0, 2, { revision: "  +value", toolVersion: "@formula" })], "codeLines");
    const csv = historyTrendCsv(result);
    expect(csv).toContain(`"'=HYPERLINK(""x"")"`);
    expect(csv).toContain(`"'  +value"`);
    expect(csv).toContain(`"'@formula"`);
  });
  it("exports missing values as empty cells and preserves history coverage", () => {
    const result = buildHistoryTrend([scan("a", 0, -1)], "codeLines", {}, true);
    const csv = historyTrendCsv(result);
    expect(csv).toContain('"codeLines",,,,,"false","false","true"');
  });
});
