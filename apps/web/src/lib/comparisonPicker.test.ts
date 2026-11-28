import { describe, expect, it } from "vitest";
import type { RepositoryDna } from "@repodna/schema";
import type { Backend, RepositorySummary, ScanSummary } from "./backend";
import { Clock, deferred, flush } from "./controllerTestSupport";
import {
  ComparisonPickerController,
  PICKER_DEADLINES,
  matchingRepositories,
  otherStoredScans,
  pickerPage,
  type ComparisonSide,
} from "./comparisonPicker";
import type { DemoEntry } from "./demo";

function repository(id: string, name = id, scans = 1): RepositorySummary {
  return {
    id, name, scans,
    location: `/projects/${name}`,
    kind: "local-directory",
    firstScannedAt: "2026-11-28T00:00:00Z",
    lastScannedAt: "2026-11-28T00:00:00Z",
  };
}

function scan(id: string): ScanSummary {
  return {
    id,
    generatedAt: "2026-11-28T00:00:00Z",
    profile: "standard",
    dnaHash: "hash",
    toolVersion: "1.5.0",
    files: 2,
    codeLines: 3,
    commits: 1,
    contributors: 1,
    findings: { critical: 0, warning: 0, attention: 0, info: 0, suppressed: 0 },
  };
}

function dna(id: string): RepositoryDna {
  return { analysisMetadata: { id } } as RepositoryDna;
}

function backend(overrides: Partial<Pick<Backend, "repositories" | "artifact">> = {}) {
  const defaultBackend: Pick<Backend, "repositories" | "artifact"> = {
    repositories: async () => [repository("a")],
    artifact: async (id, scanId) => dna(scanId ?? id),
  };
  return { ...defaultBackend, ...overrides };
}

const demo: DemoEntry = { file: "demo.json", title: "Demo", description: "Example" };

function fixture(service: Pick<Backend, "repositories" | "artifact"> | null, demos: () => Promise<DemoEntry[]> = async () => [demo]) {
  const clock = new Clock();
  const results: ComparisonSide[] = [];
  let publications = 0;
  const controller = new ComparisonPickerController(service, demos, (side) => results.push(side), () => { publications += 1; }, clock);
  return { controller, clock, results, publications: () => publications };
}

describe("comparison picker choices", () => {
  it("excludes only the exact current scan, preserving same-timestamp analyses", () => {
    const scans = [scan("a"), scan("b"), scan("c")];
    expect(otherStoredScans(scans, "b").map((item) => item.id)).toEqual(["a", "c"]);
    expect(otherStoredScans(scans, "")).toEqual(scans);
  });

  it("filters stored repositories by name and path without duplicate or empty choices", () => {
    const entries = [repository("same"), repository("a", "Audio"), repository("b", "Editor"), repository("a", "Audio"), repository("c", "audio", 0)];
    expect(matchingRepositories(entries, "same", " AUDIO ").map((item) => item.id)).toEqual(["a"]);
    expect(matchingRepositories(entries, "same", "/projects/editor").map((item) => item.id)).toEqual(["b"]);
    expect(matchingRepositories(entries, "same", "").map((item) => item.id)).toEqual(["a", "b"]);
  });

  it("makes every stored choice reachable and clamps stale page indexes", () => {
    const items = Array.from({ length: 31 }, (_, index) => index);
    expect(pickerPage(items, 0, 12).items).toEqual(items.slice(0, 12));
    expect(pickerPage(items, 2, 12).items).toEqual(items.slice(24));
    expect(pickerPage(items, 90, 12).page).toBe(2);
    expect(pickerPage(items, -4, 12).page).toBe(0);
    expect(pickerPage(items, NaN, 12).page).toBe(0);
    expect(pickerPage([], 4, 12)).toEqual({ items: [], page: 0, pages: 1, total: 0 });
  });
});

describe("comparison picker lifecycle", () => {
  it("allows a static browser to select a file without a native backend", async () => {
    const { controller, results, clock } = fixture(null);
    await controller.loadInventory();
    expect(controller.state.demos).toEqual([demo]);
    expect(controller.state.repositories).toEqual([]);
    await controller.select("example.rep odna", async () => dna("file"));
    expect(results.map((item) => item.dna.analysisMetadata.id)).toEqual(["file"]);
    expect(clock.pending).toBe(0);
  });

  it("preserves a successful list if another inventory request fails", async () => {
    const { controller, clock } = fixture(backend({ repositories: async () => { throw new Error("offline"); } }));
    await controller.loadInventory();
    expect(controller.state.demos).toEqual([demo]);
    expect(controller.state.inventoryError).toContain("offline");
    expect(controller.state.inventoryLoading).toBe(false);
    expect(clock.pending).toBe(0);
  });

  it("does not block file selection while repository inventory is stalled", async () => {
    const pending = deferred<RepositorySummary[]>();
    const { controller, results, clock } = fixture(backend({ repositories: () => pending.promise }));
    const inventory = controller.loadInventory();
    await controller.select("file", async () => dna("file-id"));
    expect(results.length).toBe(1);
    expect(controller.state.inventoryLoading).toBe(true);
    await clock.advance(PICKER_DEADLINES.inventory);
    await inventory;
    expect(controller.state.inventoryError).toContain("did not respond");
    expect(controller.state.demos).toEqual([demo]);
    pending.resolve([repository("late")]);
    await flush();
    expect(controller.state.repositories).toEqual([]);
  });

  it("does not issue duplicate inventory requests while one is active", async () => {
    let calls = 0;
    const slow = deferred<RepositorySummary[]>();
    const { controller } = fixture(backend({ repositories: () => { calls += 1; return slow.promise; } }));
    const pending = controller.loadInventory();
    await controller.loadInventory();
    expect(calls).toBe(1);
    slow.resolve([repository("a")]);
    await pending;
    expect(controller.state.repositories.length).toBe(1);
  });

  it("keeps only the most recently selected baseline even if earlier results finish last", async () => {
    const slow = deferred<RepositoryDna>();
    const { controller, results, clock } = fixture(null);
    const previous = controller.select("previous", () => slow.promise);
    await controller.select("new", async () => dna("new"));
    slow.resolve(dna("old"));
    await previous;
    expect(results.map((item) => item.label)).toEqual(["new"]);
    expect(controller.state.selection).toBeNull();
    expect(clock.pending).toBe(0);
  });

  it("suppresses errors from a superseded or manually cancelled selection", async () => {
    const slow = deferred<RepositoryDna>();
    const { controller, results } = fixture(null);
    const pending = controller.select("old", () => slow.promise);
    controller.cancelSelection();
    slow.reject(new Error("late read error"));
    await pending;
    await flush();
    expect(results).toEqual([]);
    expect(controller.state.selectionError).toBeNull();
    expect(controller.state.selection).toBeNull();
  });

  it("releases a hung selection at its deadline and permits retry", async () => {
    const slow = deferred<RepositoryDna>();
    const { controller, results, clock } = fixture(null);
    const pending = controller.select("stalled", () => slow.promise);
    await clock.advance(PICKER_DEADLINES.selection);
    await pending;
    expect(controller.state.selectionError).toContain("did not respond");
    await controller.select("retry", async () => dna("new"));
    slow.resolve(dna("old"));
    await flush();
    expect(results.map((item) => item.label)).toEqual(["retry"]);
    expect(controller.state.selectionError).toBeNull();
  });

  it("validates the exact requested scan identifier and preserves the local repository identity", async () => {
    const calls: [string, string | undefined][] = [];
    const service = backend({ artifact: async (id, scanId) => {
      calls.push([id, scanId]);
      return dna(calls.length === 1 ? "wrong" : scanId ?? id);
    } });
    const { controller, results } = fixture(service);
    await controller.selectScan("repo", scan("requested"), "wrong result");
    expect(controller.state.selectionError).toContain("different stored analysis");
    expect(results).toEqual([]);
    await controller.selectScan("repo", scan("requested"), "right result");
    expect(results[0]?.repositoryId).toBe("repo");
    expect(results[0]?.dna.analysisMetadata.id).toBe("requested");
    expect(calls).toEqual([["repo", "requested"], ["repo", "requested"]]);
  });

  it("supports a legacy stored artifact that has no embedded analysis ID", async () => {
    const { controller, results } = fixture(backend({ artifact: async () => dna("") }));
    await controller.selectScan("repo", scan("legacy-fallback"), "legacy");
    expect(results.length).toBe(1);
    expect(results[0]?.repositoryId).toBe("repo");
  });

  it("does not request a repository with no stored scans", async () => {
    let calls = 0;
    const { controller, results } = fixture(backend({ artifact: async () => { calls += 1; return dna("a"); } }));
    await controller.selectRepository(repository("empty", "empty", 0));
    expect(calls).toBe(0);
    expect(results).toEqual([]);
  });

  it("disposes both inventory and selection observers without publishing late data", async () => {
    const slowList = deferred<RepositorySummary[]>();
    const slowSelection = deferred<RepositoryDna>();
    const { controller, results, clock, publications } = fixture(backend({ repositories: () => slowList.promise }));
    const inventory = controller.loadInventory();
    const selected = controller.select("old", () => slowSelection.promise);
    const before = publications();
    controller.dispose();
    slowList.resolve([repository("late")]);
    slowSelection.resolve(dna("late"));
    await Promise.all([inventory, selected]);
    expect(publications()).toBe(before);
    expect(results).toEqual([]);
    expect(clock.pending).toBe(0);
  });
});
