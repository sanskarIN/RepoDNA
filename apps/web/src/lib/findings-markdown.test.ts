import { describe, expect, it } from "vitest";
import type { Finding } from "@repodna/schema";
import { findingsMarkdown } from "./findings-markdown";

function finding(overrides: Partial<Finding> = {}): Finding {
  return {
    id: "finding-1",
    rule: "security.example",
    title: "Example finding",
    summary: "Something needs review.",
    severity: "warning",
    category: "security",
    confidence: "high",
    evidence: [{ kind: "file", path: "src/main.rs", line: 42 }],
    method: "Static analysis",
    rationale: "Potential security impact",
    paths: ["src/main.rs"],
    limitations: ["Lexical analysis only"],
    nextSteps: ["Review affected file"],
    ...overrides,
  };
}

describe("findings Markdown review", () => {
  it("includes every supplied finding in display order, with evidence and next steps", () => {
    const first = finding({ title: "First", severity: "critical" });
    const second = finding({ title: "Second", severity: "info" });
    const report = findingsMarkdown([first, second]);
    expect(report).toContain("Matching findings: 2");
    expect(report.indexOf("## 1. First")).toBeLessThan(report.indexOf("## 2. Second"));
    expect(report).toContain("- **Severity:** Critical");
    expect(report).toContain("**Evidence**");
  });

  it("preserves evidence descriptions, limitations, and suppression notes", () => {
    const report = findingsMarkdown([
      finding({
        evidence: [{ kind: "file", path: "src/main.rs", line: 42 }],
        suppressed: { source: "repodna.toml", reason: "Reviewed" },
      }),
    ]);
    expect(report).toContain("src/main\\.rs:42");
    expect(report).toContain("**Limitations**");
    expect(report).toContain("**Suggested next steps**");
    expect(report).toContain("- **Suppressed:** Yes");
    expect(report).toContain("- **Suppression reason:** Reviewed");
  });

  it("prevents untrusted findings from injecting headings, links, and HTML", () => {
    const report = findingsMarkdown([
      finding({
        title: "Review\n## Forged heading",
        summary: "<script>alert(1)</script> [click](https://example.org)",
        paths: ["src/a|b.md"],
      }),
    ]);
    expect(report).not.toContain("\n## Forged heading");
    expect(report).not.toContain("<script>");
    expect(report).not.toContain("[click](https://example.org)");
    expect(report).toContain("&lt;script&gt;");
    expect(report).toContain("\\[click\\]\\(https://example\\.org\\)");
    expect(report).toContain("a\\|b\\.md");
  });

  it("exports no matching findings as a valid standalone document", () => {
    const report = findingsMarkdown([]);
    expect(report).toContain("Matching findings: 0");
    expect(report).not.toContain("## 1.");
    expect(report.endsWith("\n")).toBe(true);
  });
});
