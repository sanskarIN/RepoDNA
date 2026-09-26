import { describe, expect, it } from "vitest";
import {
  ArtifactError,
  checkArtifact,
  describeEvidence,
  evidencePath,
  findingCounts,
  parseArtifact,
  severityLabel,
  severityRank,
  type RepositoryDna,
} from "../src/index";

function minimal(version = "1.0"): Record<string, unknown> {
  return {
    schemaVersion: version,
    tool: { name: "RepoDNA", version: "1.0.0", schemaVersion: "1.0" },
    identity: { name: "widget" },
    analysisMetadata: {},
    findings: [
      { id: "a", severity: "warning", suppressed: null },
      { id: "b", severity: "info" },
      { id: "c", severity: "critical", suppressed: { reason: "accepted", source: "repodna.toml" } },
    ],
  };
}

describe("loading artifacts", () => {
  it("accepts schema 1.x and notes newer minor versions", () => {
    const loaded = parseArtifact(JSON.stringify(minimal()));
    expect(loaded.warnings).toEqual([]);
    expect(loaded.artifact.identity.name).toBe("widget");
    const newer = checkArtifact(minimal("1.7"));
    expect(newer.warnings[0]).toContain("newer RepoDNA");
  });

  it("rejects documents it cannot read", () => {
    expect(() => parseArtifact("{")).toThrow(ArtifactError);
    expect(() => parseArtifact("[]")).toThrow("not a JSON object");
    expect(() => checkArtifact({})).toThrow("no schemaVersion");
    expect(() => checkArtifact(minimal("2.0"))).toThrow("newer RepoDNA");
    expect(() => checkArtifact(minimal("0.9"))).toThrow("cannot read");
    expect(() => checkArtifact({ ...minimal(), identity: null })).toThrow("identity");
  });
});

describe("helpers", () => {
  it("counts active and suppressed findings", () => {
    const dna = checkArtifact(minimal()).artifact as RepositoryDna;
    expect(findingCounts(dna)).toEqual({
      critical: 0,
      warning: 1,
      attention: 0,
      info: 1,
      suppressed: 1,
    });
    expect(severityRank("critical")).toBeGreaterThan(severityRank("warning"));
    expect(severityLabel("attention")).toBe("Attention");
  });

  it("describes evidence like the Rust model", () => {
    expect(describeEvidence({ kind: "file", path: "src/a.rs", line: 3, endLine: 9 })).toBe(
      "src/a.rs:3-9",
    );
    expect(describeEvidence({ kind: "file", path: "src/a.rs", note: "entry" })).toBe(
      "src/a.rs — entry",
    );
    expect(describeEvidence({ kind: "directory", path: "" })).toBe("(repository root)/");
    expect(
      describeEvidence({ kind: "commit", hash: "0123456789abcdef", date: "2026-01-02T03:04:05Z" }),
    ).toBe("commit 0123456 (2026-01-02)");
    expect(describeEvidence({ kind: "dependency-edge", from: "a", to: "b" })).toBe(
      "dependency edge: a → b",
    );
    expect(
      describeEvidence({ kind: "package", ecosystem: "npm", name: "react", version: "19" }),
    ).toBe("npm package react 19");
    expect(
      describeEvidence({
        kind: "metric",
        metric: "lines",
        value: 1200,
        threshold: 1000,
        unit: "lines",
      }),
    ).toBe("lines = 1200 lines (threshold 1000 lines)");
    expect(describeEvidence({ kind: "observation", text: "No README" })).toBe("No README");
    expect(evidencePath({ kind: "symbol", path: "a.ts", name: "f" })).toBe("a.ts");
    expect(evidencePath({ kind: "observation", text: "x" })).toBeUndefined();
  });
});
