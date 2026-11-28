import { describe, expect, it } from "vitest";
import { inspectConsistency, type ConsistencyArtifact } from "../src/artifactConsistency";

function sample() {
  return {
    schemaVersion: "1.0",
    tool: { schemaVersion: "1.0" },
    analysisMetadata: { partial: false as boolean, suppressedFindings: 0 },
    structure: {
      status: "analyzed", truncated: false as boolean, totalFiles: 2, totalBytes: 30,
      totalLines: 6, codeLines: 4, commentLines: 1, blankLines: 1,
      files: [
        { path: "src/a.rs", bytes: 20, lines: { total: 4, code: 2, comment: 1, blank: 1 }, module: "a" },
        { path: "src/b.rs", bytes: 10, lines: { total: 2, code: 2, comment: 0, blank: 0 }, module: "b" },
      ],
      directories: [
        { path: "", depth: 0, files: 2, bytes: 30, codeLines: 4 },
        { path: "src", depth: 1, files: 2, bytes: 30, codeLines: 4 },
      ],
      categories: [{ category: "source", files: 2, bytes: 30, lines: 6 }],
    },
    languages: { status: "analyzed", languages: [{ id: "rust", share: 1, files: 2 }], primary: ["rust"] },
    architecture: {
      status: "analyzed", fileEdgesTruncated: false as boolean,
      modules: [
        { id: "a", files: 1, codeLines: 2, fanIn: 0, fanOut: 1, instability: 1, centrality: 0, layer: 1 },
        { id: "b", files: 1, codeLines: 2, fanIn: 1, fanOut: 0, instability: 0, centrality: 0, layer: 0 },
      ],
      moduleEdges: [{ from: "a", to: "b", weight: 1 }],
      fileEdges: [{ from: "src/a.rs", to: "src/b.rs", line: 1 }],
      cycles: [] as { id: string; level: string; members: string[]; path: string[] }[],
      layers: [{ index: 0, modules: ["b"] }, { index: 1, modules: ["a"] }],
      isolatedModules: [] as string[],
    },
    dependencies: {
      status: "analyzed", directCount: 1,
      dependencies: [{ ecosystem: "cargo", name: "serde", scope: "runtime", source: "registry", manifests: ["Cargo.toml"], resolved: ["1"] }],
      concentration: [{ share: 1 }],
    },
    codeQuality: { status: "analyzed", duplication: { status: "analyzed", analyzedLines: 4, duplicatedLines: 1, ratio: 0.25, clusters: [{ occurrenceCount: 2, occurrences: [{}] }] } },
    similarity: { status: "analyzed", similarFiles: [{ a: "src/a.rs", b: "src/b.rs", similarity: 0.6 }] },
    tests: { status: "analyzed", testRatio: 0 },
    git: { status: "analyzed", commitCount: 4, ownership: { contributors: 2, contributorsForHalfOfCommits: 1, topContributorShare: 0.5 } },
    findings: [{ id: "warning-1", suppressed: null as unknown }],
  } satisfies ConsistencyArtifact;
}

type Sample = ReturnType<typeof sample>;
async function report(change: (dna: Sample) => void = () => undefined) {
  const dna = sample();
  change(dna);
  return inspectConsistency(dna, { yieldControl: async () => undefined });
}
async function has(code: string, change: (dna: Sample) => void) {
  const result = await report(change);
  expect(result.issues.map((issue) => issue.code)).toContain(code);
}

describe("artifact inventory consistency", () => {
  it("accepts a coherent snapshot and checks every record", async () => {
    const result = await report();
    expect(result.status).toBe("consistent");
    expect(result.observations).toBe(0);
    expect(result.complete).toBe(true);
    expect(result.checked > 20).toBe(true);
    expect(result.skipped).toEqual([]);
  });
  it("checks file line partitions", async () => {
    await has("line-total", (dna) => { dna.structure.files[0]!.lines.total = 9; });
  });
  it("checks aggregate line partitions even when discovery was partial", async () => {
    await has("line-total", (dna) => {
      dna.structure.status = "partial";
      dna.structure.totalLines = 19;
    });
  });
  it("detects duplicate file identities", async () => {
    await has("duplicate-file", (dna) => { dna.structure.files[1]!.path = dna.structure.files[0]!.path; });
  });
  it("detects non-normalized repository-relative paths", async () => {
    for (const path of ["../secret", "/root/file", "a//b", "a\\b", "a/./b", "C:/repo/file", "a\0b", ""]) {
      await has("file-path", (dna) => { dna.structure.files[0]!.path = path; });
    }
  });
  it("does not conflate case-distinct paths", async () => {
    const result = await report((dna) => {
      dna.structure.files[0]!.path = "src/Foo.rs";
      dna.structure.files[1]!.path = "src/foo.rs";
      dna.architecture.fileEdges = [];
    });
    expect(result.issues.some((issue) => issue.code === "duplicate-file")).toBe(false);
  });
  it("checks complete file byte totals", async () => {
    await has("inventory-total", (dna) => { dna.structure.totalBytes = 99; });
  });
  it("checks complete file counts", async () => {
    const result = await report((dna) => { dna.structure.totalFiles = 10; });
    expect(result.issues.some((issue) => issue.code === "inventory-total" && issue.path === "/structure/totalFiles")).toBe(true);
  });
  it("does not report missing inventory data for partial or capped analysis", async () => {
    for (const partial of [true, false]) {
      const result = await report((dna) => {
        dna.structure.files.pop();
        dna.structure.status = partial ? "partial" : "analyzed";
        dna.structure.truncated = !partial;
      });
      expect(result.issues.some((issue) => ["inventory-total", "unknown-file-endpoint"].includes(issue.code))).toBe(false);
      expect(result.skipped.length > 0).toBe(true);
    }
  });
  it("skips exact totals exceeding safe integer range", async () => {
    const result = await report((dna) => {
      dna.structure.files[0]!.bytes = Number.MAX_SAFE_INTEGER;
      dna.structure.files[1]!.bytes = 100;
    });
    expect(result.skipped.some((reason) => reason.includes("safe integer"))).toBe(true);
    expect(result.issues.some((issue) => issue.path === "/structure/totalBytes")).toBe(false);
  });
  it("detects duplicate directories", async () => {
    await has("duplicate-directory", (dna) => { dna.structure.directories.push({ ...dna.structure.directories[1]! }); });
  });
  it("detects inconsistent directory depth", async () => {
    await has("directory-depth", (dna) => { dna.structure.directories[1]!.depth = 9; });
  });
  it("accepts the repository root but rejects a traversing directory", async () => {
    await has("directory-path", (dna) => { dna.structure.directories[1]!.path = "../src"; });
    expect((await report()).issues.some((issue) => issue.code === "directory-path")).toBe(false);
  });
  it("checks the root recursive totals", async () => {
    await has("root-directory-total", (dna) => { dna.structure.directories[0]!.files = 99; });
  });
  it("checks category counts and uniqueness", async () => {
    const result = await report((dna) => { dna.structure.categories.push({ ...dna.structure.categories[0]! }); });
    expect(result.issues.map((issue) => issue.code)).toContain("duplicate-category");
    expect(result.issues.map((issue) => issue.code)).toContain("category-total");
  });
});

describe("artifact graph consistency", () => {
  it("checks unique language IDs and primary references", async () => {
    const result = await report((dna) => {
      dna.languages.languages.push({ ...dna.languages.languages[0]! });
      dna.languages.primary.push("unknown", "rust");
    });
    for (const code of ["duplicate-language", "duplicate-primary-language", "unknown-language"]) expect(result.issues.map((issue) => issue.code)).toContain(code);
  });
  it("does not treat missing primary languages in a partial list as corrupt", async () => {
    const result = await report((dna) => { dna.languages.status = "partial"; dna.languages.languages = []; });
    expect(result.issues.some((issue) => issue.code === "unknown-language")).toBe(false);
  });
  it("checks duplicate module IDs", async () => {
    await has("duplicate-module", (dna) => { dna.architecture.modules[1]!.id = "a"; });
  });
  it("checks directed module edge identities", async () => {
    await has("duplicate-module-edge", (dna) => { dna.architecture.moduleEdges.push({ ...dna.architecture.moduleEdges[0]! }); });
  });
  it("treats reverse module edges as distinct edges", async () => {
    const result = await report((dna) => {
      dna.architecture.moduleEdges.push({ from: "b", to: "a", weight: 1 });
    });
    expect(result.issues.some((issue) => issue.code === "duplicate-module-edge")).toBe(false);
  });
  it("does not conflate pairs whose identifiers contain separators", async () => {
    const result = await report((dna) => {
      dna.architecture.moduleEdges = [{ from: "x:y", to: "z", weight: 1 }, { from: "x", to: "y:z", weight: 1 }];
    });
    expect(result.issues.some((issue) => issue.code === "duplicate-module-edge")).toBe(false);
  });
  it("detects missing endpoints in complete graphs", async () => {
    await has("unknown-module-endpoint", (dna) => { dna.architecture.moduleEdges[0]!.to = "absent"; });
  });
  it("does not infer missing module endpoints from a partial graph", async () => {
    const result = await report((dna) => { dna.architecture.status = "partial"; dna.architecture.modules = []; });
    expect(result.issues.some((issue) => issue.code === "unknown-module-endpoint" || issue.code === "module-degree")).toBe(false);
  });
  it("checks zero supporting file edge counts", async () => {
    await has("empty-edge", (dna) => { dna.architecture.moduleEdges[0]!.weight = 0; });
  });
  it("checks degree counts and instability separately", async () => {
    const result = await report((dna) => { dna.architecture.modules[0]!.fanIn = 10; });
    expect(result.issues.map((issue) => issue.code)).toContain("module-degree");
    expect(result.issues.map((issue) => issue.code)).toContain("instability");
  });
  it("checks file edge endpoints and one-based declaration lines", async () => {
    const result = await report((dna) => { dna.architecture.fileEdges[0]!.to = "missing"; dna.architecture.fileEdges[0]!.line = 0; });
    expect(result.issues.map((issue) => issue.code)).toContain("unknown-file-endpoint");
    expect(result.issues.map((issue) => issue.code)).toContain("invalid-line");
  });
  it("checks layer membership and duplicate layer indices", async () => {
    const result = await report((dna) => { dna.architecture.layers.push({ index: 1, modules: ["b", "absent"] }); });
    for (const code of ["duplicate-layer", "duplicate-layer-module", "unknown-layer-module", "module-layer"]) expect(result.issues.map((issue) => issue.code)).toContain(code);
  });
  it("checks isolated module references and degree", async () => {
    const result = await report((dna) => { dna.architecture.isolatedModules = ["a", "a", "unknown"]; });
    for (const code of ["duplicate-isolated-module", "unknown-isolated-module", "isolated-module-edge"]) expect(result.issues.map((issue) => issue.code)).toContain(code);
  });
  it("detects open and incomplete cycle witnesses", async () => {
    const result = await report((dna) => { dna.architecture.cycles = [{ id: "c", level: "module", members: ["a", "a", "absent"], path: ["a", "b"] }]; });
    for (const code of ["duplicate-cycle-member", "unknown-cycle-member", "open-cycle", "cycle-witness-member"]) expect(result.issues.map((issue) => issue.code)).toContain(code);
  });
  it("checks witness steps against complete directed graphs", async () => {
    await has("cycle-witness-edge", (dna) => { dna.architecture.cycles = [{ id: "c", level: "module", members: ["a", "b"], path: ["a", "b", "a"] }]; });
  });
  it("does not infer missing witness edges from capped file edge lists", async () => {
    const result = await report((dna) => {
      dna.architecture.fileEdgesTruncated = true;
      dna.architecture.cycles = [{ id: "c", level: "file", members: ["src/a.rs", "src/b.rs"], path: ["src/a.rs", "src/b.rs", "src/a.rs"] }];
    });
    expect(result.issues.some((issue) => issue.code === "cycle-witness-edge")).toBe(false);
    expect(result.skipped.some((reason) => reason.includes("truncated edge list"))).toBe(true);
  });
});

describe("artifact metric relationships and bounded diagnostics", () => {
  it("checks the two schema version declarations", async () => {
    await has("schema-version", (dna) => { dna.tool.schemaVersion = "2.0"; });
  });
  it("tolerates equivalent major-only schema version spelling", async () => {
    expect((await report((dna) => { dna.tool.schemaVersion = "1"; })).observations).toBe(0);
  });
  it("distinguishes dependency scope and source", async () => {
    const result = await report((dna) => {
      dna.dependencies.dependencies.push({ ...dna.dependencies.dependencies[0]!, scope: "development" });
      dna.dependencies.dependencies.push({ ...dna.dependencies.dependencies[0]!, source: "path" });
    });
    expect(result.issues.some((issue) => issue.code === "duplicate-dependency")).toBe(false);
  });
  it("checks repeated dependency records and repeated values", async () => {
    const result = await report((dna) => {
      dna.dependencies.dependencies.push({ ...dna.dependencies.dependencies[0]! });
      dna.dependencies.dependencies[0]!.resolved.push("1");
    });
    expect(result.issues.map((issue) => issue.code)).toContain("duplicate-dependency");
    expect(result.issues.map((issue) => issue.code)).toContain("duplicate-dependency-value");
  });
  it("checks normalized shares and ratios", async () => {
    const result = await report((dna) => {
      dna.languages.languages[0]!.share = -0.1;
      dna.architecture.modules[0]!.centrality = 1.2;
      dna.dependencies.concentration[0]!.share = 3;
      dna.tests.testRatio = 1.5;
      dna.similarity.similarFiles[0]!.similarity = -1;
      dna.git.ownership.topContributorShare = 5;
    });
    expect(result.issues.filter((issue) => issue.code === "ratio-range").length).toBe(6);
  });
  it("checks duplication counts and derived ratios", async () => {
    const result = await report((dna) => { dna.codeQuality.duplication.duplicatedLines = 9; });
    expect(result.issues.map((issue) => issue.code)).toContain("duplication-count");
    expect(result.issues.map((issue) => issue.code)).toContain("duplication-ratio");
  });
  it("permits an intentionally capped duplicate occurrence list", async () => {
    expect((await report()).issues.some((issue) => issue.code === "occurrence-count")).toBe(false);
    await has("occurrence-count", (dna) => { dna.codeQuality.duplication.clusters[0]!.occurrenceCount = 0; });
  });
  it("identifies duplicate unordered similarity pairs and self pairs", async () => {
    const result = await report((dna) => {
      dna.similarity.similarFiles.push({ a: "src/b.rs", b: "src/a.rs", similarity: 0.6 });
      dna.similarity.similarFiles.push({ a: "src/a.rs", b: "src/a.rs", similarity: 1 });
    });
    expect(result.issues.map((issue) => issue.code)).toContain("self-similarity");
    expect(result.issues.map((issue) => issue.code)).toContain("duplicate-similarity");
  });
  it("checks contributor counts against the analyzed commits", async () => {
    await has("contributor-count", (dna) => { dna.git.ownership.contributors = 10; });
  });
  it("checks suppressed finding counts without counting null as suppression", async () => {
    await has("suppression-count", (dna) => { dna.findings[0]!.suppressed = {}; });
    expect((await report((dna) => { dna.analysisMetadata.partial = true; dna.findings[0]!.suppressed = {}; })).issues.some((issue) => issue.code === "suppression-count")).toBe(false);
  });
  it("does not expose instance values in diagnostic messages or paths", async () => {
    const result = await report((dna) => { dna.structure.files[0]!.path = "../private-secret-name"; });
    expect(JSON.stringify(result).includes("private-secret-name")).toBe(false);
  });
  it("counts omitted observations after the diagnostic cap", async () => {
    const dna = sample();
    dna.structure.files[0]!.path = "../bad";
    dna.structure.files[1]!.path = "../bad";
    const result = await inspectConsistency(dna, { maxIssues: 1 });
    expect(result.issues.length).toBe(1);
    expect(result.observations > 1).toBe(true);
    expect(result.omittedIssues).toBe(result.observations - 1);
  });
  it("marks a clean prefix incomplete when its work budget runs out", async () => {
    const result = await inspectConsistency(sample(), { maxSteps: 1 });
    expect(result.status).toBe("incomplete");
    expect(result.checked).toBe(1);
    expect(result.complete).toBe(false);
  });
  it("yields cooperatively and honors cancellation", async () => {
    const controller = new AbortController();
    let yields = 0;
    let failure: unknown;
    try {
      await inspectConsistency(sample(), { signal: controller.signal, batch: 1, yieldControl: async () => { yields += 1; controller.abort(); } });
    } catch (reason) { failure = reason; }
    expect(yields).toBe(1);
    expect(failure instanceof DOMException && failure.name).toBe("AbortError");
  });
  it("rejects invalid configurable work limits", async () => {
    let error: unknown;
    try { await inspectConsistency(sample(), { maxSteps: 0 }); } catch (reason) { error = reason; }
    expect(error).toBeInstanceOf(RangeError);
  });
  it("does not mutate the document", async () => {
    const dna = sample();
    const before = JSON.stringify(dna);
    await inspectConsistency(dna);
    expect(JSON.stringify(dna)).toBe(before);
  });
});
