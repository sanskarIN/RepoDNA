import { describe, expect, it } from "vitest";
import type { Finding } from "@repodna/schema";
import { findingsCsv, findingsExportReview, findingsJson } from "./findings-export";
import { findingsMarkdown } from "./findings-markdown";

function sample(values: Record<string, unknown> = {}): Finding {
  return {
    id: "finding-1",
    rule: "security.example",
    title: "Example finding",
    summary: "Something needs review.",
    severity: "warning",
    category: "security",
    confidence: "high",
    evidence: [],
    paths: ["src/main.rs"],
    ...values,
  } as unknown as Finding;
}

describe("findings CSV export", () => {
  it("exports just the supplied findings in order, with named columns", () => {
    const csv = findingsCsv([sample({ title: "First" }), sample({ title: "Second" })]);
    const lines = csv.trim().split("\r\n");
    expect(lines[0]).toBe("Severity,Category,Rule,Title,Summary,Confidence,Paths,Suppressed");
    expect(lines).toHaveLength(3);
    expect(lines[1]).toContain(",First,");
    expect(lines[2]).toContain(",Second,");
  });

  it("escapes spreadsheet formulas, quotes and commas from untrusted repository text", () => {
    const csv = findingsCsv([
      sample({ title: "=1+2", summary: 'A, "quoted" finding', paths: ["=bad/path.rs"] }),
    ]);
    expect(csv).toContain(",'=1+2,");
    expect(csv).toContain('"A, ""quoted"" finding"');
    expect(csv).toContain("'=bad/path.rs");
  });

  it("exports only headers when no findings match", () => {
    expect(findingsCsv([]).split("\r\n")).toHaveLength(2);
  });
});

describe("findings JSON export", () => {
  it("keeps the supplied findings and their display order without changing any fields", () => {
    const first = sample({ id: "one", title: "First", severity: "critical" });
    const second = sample({ id: "two", title: "Second", severity: "warning" });
    const json = findingsJson([first, second]);
    expect(json.endsWith("\n")).toBe(true);
    expect(JSON.parse(json)).toEqual([first, second]);
  });

  it("preserves nested evidence and safely encodes untrusted repository text", () => {
    const record = sample({
      title: 'Quoted "message" with =SUM(1,2)',
      evidence: [{ kind: "source", path: "src/test.rs", detail: "line\n<script>" }],
      paths: ["src/a\tb.rs"],
    });
    const json = findingsJson([record]);
    expect(JSON.parse(json)).toEqual([record]);
    expect(json).toContain('Quoted \\"message\\"');
    expect(json).toContain("line\\n<script>");
  });

  it("serializes no matching findings as a valid empty array", () => {
    expect(findingsJson([])).toBe("[]\n");
  });
});

describe("filtered findings export review", () => {
  it("previews the first three findings but counts every matching record", () => {
    const findings = Array.from({ length: 5 }, (_, index) =>
      sample({
        id: "finding-" + index,
        title: "Result " + index,
        paths: ["src/" + index + ".rs"],
        evidence: [{ kind: "source", path: "src/" + index + ".rs", detail: "Checked" }],
        ...(index === 4 ? { suppressed: { reason: "accepted", source: "config" } } : {}),
      }),
    );
    const review = findingsExportReview(findings, "json");
    expect(review.paths).toBe(5);
    expect(review.evidenceItems).toBe(5);
    expect(review.suppressed).toBe(1);
    expect(review.sampled).toBe(3);
    expect(review.shortened).toBe(true);
    expect(JSON.parse(review.preview)).toEqual(findings.slice(0, 3));
    expect(review.preview).not.toContain("Result 4");
  });

  it("bounds long untrusted repository text before display", () => {
    const review = findingsExportReview([sample({ summary: "x".repeat(20_000) })], "markdown");
    expect(review.shortened).toBe(true);
    expect(review.preview).toContain("[Sample shortened]");
    expect(review.preview.length).toBeLessThan(4_100);
  });

  it("keeps a small preview in the requested format without removing evidence", () => {
    const findings = [sample({ paths: ["src/a.rs"] })];
    const csv = findingsExportReview(findings, "csv");
    const markdown = findingsExportReview(findings, "markdown");
    expect(csv.preview).toBe(findingsCsv(findings));
    expect(markdown.preview).toBe(findingsMarkdown(findings));
    expect(csv.shortened).toBe(false);
    expect(markdown.shortened).toBe(false);
  });
});
