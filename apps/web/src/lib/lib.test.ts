import { describe, expect, it } from "vitest";
import type { Finding, RepositoryDna } from "@repodna/schema";
import { continuousDays } from "./activity";
import { artifactFileName } from "./download";
import { highlights } from "./findings";
import { parseMarkdown } from "./markdown";
import { contributorNamer, count, languageNamer, unitFor } from "./names";
import { href, parseHash } from "./router";
import privacySource from "../../../../PRIVACY.md?raw";
import termsSource from "../../../../TERMS.md?raw";

describe("router", () => {
  it("parses paths and query parameters from the hash", () => {
    const route = parseHash("#/files?q=src%2Fmain.rs");
    expect(route.path).toBe("/files");
    expect(route.params.get("q")).toBe("src/main.rs");
    expect(parseHash("").path).toBe("/");
    expect(parseHash("#overview").path).toBe("/overview");
  });

  it("builds links that round-trip", () => {
    const link = href("/architecture", { module: "crates/core" });
    expect(link).toBe("#/architecture?module=crates%2Fcore");
    expect(parseHash(link).params.get("module")).toBe("crates/core");
    expect(href("/")).toBe("#/");
  });
});

describe("names", () => {
  const dna = {
    identity: { name: "Demo Project!" },
    analysisMetadata: { generatedAt: "2026-09-24T10:00:00Z" },
    languages: { languages: [{ id: "rust", name: "Rust" }] },
    git: { contributors: [{ id: "c-1", name: "Ada" }] },
  } as unknown as RepositoryDna;

  it("turns identifiers into display names", () => {
    const language = languageNamer(dna);
    expect(language("rust")).toBe("Rust");
    expect(language("zig")).toBe("zig");
    expect(language(null)).toBe("–");
    expect(contributorNamer(dna)("c-1")).toBe("Ada");
    expect(contributorNamer(dna)("c-2")).toBe("c-2");
  });

  it("counts with singular and plural nouns", () => {
    expect(count(1, "file", "files")).toBe("1 file");
    expect(count(1200, "file", "files")).toBe("1,200 files");
    expect(count(0, "file", "files")).toBe("0 files");
  });

  it("names exported files like the command line", () => {
    expect(artifactFileName(dna)).toBe("repodna-demo-project-2026-09-24.repodna");
  });
});

describe("activity", () => {
  it("fills the days between commits", () => {
    const days = continuousDays([
      { date: "2026-02-27", commits: 2, churn: 10 },
      { date: "2026-03-02", commits: 1, churn: 4 },
    ]);
    expect(days?.map((d) => [d.date, d.commits])).toEqual([
      ["2026-02-27", 2],
      ["2026-02-28", 0],
      ["2026-03-01", 0],
      ["2026-03-02", 1],
    ]);
  });

  it("gives up on empty, long, or unreadable spans", () => {
    expect(continuousDays([])).toBeNull();
    expect(
      continuousDays([
        { date: "2025-01-01", commits: 1, churn: 1 },
        { date: "2026-01-01", commits: 1, churn: 1 },
      ]),
    ).toBeNull();
    expect(continuousDays([{ date: "yesterday", commits: 1, churn: 1 }])).toBeNull();
  });
});

describe("units", () => {
  it("puts units in the singular for one", () => {
    expect(unitFor(1, "files")).toBe("file");
    expect(unitFor(2, "files")).toBe("files");
    expect(unitFor(1.001, "effective languages")).toBe("effective language");
    expect(unitFor(1, "checks met")).toBe("check met");
    expect(unitFor(1.5, "checks met")).toBe("checks met");
    expect(unitFor(1, "dependencies")).toBe("dependency");
    expect(unitFor(1, "branches")).toBe("branch");
    expect(unitFor(1, "ratio")).toBe("ratio");
    expect(unitFor(1, "share of code files")).toBe("share of code files");
  });
});

describe("findings", () => {
  const finding = (rule: string, id: string, suppressed = false) =>
    ({
      rule,
      id,
      ...(suppressed ? { suppressed: { reason: "known" } } : {}),
    }) as unknown as Finding;

  it("highlights one finding per rule first, in display order", () => {
    const findings = [
      finding("a", "a1"),
      finding("a", "a2"),
      finding("b", "b1", true),
      finding("c", "c1"),
      finding("d", "d1"),
    ];
    expect(highlights(findings, 2).map((f) => f.id)).toEqual(["a1", "c1"]);
    expect(highlights(findings, 4).map((f) => f.id)).toEqual(["a1", "a2", "c1", "d1"]);
  });
});

describe("markdown", () => {
  it("parses headings, paragraphs, and lists with wrapped items", () => {
    const blocks = parseMarkdown(
      "# Title\n\nFirst line\nsecond line.\n\n## Part\n\n- One\n  continued\n- Two\nAfter.\n",
    );
    expect(blocks).toEqual([
      { kind: "heading", level: 1, text: "Title" },
      { kind: "paragraph", text: "First line second line." },
      { kind: "heading", level: 2, text: "Part" },
      { kind: "list", items: ["One continued", "Two"] },
      { kind: "paragraph", text: "After." },
    ]);
  });

  it("reads the repository's policy documents", () => {
    for (const source of [privacySource, termsSource]) {
      const blocks = parseMarkdown(source);
      expect(blocks[0]?.kind).toBe("heading");
      expect(blocks.filter((block) => block.kind === "heading" && block.level === 1)).toHaveLength(
        1,
      );
    }
  });
});
