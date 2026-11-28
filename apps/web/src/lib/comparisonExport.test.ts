import { describe, expect, it } from "vitest";
import type { AnalysisChange, AnalysisComparison, ChangeGroup, ChangeKind } from "./analysisComparison";
import { buildComparisonExport, COMPARISON_SECTIONS, serializeComparisonExport, type ExportSelection } from "./comparisonExport";

function change(kind: ChangeKind, label: string, suffix = ""): AnalysisChange {
  return { key: `secret-internal-key-${label}`, label, kind, before: `old-${label}${suffix}`, after: `new-${label}${suffix}`, details: [`evidence-${label}${suffix}`] };
}
function group(changes: AnalysisChange[] = []): ChangeGroup {
  return { coverage: { current: "complete", baseline: "partial", absenceComparable: false, notes: ["secret-source-coverage"] }, ambiguousKeys: 3, unverified: 4, changes };
}
function fixture(): AnalysisComparison {
  return {
    files: group([change("added", "private/path/a.rs"), change("modified", "private/path/b.rs"), change("possible-move", "private/path/c.rs")]),
    dependencies: group([change("removed", "private-dependency"), change("metadata", "private-dependency-2")]),
    languages: group([change("only-current", "private-language")]),
    modules: group([change("only-baseline", "private-module")]),
    moduleEdges: group(),
    cycles: group(),
    findings: group([change("suppressed", "private-rule"), change("unsuppressed", "private-rule-2")]),
  };
}
const counts: ExportSelection = { mode: "counts", scope: "all" };
const details: ExportSelection = { mode: "details", scope: "all" };
function thrown(operation: () => unknown): unknown {
  try { operation(); return null; } catch (reason) { return reason; }
}

/** Parse CSV for assertions including quoted commas, quotes and multiline fields. */
function csvRecords(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = "";
  let quoted = false;
  for (let index = 0; index < text.length; index += 1) {
    const character = text[index];
    if (character === '"') {
      if (quoted && text[index + 1] === '"') { field += '"'; index += 1; }
      else quoted = !quoted;
    } else if (character === "," && !quoted) { row.push(field); field = ""; }
    else if (character === "\r" && text[index + 1] === "\n" && !quoted) {
      row.push(field); rows.push(row); row = []; field = ""; index += 1;
    } else field += character;
  }
  return rows;
}

describe("comparison export summary and privacy", () => {
  it("exports every inventory in a stable, explicit category order", () => {
    const report = buildComparisonExport(fixture(), counts);
    expect(report.groups.map((item) => item.section)).toEqual(COMPARISON_SECTIONS.map(([section]) => section));
    expect(report.direction).toBe("baseline-to-current");
    expect(report.format).toBe("repodna-comparison-1");
    expect(report.selectedChanges).toBe(9);
  });
  it("counts every change category without mixing absence and verified additions", () => {
    const report = buildComparisonExport(fixture(), counts);
    const files = report.groups[0]!;
    expect(files.selected.added).toBe(1);
    expect(files.selected.modified).toBe(1);
    expect(files.selected["possible-move"]).toBe(1);
    expect(files.selected.removed).toBe(0);
    expect(report.groups[2]?.selected["only-current"]).toBe(1);
    expect(report.groups[3]?.selected["only-baseline"]).toBe(1);
  });
  it("preserves coverage and ambiguous/unverified counts independently of selected changes", () => {
    const report = buildComparisonExport(fixture(), { mode: "counts", scope: "filtered", section: "files", kind: "modified" });
    expect(report.groups.length).toBe(1);
    expect(report.groups[0]?.inventoryChanges).toBe(3);
    expect(report.groups[0]?.selected.total).toBe(1);
    expect(report.groups[0]?.coverage).toEqual({ current: "complete", baseline: "partial", absenceComparable: false, ambiguousKeys: 3, unverified: 4 });
  });
  it("never includes source-derived text in the counts-only report", () => {
    const json = serializeComparisonExport(fixture(), counts, "json").text;
    const csv = serializeComparisonExport(fixture(), counts, "csv").text;
    for (const text of [json, csv]) {
      expect(text.includes("private/")).toBe(false);
      expect(text.includes("private-dependency")).toBe(false);
      expect(text.includes("secret-internal-key")).toBe(false);
      expect(text.includes("secret-source-coverage")).toBe(false);
    }
    expect(JSON.parse(json).sourceDerivedText).toBe(false);
    expect(JSON.parse(json).exportedDetails).toBe(0);
    expect(JSON.parse(json).omittedDetails).toBe(0);
  });
  it("does not leak private search terms even for a filtered summary", () => {
    const report = serializeComparisonExport(fixture(), { ...counts, scope: "filtered", section: "files", query: "private/path/a.rs" }, "json");
    expect(JSON.parse(report.text).selectedChanges).toBe(1);
    expect(report.text.includes("private/path/a.rs")).toBe(false);
  });
  it("retains explicit empty sections rather than treating them as missing", () => {
    const report = buildComparisonExport(fixture(), counts);
    expect(report.groups.find((item) => item.section === "cycles")?.selected.total).toBe(0);
    expect(report.groups.find((item) => item.section === "cycles")?.coverage.baseline).toBe("partial");
  });
  it("rejects a filtered export without a section", () => {
    expect(thrown(() => buildComparisonExport(fixture(), { mode: "counts", scope: "filtered" }))).toBeInstanceOf(Error);
  });
  it("combines change type and query filters using the view's own selection rules", () => {
    const report = buildComparisonExport(fixture(), { mode: "details", scope: "filtered", section: "files", kind: "modified", query: "B.RS" });
    expect(report.selectedChanges).toBe(1);
    expect(report.groups[0]?.details?.[0]?.label).toBe("private/path/b.rs");
  });
  it("does not apply a stale section or filter to an all-categories export", () => {
    const report = buildComparisonExport(fixture(), { ...counts, section: "files", kind: "removed", query: "no match" });
    expect(report.selectedChanges).toBe(9);
  });
});

describe("comparison detail limits and export contracts", () => {
  it("includes chosen detail fields but not internal change keys", () => {
    const report = buildComparisonExport(fixture(), details);
    expect(report.sourceDerivedText).toBe(true);
    expect(report.groups[0]?.details?.[0]).toEqual({ kind: "added", label: "private/path/a.rs", before: "old-private/path/a.rs", after: "new-private/path/a.rs", evidence: ["evidence-private/path/a.rs"] });
    expect(JSON.stringify(report).includes("secret-internal-key")).toBe(false);
    expect(JSON.stringify(report).includes("secret-source-coverage")).toBe(false);
  });
  it("does not mutate the analysis result or alias evidence arrays", () => {
    const source = fixture();
    const before = JSON.stringify(source);
    const report = buildComparisonExport(source, details);
    report.groups[0]?.details?.[0]?.evidence.push("changed");
    expect(JSON.stringify(source)).toBe(before);
  });
  it("applies a global row limit and retains full counts for every category", () => {
    const report = buildComparisonExport(fixture(), details, 4);
    expect(report.exportedDetails).toBe(4);
    expect(report.omittedDetails).toBe(5);
    expect(report.selectedChanges).toBe(9);
    expect(report.groups[0]?.exportedDetails).toBe(3);
    expect(report.groups[1]?.exportedDetails).toBe(1);
    expect(report.groups[1]?.omittedDetails).toBe(1);
    expect(report.groups.at(-1)?.selected.total).toBe(2);
  });
  it("supports zero detail rows without losing coverage or selected counts", () => {
    const report = buildComparisonExport(fixture(), details, 0);
    expect(report.exportedDetails).toBe(0);
    expect(report.omittedDetails).toBe(9);
    expect(report.groups[0]?.details).toEqual([]);
  });
  it("rejects invalid row and byte limits", () => {
    expect(thrown(() => buildComparisonExport(fixture(), details, -1))).toBeInstanceOf(RangeError);
    expect(thrown(() => serializeComparisonExport(fixture(), details, "json", { maxBytes: Infinity }))).toBeInstanceOf(RangeError);
    expect(thrown(() => serializeComparisonExport(fixture(), details, "json", { maxRows: 1.2 }))).toBeInstanceOf(RangeError);
  });
  it("refuses to drop summary information just to meet a tiny byte cap", () => {
    const failure = thrown(() => serializeComparisonExport(fixture(), counts, "json", { maxBytes: 1 }));
    expect(failure instanceof Error && failure.message).toContain("summary exceeds");
  });
  it("limits actual UTF-8 bytes and reports omitted details in JSON and CSV", () => {
    const source = fixture();
    source.files.changes = [change("modified", "😀".repeat(5000)), change("added", "small")];
    const selection: ExportSelection = { ...details, scope: "filtered", section: "files" };
    for (const format of ["json", "csv"] as const) {
      const summary = serializeComparisonExport(source, selection, format, { maxRows: 0 });
      const output = serializeComparisonExport(source, selection, format, { maxBytes: summary.bytes + 300 });
      expect(output.bytes <= summary.bytes + 300).toBe(true);
      expect(output.bytes).toBe(new TextEncoder().encode(output.text).byteLength);
      expect(output.exportedDetails).toBe(0);
      expect(output.omittedDetails).toBe(2);
    }
  });
  it("retains a small prefix rather than discarding every detail when a later item is too large", () => {
    const source = fixture();
    source.files.changes = [change("added", "small"), change("modified", "😀".repeat(5000))];
    const selection: ExportSelection = { ...details, scope: "filtered", section: "files" };
    const summary = serializeComparisonExport(source, selection, "json", { maxRows: 0 });
    const output = serializeComparisonExport(source, selection, "json", { maxBytes: summary.bytes + 2000 });
    expect(output.exportedDetails).toBe(1);
    expect(output.omittedDetails).toBe(1);
  });
  it("produces deterministic output, portable filenames, and media types", () => {
    const source = fixture();
    const json = serializeComparisonExport(source, counts, "json");
    expect(json).toEqual(serializeComparisonExport(source, counts, "json"));
    expect(json.filename).toBe("repodna-comparison-counts.json");
    expect(json.mime).toBe("application/json");
    const csv = serializeComparisonExport(source, details, "csv");
    expect(csv.filename).toBe("repodna-comparison-details.csv");
    expect(csv.mime).toBe("text/csv;charset=utf-8");
  });
});

describe("comparison CSV interoperability", () => {
  it("uses fixed-width records and preserves zero counts for all change kinds", () => {
    const rows = csvRecords(serializeComparisonExport(fixture(), counts, "csv").text);
    expect(rows.length).toBe(1 + 7 * 9);
    expect(rows.every((row) => row.length === rows[0]?.length)).toBe(true);
    const first = rows[1]!;
    expect(first[0]).toBe("count");
    expect(first[1]).toBe("files");
    expect(first[14]).toBe("added");
    expect(first[15]).toBe("1");
    expect(rows.some((row) => row[0] === "count" && row[1] === "files" && row[14] === "removed" && row[15] === "0")).toBe(true);
  });
  it("includes detail rows only for explicit detail export", () => {
    const rows = csvRecords(serializeComparisonExport(fixture(), details, "csv", { maxRows: 2 }).text);
    const records = rows.filter((row) => row[0] === "detail");
    expect(records.length).toBe(2);
    expect(records[0]?.[16]).toBe("private/path/a.rs");
    expect(records[0]?.[17]).toBe("old-private/path/a.rs");
    expect(records[0]?.[18]).toBe("new-private/path/a.rs");
    expect(records[0]?.[19]).toBe("evidence-private/path/a.rs");
    expect(records[0]?.[13]).toBe("1");
  });
  it("quotes commas, quotes and multiline evidence as a single CSV cell", () => {
    const source = fixture();
    source.files.changes = [change("modified", 'a,"b"\r\nc')];
    const rows = csvRecords(serializeComparisonExport(source, details, "csv").text);
    expect(rows.find((row) => row[0] === "detail")?.[16]).toBe('a,"b"\r\nc');
  });
  it("neutralizes formula-like source fields, including leading whitespace", () => {
    const source = fixture();
    source.files.changes = [{ ...change("modified", "=formula"), before: "  +input", after: "@value", details: ["\t-leading"] }];
    const row = csvRecords(serializeComparisonExport(source, details, "csv").text).find((item) => item[0] === "detail");
    expect(row?.slice(16)).toEqual(["'=formula", "'  +input", "'@value", "'\t-leading"]);
  });
  it("preserves literal text in JSON without changing the source-derived string", () => {
    const source = fixture();
    source.files.changes = [change("modified", "=formula")];
    const report = JSON.parse(serializeComparisonExport(source, details, "json").text);
    expect(report.groups[0].details[0].label).toBe("=formula");
  });
});
