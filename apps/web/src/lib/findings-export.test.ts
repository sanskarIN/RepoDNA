import { describe, expect, it } from "vitest";
import type { Finding } from "@repodna/schema";
import { findingsCsv, findingsJson } from "./findings-export";

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
