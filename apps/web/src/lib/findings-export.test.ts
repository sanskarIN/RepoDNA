import { describe, expect, it } from "vitest";
import type { Finding } from "@repodna/schema";
import { findingsCsv } from "./findings-export";

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
