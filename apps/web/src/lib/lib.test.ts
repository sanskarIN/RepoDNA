import { describe, expect, it } from "vitest";
import type { RepositoryDna } from "@repodna/schema";
import { continuousDays } from "./activity";
import { artifactFileName } from "./download";
import { contributorNamer, count, languageNamer } from "./names";
import { href, parseHash } from "./router";

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
