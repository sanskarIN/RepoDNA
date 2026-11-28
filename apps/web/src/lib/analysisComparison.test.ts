import { describe, expect, it } from "vitest";
import type { DependencyRecord, FileRecord, Finding, RepositoryDna, LanguageStat, ModuleRecord, ModuleEdge, DependencyCycle } from "@repodna/schema";
import {
  compareAnalyses,
  filterAnalysisChanges,
  remoteIdentity,
  repositoryRelation,
} from "./analysisComparison";

function file(path: string, bytes = 20, hash: string | null = "0123456789abcdef"): FileRecord {
  return {
    path,
    bytes,
    hash,
    category: "source",
    language: "rust",
    lines: { code: 2, blank: 1, comment: 0, total: 3 },
  };
}

function dependency(name: string, requirement = "^1.0.0"): DependencyRecord {
  return {
    ecosystem: "npm",
    name,
    requirement,
    scope: "runtime",
    source: "registry",
    manifests: ["package.json"],
    resolved: ["1.0.1"],
  };
}

function finding(id: string): Finding {
  return {
    id,
    title: `Finding ${id}`,
    rule: "architecture.cycle",
    summary: "Observed cycle",
    category: "architecture",
    severity: "warning",
    confidence: "medium",
    rationale: "Review this observation",
    method: "Lexical inference",
    evidence: [],
    paths: ["src/main.rs"],
  };
}

/** The comparison engine reads only the selected artifact sections. Build a
 * small structurally valid projection rather than inventing unrelated metrics. */
function artifact(
  id: string,
  files: FileRecord[] = [],
  dependencies: DependencyRecord[] = [],
  findings: Finding[] = [],
): RepositoryDna {
  return {
    schemaVersion: "1.0",
    tool: { name: "RepoDNA", version: "1.5.0", schemaVersion: "1.0" },
    identity: { name: "repo", owner: "owner", remotes: [] },
    analysisMetadata: {
      id,
      configHash: "config-1",
      profile: "standard",
      partial: false,
      privacy: { preset: "local", redactions: [] },
    },
    structure: { status: "analyzed", files, totalFiles: files.length },
    dependencies: { status: "analyzed", dependencies },
    languages: { status: "analyzed", languages: [] },
    architecture: { status: "analyzed", method: "lexical", modules: [], moduleEdges: [], cycles: [] },
    findings,
  } as unknown as RepositoryDna;
}

function remote(dna: RepositoryDna, url: string, name = "origin"): void {
  dna.identity.remotes.push({ name, url, provider: "other" });
}

describe("repository identity evidence", () => {
  it("canonicalizes ordinary GitHub HTTPS and SSH remotes, including case differences", () => {
    for (const input of [
      "https://github.com/Owner/Repo.git",
      "https://www.github.com/owner/repo/",
      "git@github.com:OWNER/REPO.git",
      "ssh://git@github.com:22/owner/repo.git",
      "https://github.com:443/owner/repo",
    ]) {
      expect(remoteIdentity(input)).toBe("github.com/owner/repo");
    }
  });

  it("does not conflate custom SSH paths, ports, or case-sensitive providers", () => {
    expect(remoteIdentity("https://example.org/team/repo")).not.toBe(remoteIdentity("git@example.org:team/repo"));
    expect(remoteIdentity("ssh://git@example.org:2222/team/repo")).not.toBe(remoteIdentity("ssh://git@example.org/team/repo"));
    expect(remoteIdentity("git@example.org:team/repo")).not.toBe(remoteIdentity("git@example.org:/team/repo"));
    expect(remoteIdentity("https://gitlab.com/Group/Repo")).not.toBe(remoteIdentity("https://gitlab.com/group/repo"));
    expect(remoteIdentity("https://gitlab.com/team/subgroup/repo.git")).toBe("gitlab.com/team/subgroup/repo");
  });

  it("declines local, unsupported, malformed, encoded, and browser-page inputs", () => {
    for (const input of [
      "/tmp/project",
      "C:\\project",
      "file:///tmp/project",
      "ext::helper",
      "https://github.com/org/repo/tree/main",
      "https://github.com/org/../other/repo",
      "https://github.com/org/%72epo",
      "https://github.com/org/repo?token=secret",
      "https://github.com/org/repo#fragment",
      "https://github.com\\org\\repo",
      "https://github.com/org//repo",
    ]) {
      expect(remoteIdentity(input)).toBeNull();
    }
  });

  it("does not identify repositories merely because their display names match", () => {
    const a = artifact("a");
    const b = artifact("b");
    expect(repositoryRelation(a, b).relation).toBe("unknown");
    remote(a, "https://github.com/alice/repo");
    remote(b, "https://github.com/bob/repo");
    expect(repositoryRelation(a, b).relation).toBe("different");
  });

  it("does not mistake a shared upstream remote for an identical fork", () => {
    const a = artifact("a");
    const b = artifact("b");
    remote(a, "https://github.com/alice/repo");
    remote(b, "https://github.com/bob/repo");
    remote(a, "https://github.com/upstream/repo", "upstream");
    remote(b, "https://github.com/upstream/repo", "upstream");
    expect(repositoryRelation(a, b).relation).toBe("different");
  });

  it("uses an exact stored repository selection or identical analysis identifier", () => {
    expect(repositoryRelation(artifact("a"), artifact("b"), "store-1", "store-1").relation).toBe("same");
    expect(repositoryRelation(artifact("same"), artifact("same")).relation).toBe("same");
    expect(repositoryRelation(artifact("a"), artifact("b"), "store-1", "store-2").relation).toBe("unknown");
  });

  it("uses matching primary remotes while declining an ambiguous primary identity", () => {
    const a = artifact("a");
    const b = artifact("b");
    remote(a, "https://github.com/org/repo");
    remote(b, "git@github.com:org/repo.git");
    expect(repositoryRelation(a, b).relation).toBe("same");
    remote(b, "https://github.com/org/other");
    expect(repositoryRelation(a, b).relation).toBe("unknown");
  });
});

describe("file inventory comparison", () => {
  it("reports additions, removals, recorded content changes, and classification changes", () => {
    const before = artifact("b", [file("removed", 30, "1111111111111111"), file("modified"), file("metadata"), file("same")]);
    const after = artifact("a", [file("added", 10, "2222222222222222"), file("modified", 20, "abcdef0123456789"), { ...file("metadata"), category: "test" }, file("same")]);
    const changes = compareAnalyses(after, before).files;
    expect(changes.coverage.absenceComparable).toBe(true);
    expect(changes.changes.map((item) => [item.label, item.kind])).toEqual([
      ["added", "added"], ["metadata", "metadata"], ["modified", "modified"], ["removed", "removed"],
    ]);
    expect(changes.changes.find((item) => item.label === "modified")?.details).toContain("Recorded content hash changed");
  });

  it("uses byte-size evidence when the content hash was not recorded", () => {
    const result = compareAnalyses(artifact("a", [file("x", 30, null)]), artifact("b", [file("x", 20, null)])).files;
    expect(result.changes[0]?.kind).toBe("modified");
    expect(result.changes[0]?.details).toEqual(["Byte size changed"]);
    expect(result.unverified).toBe(0);
  });

  it("does not equate same-size files without valid content hashes", () => {
    const result = compareAnalyses(artifact("a", [file("x", 20, null), file("y", 20, "bad")]), artifact("b", [file("x", 20, null), file("y", 20, "bad")])).files;
    expect(result.changes).toEqual([]);
    expect(result.unverified).toBe(2);
  });

  it("does not infer removals from capped inventories or incomplete analysis", () => {
    const before = artifact("b", [file("old")]);
    const after = artifact("a", []);
    after.structure.truncated = true;
    after.structure.totalFiles = 200;
    const result = compareAnalyses(after, before).files;
    expect(result.coverage.current).toBe("partial");
    expect(result.coverage.absenceComparable).toBe(false);
    expect(result.changes[0]?.kind).toBe("only-baseline");
  });

  it("marks differences as observations when settings, analyzer versions, or privacy differ", () => {
    const before = artifact("b", [file("old")]);
    for (const modify of [
      (dna: RepositoryDna) => { dna.analysisMetadata.configHash = "different"; },
      (dna: RepositoryDna) => { dna.analysisMetadata.profile = "quick"; },
      (dna: RepositoryDna) => { dna.tool.version = "1.5.1"; },
      (dna: RepositoryDna) => { dna.analysisMetadata.privacy.preset = "public"; },
      (dna: RepositoryDna) => { dna.analysisMetadata.partial = true; },
      (dna: RepositoryDna) => { dna.analysisMetadata.privacy.redactions = ["paths removed"]; },
    ]) {
      const after = artifact("a", []);
      modify(after);
      const result = compareAnalyses(after, before).files;
      expect(result.coverage.absenceComparable).toBe(false);
      expect(result.changes[0]?.kind).toBe("only-baseline");
    }
  });

  it("recognizes a unique possible move without claiming Git rename detection", () => {
    const result = compareAnalyses(artifact("a", [file("new/path")]), artifact("b", [file("old/path")])).files;
    expect(result.changes.map((item) => [item.kind, item.before, item.after])).toEqual([
      ["possible-move", "old/path", "new/path"],
    ]);
  });

  it("does not pair duplicate-content candidates or a retained source path as moves", () => {
    for (const [after, before] of [
      [[file("new-1"), file("new-2")], [file("old")]],
      [[file("new")], [file("old-1"), file("old-2")]],
      [[file("same"), file("new")], [file("same"), file("old")]],
      [[file("same"), file("new")], [file("same")]],
    ] as [FileRecord[], FileRecord[]][]) {
      const result = compareAnalyses(artifact("a", after), artifact("b", before)).files;
      expect(result.changes.some((item) => item.kind === "possible-move")).toBe(false);
    }
  });

  it("does not pair identical hash candidates across mismatched analysis coverage", () => {
    const before = artifact("b", [file("old")]);
    const after = artifact("a", [file("new")]);
    after.analysisMetadata.configHash = "new config";
    const result = compareAnalyses(after, before).files;
    expect(result.changes.map((item) => item.kind)).toEqual(["only-current", "only-baseline"]);
  });

  it("excludes duplicate file identifiers without selecting arbitrary content", () => {
    const result = compareAnalyses(artifact("a", [file("x", 10), file("x", 20)]), artifact("b", [file("x", 30)])).files;
    expect(result.ambiguousKeys).toBe(1);
    expect(result.changes).toEqual([]);
  });

  it("compares recorded line count and skip availability as metadata changes", () => {
    const before = file("x");
    const after = { ...before, lines: null };
    const result = compareAnalyses(artifact("a", [after]), artifact("b", [before])).files;
    expect(result.changes[0]?.kind).toBe("metadata");
    expect(result.changes[0]?.details).toContain("Line-count availability changed");
  });

  it("produces deterministic results independent of inventory ordering", () => {
    const before = [file("z", 10, "1111111111111111"), file("a", 20, "2222222222222222")];
    const after = [file("b", 11, "3333333333333333"), file("z", 20, "1111111111111111")];
    const one = compareAnalyses(artifact("a", after), artifact("b", before));
    const two = compareAnalyses(artifact("a", [...after].reverse()), artifact("b", [...before].reverse()));
    expect(one).toEqual(two);
  });
});

describe("dependency comparison", () => {
  it("compares dependency versions, scopes and manifest membership under the same key", () => {
    const before = dependency("lib");
    const after: DependencyRecord = {
      ...before,
      requirement: "^2.0.0",
      resolved: ["2.0.0"],
      scope: "development",
      manifests: ["packages/lib/package.json"],
    };
    const result = compareAnalyses(artifact("a", [], [after]), artifact("b", [], [before])).dependencies;
    expect(result.changes.length).toBe(1);
    expect(result.changes[0]?.details).toEqual([
      "Declared requirement changed", "Resolved versions changed", "Usage scope changed", "Declaring manifests changed",
    ]);
  });

  it("does not report reordered version and manifest sets as changes", () => {
    const before = { ...dependency("lib"), resolved: ["1", "2"], manifests: ["a", "b"] };
    const after = { ...before, resolved: ["2", "1", "1"], manifests: ["b", "a"] };
    expect(compareAnalyses(artifact("a", [], [after]), artifact("b", [], [before])).dependencies.changes).toEqual([]);
  });

  it("does not merge identical names across package ecosystems", () => {
    const a = artifact("a", [], [dependency("lib")]);
    const b = artifact("b", [], [{ ...dependency("lib"), ecosystem: "cargo" }]);
    const changes = compareAnalyses(a, b).dependencies.changes;
    expect(changes.map((item) => [item.label, item.kind])).toEqual([["cargo: lib", "removed"], ["npm: lib", "added"]]);
  });

  it("detects reassociated requirements even when the aggregate field sets match", () => {
    const one = dependency("lib", "1");
    const two = { ...dependency("lib", "2"), scope: "development" as const };
    const after = [{ ...one, requirement: "2" }, { ...two, requirement: "1" }];
    const result = compareAnalyses(artifact("a", [], after), artifact("b", [], [one, two])).dependencies;
    expect(result.changes[0]?.details).toEqual(["Association of requirements, scopes or manifests changed"]);
  });

  it("detects lost dependencies only when both inventories were comparable", () => {
    const a = artifact("a");
    a.dependencies.status = "skipped";
    const result = compareAnalyses(a, artifact("b", [], [dependency("lib")])).dependencies;
    expect(result.coverage.current).toBe("unavailable");
    expect(result.changes[0]?.kind).toBe("only-baseline");
  });
});

describe("finding comparison", () => {
  it("distinguishes suppression transitions from disappeared findings", () => {
    const original = finding("s");
    const suppressed = { ...original, suppressed: { reason: "accepted", source: "repo" } };
    const result = compareAnalyses(artifact("a", [], [], [suppressed, finding("new")]), artifact("b", [], [], [original, finding("gone")])).findings;
    expect(result.coverage.absenceComparable).toBe(false);
    expect(result.changes.map((item) => [item.key, item.kind])).toEqual([
      ["gone", "only-baseline"], ["new", "only-current"], ["s", "suppressed"],
    ]);
    expect(compareAnalyses(artifact("a", [], [], [original]), artifact("b", [], [], [suppressed])).findings.changes[0]?.kind).toBe("unsuppressed");
  });

  it("records severity and confidence changes for stable finding IDs", () => {
    const before = finding("id");
    const after = { ...before, severity: "critical" as const, confidence: "high" as const };
    const change = compareAnalyses(artifact("a", [], [], [after]), artifact("b", [], [], [before])).findings.changes[0];
    expect(change?.kind).toBe("modified");
    expect(change?.details).toEqual(["Severity changed", "Confidence changed"]);
  });

  it("does not silently compare conflicting duplicate finding identifiers", () => {
    const before = finding("x");
    const result = compareAnalyses(artifact("a", [], [], [before, { ...before, title: "other" }]), artifact("b", [], [], [before])).findings;
    expect(result.ambiguousKeys).toBe(1);
    expect(result.changes).toEqual([]);
  });

  it("does not treat reordered path lists as changed evidence", () => {
    const before = { ...finding("id"), paths: ["a", "b"] };
    const after = { ...before, paths: ["b", "a"] };
    expect(compareAnalyses(artifact("a", [], [], [after]), artifact("b", [], [], [before])).findings.changes).toEqual([]);
  });

  it("filters the observed change set by kind and case-insensitive evidence text", () => {
    const result = compareAnalyses(artifact("a", [file("NEW")]), artifact("b", [file("old", 10)])).files;
    expect(filterAnalysisChanges(result.changes, "new", "all").map((change) => change.label)).toEqual(["NEW"]);
    expect(filterAnalysisChanges(result.changes, "", "removed").map((change) => change.label)).toEqual(["old"]);
    expect(filterAnalysisChanges(result.changes, "unmatched", "all")).toEqual([]);
  });
});

function language(id: string, overrides: Partial<LanguageStat> = {}): LanguageStat {
  return { id, name: id, files: 2, bytes: 100, codeLines: 10, commentLines: 2, blankLines: 1,
    capability: "lexical", kind: "programming", share: 0.5, ...overrides };
}

function module(id: string, overrides: Partial<ModuleRecord> = {}): ModuleRecord {
  return { id, name: id, path: id, kind: "directory", inferred: true, confidence: "medium",
    evidence: [], files: 2, codeLines: 20, fanIn: 1, fanOut: 1, instability: 0.5,
    centrality: 0.2, layer: 1, language: "rust", ...overrides };
}

function edge(from: string, to: string, weight = 1): ModuleEdge {
  return { from, to, weight, confidence: "medium", samples: [] };
}

function cycle(id: string, members = ["a", "b"], level: DependencyCycle["level"] = "module"): DependencyCycle {
  return { id, members, level, path: [...members, members[0]!], languages: ["rust"], evidence: [], confidence: "medium" };
}

describe("language and architecture inventory comparisons", () => {
  it("rejects malformed URL spellings and strips only one Git suffix", () => {
    expect(remoteIdentity("https:github.com/org/repo")).toBeNull();
    expect(remoteIdentity("https:/github.com/org/repo")).toBeNull();
    expect(remoteIdentity("https://github.com/org/repo.git.git")).toBe("github.com/org/repo.git");
    expect(remoteIdentity("https://github.com/org/REPO.GIT")).toBe("github.com/org/repo");
  });

  it("retains language additions and removals from the original comparison experience", () => {
    const before = artifact("before");
    const after = artifact("after");
    before.languages.languages = [language("rust"), language("python")];
    after.languages.languages = [language("rust"), language("typescript")];
    expect(compareAnalyses(after, before).languages.changes.map(({ label, kind }) => [label, kind])).toEqual([["python", "removed"], ["typescript", "added"]]);
  });

  it("reports changed language metrics, capability and family separately", () => {
    const before = artifact("before");
    const after = artifact("after");
    before.languages.languages = [language("typescript")];
    after.languages.languages = [language("typescript", { codeLines: 30, files: 4, share: 0.75, capability: "syntactic", kind: "data" })];
    const changes = compareAnalyses(after, before).languages.changes;
    expect(changes.length).toBe(1);
    expect(changes[0]?.details).toContain("Code lines: 10 → 30");
    expect(changes[0]?.details).toContain("Analysis capability: lexical → syntactic");
    expect(changes[0]?.details).toContain("Language family: programming → data");
  });

  it("does not treat skipped language measurements as removals", () => {
    const before = artifact("before");
    const after = artifact("after");
    before.languages.languages = [language("rust")];
    after.languages.status = "skipped";
    const result = compareAnalyses(after, before).languages;
    expect(result.coverage.current).toBe("unavailable");
    expect(result.changes[0]?.kind).toBe("only-baseline");
  });

  it("marks duplicate language identifiers as ambiguous rather than comparing arbitrary records", () => {
    const before = artifact("before");
    const after = artifact("after");
    before.languages.languages = [language("rust"), language("rust", { files: 99 })];
    after.languages.languages = [language("rust", { files: 3 })];
    const result = compareAnalyses(after, before).languages;
    expect(result.ambiguousKeys).toBe(1);
    expect(result.changes).toEqual([]);
  });

  it("reports module membership differences and boundary changes", () => {
    const before = artifact("before");
    const after = artifact("after");
    before.architecture.modules = [module("core"), module("old")];
    after.architecture.modules = [module("core", { inferred: false, kind: "package", fanIn: 5, layer: 0 }), module("new")];
    const result = compareAnalyses(after, before).modules;
    expect(result.changes.map(({ key, kind }) => [key, kind])).toEqual([["core", "modified"], ["new", "added"], ["old", "removed"]]);
    expect(result.changes[0]?.details).toContain("Boundary kind: directory → package");
    expect(result.changes[0]?.details).toContain("Incoming modules: 1 → 5");
    expect(result.changes[0]?.details).toContain("Layer: 1 → 0");
  });

  it("compares optional module dependencies as sets and optional metrics as nullable", () => {
    const before = artifact("before");
    const after = artifact("after");
    before.architecture.modules = [module("core", { layer: null, language: undefined, externalDependencies: ["a", "b", "a"] })];
    after.architecture.modules = [module("core", { layer: undefined, language: null, externalDependencies: ["b", "a"] })];
    expect(compareAnalyses(after, before).modules.changes).toEqual([]);
    after.architecture.modules[0]!.externalDependencies = ["a", "c"];
    expect(compareAnalyses(after, before).modules.changes[0]?.details).toEqual(["Recorded external dependencies changed"]);
  });

  it("does not claim that modules disappeared after the inference method changed", () => {
    const before = artifact("before");
    const after = artifact("after");
    before.architecture.modules = [module("core")];
    after.architecture.method = "syntactic";
    const result = compareAnalyses(after, before).modules;
    expect(result.coverage.absenceComparable).toBe(false);
    expect(result.coverage.notes).toContain("The recorded architecture inference methods differ.");
    expect(result.changes[0]?.kind).toBe("only-baseline");
  });

  it("preserves direction when comparing graph edges", () => {
    const before = artifact("before");
    const after = artifact("after");
    before.architecture.moduleEdges = [edge("a", "b")];
    after.architecture.moduleEdges = [edge("b", "a")];
    const result = compareAnalyses(after, before).moduleEdges;
    expect(result.changes.map(({ label, kind }) => [label, kind])).toEqual([["a → b", "removed"], ["b → a", "added"]]);
  });

  it("distinguishes edge IDs containing punctuation and arrow separators", () => {
    const before = artifact("before");
    const after = artifact("after");
    before.architecture.moduleEdges = [edge("a → b", "c"), edge("a", "b → c")];
    after.architecture.moduleEdges = [edge("a → b", "c", 5), edge("a", "b → c")];
    const result = compareAnalyses(after, before).moduleEdges;
    expect(result.changes.length).toBe(1);
    expect(result.ambiguousKeys).toBe(0);
    expect(result.changes[0]?.details).toEqual(["Supporting file-edge count: 1 → 5"]);
  });

  it("excludes duplicate module edges and reports confidence changes", () => {
    const before = artifact("before");
    const after = artifact("after");
    before.architecture.moduleEdges = [edge("a", "b"), edge("a", "b", 2), edge("c", "d")];
    after.architecture.moduleEdges = [edge("a", "b"), { ...edge("c", "d"), confidence: "high" }];
    const result = compareAnalyses(after, before).moduleEdges;
    expect(result.ambiguousKeys).toBe(1);
    expect(result.changes.length).toBe(1);
    expect(result.changes[0]?.details).toEqual(["Confidence: medium → high"]);
  });

  it("does not infer a repaired cycle from its absence", () => {
    const before = artifact("before");
    const after = artifact("after");
    before.architecture.cycles = [cycle("old")];
    after.architecture.cycles = [cycle("new")];
    const result = compareAnalyses(after, before).cycles;
    expect(result.coverage.absenceComparable).toBe(false);
    expect(result.changes.map(({ kind }) => kind)).toEqual(["only-current", "only-baseline"]);
  });

  it("distinguishes module and file cycles sharing an ID", () => {
    const before = artifact("before");
    const after = artifact("after");
    before.architecture.cycles = [cycle("same", ["a", "b"], "module")];
    after.architecture.cycles = [cycle("same", ["a", "b"], "file")];
    const result = compareAnalyses(after, before).cycles;
    expect(result.changes.length).toBe(2);
    expect(result.ambiguousKeys).toBe(0);
  });

  it("reports a new cycle witness without claiming that the component changed", () => {
    const before = artifact("before");
    const after = artifact("after");
    before.architecture.cycles = [cycle("same", ["a", "b", "c"])];
    after.architecture.cycles = [{ ...cycle("same", ["c", "b", "a"]), path: ["b", "a", "b"] }];
    const changes = compareAnalyses(after, before).cycles.changes;
    expect(changes[0]?.details).toEqual(["Recorded cycle witness changed"]);
  });

  it("qualifies file-cycle inventories when file-level edges are truncated", () => {
    const before = artifact("before");
    const after = artifact("after");
    before.architecture.cycles = [cycle("same", ["a", "b"], "file")];
    after.architecture.fileEdgesTruncated = true;
    expect(compareAnalyses(after, before).cycles.coverage.current).toBe("partial");
  });

  it("produces stable inventory output regardless of input order", () => {
    const before = artifact("before");
    const after = artifact("after");
    after.architecture.modules = [module("z"), module("a"), module("m")];
    after.architecture.moduleEdges = [edge("z", "a"), edge("a", "m")];
    const result = compareAnalyses(after, before);
    after.architecture.modules.reverse();
    after.architecture.moduleEdges.reverse();
    expect(compareAnalyses(after, before)).toEqual(result);
  });
});
