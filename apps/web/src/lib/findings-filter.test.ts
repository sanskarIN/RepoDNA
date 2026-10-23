import { describe, expect, it } from "vitest";
import type { Finding } from "@repodna/schema";
import { filterFindings, type FindingFilters } from "./findings-filter";

function finding(title: string, overrides: Record<string, unknown> = {}): Finding {
  return {
    id: "shared-id",
    title,
    summary: "",
    rule: "test.rule",
    category: "security",
    severity: "warning",
    evidence: [],
    paths: [],
    ...overrides,
  } as unknown as Finding;
}

const defaults: FindingFilters = {
  query: "",
  severity: "all",
  category: "all",
  showSuppressed: false,
};

describe("findings search and export filtering", () => {
  it("does not mix up searchable text when imported findings repeat an ID", () => {
    const first = finding("Alpha report", { paths: ["src/alpha.rs"] });
    const second = finding("Beta report", { paths: ["src/beta.rs"] });
    expect(filterFindings([first, second], { ...defaults, query: "ALPHA" })).toEqual([first]);
    expect(filterFindings([first, second], { ...defaults, query: "beta.rs" })).toEqual([second]);
  });

  it("combines case-insensitive search terms with severity and category", () => {
    const wanted = finding("Circular dependency", {
      severity: "critical",
      category: "architecture",
      paths: ["src/graph.rs"],
    });
    const unrelated = finding("Circular dependency", { severity: "warning" });
    const options: FindingFilters = {
      ...defaults,
      query: "circular graph.rs",
      severity: "critical",
      category: "architecture",
    };
    expect(filterFindings([unrelated, wanted], options)).toEqual([wanted]);
  });

  it("omits suppressed results unless the review explicitly includes them", () => {
    const suppressed = finding("Known issue", {
      suppressed: { reason: "reviewed", source: "config" },
    });
    expect(filterFindings([suppressed], defaults)).toEqual([]);
    expect(filterFindings([suppressed], { ...defaults, showSuppressed: true })).toEqual([
      suppressed,
    ]);
  });

  it("keeps analysis display order within each severity", () => {
    const warning = finding("Warning", { severity: "warning" });
    const critical = finding("Critical", { severity: "critical" });
    const warningTwo = finding("Another warning", { severity: "warning" });
    expect(filterFindings([warning, critical, warningTwo], defaults)).toEqual([
      critical,
      warning,
      warningTwo,
    ]);
  });
});
