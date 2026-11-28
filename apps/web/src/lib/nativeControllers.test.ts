import { describe, expect, it } from "vitest";
import type { HistoryPage, JobState, ScanSummary, SourceReference, SourceReferences } from "./backend";
import {
  RequestAbandonedError,
  RequestDeadlineError,
  RequestScope,
} from "./requestDeadline";
import { SCAN_REQUEST_DEADLINES, ScanJobController, type ScanBackend } from "./scanJobController";
import {
  HISTORY_DEADLINE,
  StoredHistoryController,
  type HistoryBackend,
} from "./storedHistoryController";
import {
  REFERENCE_REQUEST_DEADLINE,
  RemoteReferenceController,
  referencePage,
  referenceQueryError,
  type DiscoverReferences,
} from "./remoteReferenceController";

import { Clock, deferred, flush } from "./controllerTestSupport";

function job(status: JobState["status"] = "running", id = "job-1"): JobState {
  return {
    id,
    input: "/repo",
    status,
    stages: [],
    filesDone: 0,
    filesTotal: 0,
    warnings: [],
    startedAt: "2026-11-28T00:00:00Z",
  };
}

function completed(id = "job-1"): JobState {
  return { ...job("completed", id), repositoryId: "repo-1", scanId: "scan-1" };
}

function scanBackend(overrides: Partial<ScanBackend> = {}): ScanBackend {
  return {
    startScan: async () => job(),
    job: async (id) => job("running", id),
    cancelJob: async () => undefined,
    ...overrides,
  };
}

function summary(id: string): ScanSummary {
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

function historyPage(id: string, ids: string[], cursor: string | null): HistoryPage {
  return { repositoryId: id, scans: ids.map(summary), nextCursor: cursor };
}

function historyBackend(history: NonNullable<HistoryBackend["history"]>): HistoryBackend {
  return {
    history,
    repository: async () => {
      throw new Error("unexpected legacy request");
    },
  };
}

function reference(name: string, kind: SourceReference["kind"] = "branch"): SourceReference {
  return { name, kind, object: "a".repeat(40) };
}

function listing(...references: SourceReference[]): SourceReferences {
  return { references, truncated: false };
}

describe("request deadlines", () => {
  it("returns successful values and clears the timer", async () => {
    const clock = new Clock();
    const scope = new RequestScope(clock);
    expect(await scope.run("Read", 100, async () => 7)).toBe(7);
    expect(clock.pending).toBe(0);
  });

  it("consumes late rejection after a deadline without changing the outcome", async () => {
    const clock = new Clock();
    const scope = new RequestScope(clock);
    const slow = deferred<number>();
    const result = scope.run("Read", 100, () => slow.promise).catch((reason: unknown) => reason);
    await clock.advance(99);
    expect(clock.pending).toBe(1);
    await clock.advance(1);
    expect(await result).toBeInstanceOf(RequestDeadlineError);
    slow.reject(new Error("late rejection"));
    await flush();
    expect(clock.pending).toBe(0);
  });

  it("closes every observer without waiting for native promises", async () => {
    const clock = new Clock();
    const scope = new RequestScope(clock);
    const slow = deferred<number>();
    const tasks = [1, 2, 3].map(() =>
      scope.run("Read", 100, () => slow.promise).catch((reason: unknown) => reason),
    );
    expect(clock.pending).toBe(3);
    scope.close();
    scope.close();
    for (const result of await Promise.all(tasks)) {
      expect(result).toBeInstanceOf(RequestAbandonedError);
    }
    expect(clock.pending).toBe(0);
    const closed = await scope.run("Read", 100, async () => 1).catch((reason: unknown) => reason);
    expect(closed).toBeInstanceOf(RequestAbandonedError);
    slow.resolve(4);
    await flush();
  });

  it("handles synchronous exceptions and asynchronous failures without leaking timers", async () => {
    const clock = new Clock();
    const scope = new RequestScope(clock);
    const error = new Error("failed");
    for (const task of [
      () => { throw error; },
      async () => { throw error; },
    ]) {
      expect(await scope.run("Read", 100, task).catch((reason: unknown) => reason)).toBe(error);
      expect(clock.pending).toBe(0);
    }
  });
});

describe("native scan controller", () => {
  it("does not automatically repeat a timed-out start, and discards the late job ID", async () => {
    const clock = new Clock();
    const pending = deferred<JobState>();
    let starts = 0;
    let opens = 0;
    const service = scanBackend({ startScan: () => { starts += 1; return pending.promise; } });
    const controller = new ScanJobController(service, () => { opens += 1; }, () => undefined, clock);
    const first = controller.start("/repo", "standard");
    await controller.start("/other", "quick");
    expect(starts).toBe(1);
    await clock.advance(SCAN_REQUEST_DEADLINES.start);
    await first;
    expect(controller.state.busy).toBe(false);
    expect(controller.state.error).toContain("may still have started");
    pending.resolve(completed());
    await flush();
    expect(opens).toBe(0);
    expect(controller.state.job).toBeNull();
    expect(starts).toBe(1);
    expect(clock.pending).toBe(0);
  });

  it("bounds stalled progress checks, resumes by ID, and ignores old successful responses", async () => {
    const clock = new Clock();
    const slow = deferred<JobState>();
    const ids: string[] = [];
    const opened: string[] = [];
    const service = scanBackend({
      job: (id) => {
        ids.push(id);
        return ids.length <= 3 ? slow.promise : Promise.resolve(completed(id));
      },
    });
    const controller = new ScanJobController(service, (id) => { opened.push(id); }, () => undefined, clock);
    await controller.start("/repo", "standard");
    await clock.advance(400 + SCAN_REQUEST_DEADLINES.poll);
    expect(controller.state.pollError).toContain("did not respond");
    await clock.advance(800 + SCAN_REQUEST_DEADLINES.poll);
    await clock.advance(1600 + SCAN_REQUEST_DEADLINES.poll);
    expect(controller.state.paused).toBe(true);
    expect(controller.state.busy).toBe(true);
    expect(ids).toEqual(["job-1", "job-1", "job-1"]);
    await clock.advance(60_000);
    expect(ids.length).toBe(3);
    controller.resume();
    await clock.advance(0);
    expect(opened).toEqual(["repo-1"]);
    expect(controller.state.busy).toBe(false);
    slow.resolve({ ...completed(), repositoryId: "wrong" });
    await flush();
    expect(opened).toEqual(["repo-1"]);
    expect(clock.pending).toBe(0);
  });

  it("rejects mismatched job identities at recovery and polling boundaries", async () => {
    const clock = new Clock();
    const service = scanBackend({ job: async () => completed("other") });
    let opens = 0;
    const controller = new ScanJobController(service, () => { opens += 1; }, () => undefined, clock);
    await controller.attach("job-1");
    expect(controller.state.error).toContain("different analysis job");
    expect(controller.state.job).toBeNull();
    await controller.start("/repo", "standard");
    await clock.advance(400);
    expect(controller.state.pollError).toContain("different analysis job");
    expect(opens).toBe(0);
    controller.dispose();
    expect(clock.pending).toBe(0);
  });

  it("treats cancellation acknowledgement as nonterminal and retries a rejected cancellation", async () => {
    const clock = new Clock();
    let cancellations = 0;
    const service = scanBackend({
      cancelJob: async () => {
        cancellations += 1;
        if (cancellations === 1) throw new Error("connection lost");
      },
      job: async (id) => job("cancelled", id),
    });
    const controller = new ScanJobController(service, () => undefined, () => undefined, clock);
    await controller.start("/repo", "standard");
    await controller.stop();
    expect(controller.state.cancelling).toBe(false);
    expect(controller.state.error).toBe("connection lost");
    await controller.stop();
    expect(controller.state.cancelling).toBe(true);
    expect(controller.state.job?.status).toBe("running");
    await clock.advance(0);
    expect(controller.state.job?.status).toBe("cancelled");
    expect(controller.state.busy).toBe(false);
    expect(controller.state.cancelling).toBe(false);
  });

  it("allows a new cancellation request after the acknowledgement deadline", async () => {
    const clock = new Clock();
    const slow = deferred<void>();
    let count = 0;
    const service = scanBackend({
      cancelJob: () => ++count === 1 ? slow.promise : Promise.resolve(),
    });
    const controller = new ScanJobController(service, () => undefined, () => undefined, clock);
    await controller.start("/repo", "standard");
    const first = controller.stop();
    await controller.stop();
    expect(count).toBe(1);
    await clock.advance(SCAN_REQUEST_DEADLINES.cancel);
    await first;
    expect(controller.state.cancelling).toBe(false);
    expect(controller.state.error).toContain("did not respond");
    await controller.stop();
    expect(count).toBe(2);
    slow.resolve(undefined);
    await flush();
    expect(controller.state.cancelling).toBe(true);
    controller.dispose();
    expect(clock.pending).toBe(0);
  });

  it("keeps the launch reserved until opening its completed result finishes", async () => {
    const clock = new Clock();
    const slow = deferred<void>();
    let starts = 0;
    const service = scanBackend({ startScan: async () => { starts += 1; return completed(); } });
    const controller = new ScanJobController(service, () => slow.promise, () => undefined, clock);
    await controller.start("/repo", "standard");
    expect(controller.state.busy).toBe(true);
    expect(controller.state.starting).toBe(false);
    await controller.start("/other", "quick");
    expect(starts).toBe(1);
    slow.reject(new Error("cannot open result"));
    await flush();
    expect(controller.state.busy).toBe(false);
    expect(controller.state.error).toBe("cannot open result");
  });

  it("does not publish after disposal, including a delayed result-opening error", async () => {
    const clock = new Clock();
    const slow = deferred<void>();
    let publications = 0;
    const controller = new ScanJobController(
      scanBackend({ startScan: async () => completed() }),
      () => slow.promise,
      () => { publications += 1; },
      clock,
    );
    await controller.start("/repo", "quick");
    const before = publications;
    controller.dispose();
    slow.reject(new Error("old failure"));
    await flush();
    expect(publications).toBe(before);
    expect(clock.pending).toBe(0);
  });

  it("does not publish late progress or open a result after the controller is disposed", async () => {
    const clock = new Clock();
    const slow = deferred<JobState>();
    let opens = 0;
    const controller = new ScanJobController(
      scanBackend({ job: () => slow.promise }),
      () => { opens += 1; },
      () => undefined,
      clock,
    );
    await controller.start("/repo", "quick");
    await clock.advance(400);
    controller.dispose();
    slow.resolve(completed());
    await clock.advance(60_000);
    expect(opens).toBe(0);
    expect(clock.pending).toBe(0);
  });

  it("does not allow stop or resume while a recovery identity is unconfirmed", async () => {
    const clock = new Clock();
    const slow = deferred<JobState>();
    let reads = 0;
    let stops = 0;
    const controller = new ScanJobController(scanBackend({
      job: () => { reads += 1; return slow.promise; },
      cancelJob: async () => { stops += 1; },
    }), () => undefined, () => undefined, clock);
    const attach = controller.attach("job-1");
    controller.resume();
    await controller.stop();
    await clock.advance(400);
    expect(reads).toBe(1);
    expect(stops).toBe(0);
    slow.resolve(job());
    await attach;
    controller.dispose();
    expect(clock.pending).toBe(0);
  });
});

describe("stored history controller", () => {
  it("detects a multi-page cursor cycle and preserves successfully committed history", async () => {
    const clock = new Clock();
    const responses = [
      historyPage("a", ["1"], "cursor-a"),
      historyPage("a", ["2"], "cursor-b"),
      historyPage("a", ["3"], "cursor-a"),
      historyPage("a", ["3"], null),
    ];
    const controller = new StoredHistoryController(historyBackend(async () => responses.shift()!), () => undefined, clock);
    controller.toggle("a");
    await flush();
    controller.more();
    await flush();
    controller.more();
    await flush();
    expect(controller.state.error).toContain("did not advance");
    expect(controller.state.scans?.map((scan) => scan.id)).toEqual(["1", "2"]);
    expect(controller.state.cursor).toBe("cursor-b");
    controller.more();
    await flush();
    expect(controller.state.scans?.map((scan) => scan.id)).toEqual(["1", "2", "3"]);
    expect(controller.state.cursor).toBeNull();
  });

  it("releases a hung first page and retries the same repository", async () => {
    const clock = new Clock();
    const slow = deferred<HistoryPage>();
    let calls = 0;
    const controller = new StoredHistoryController(historyBackend((id) => {
      calls += 1;
      return calls === 1 ? slow.promise : Promise.resolve(historyPage(id, ["new"], null));
    }), () => undefined, clock);
    controller.toggle("a");
    await clock.advance(HISTORY_DEADLINE);
    expect(controller.state.loading).toBe(false);
    expect(controller.state.error).toContain("did not respond");
    await controller.refresh("a");
    slow.resolve(historyPage("a", ["old"], null));
    await flush();
    expect(controller.state.scans?.map((scan) => scan.id)).toEqual(["new"]);
    expect(clock.pending).toBe(0);
  });

  it("retains committed scans and cursor during a stalled append", async () => {
    const clock = new Clock();
    const slow = deferred<HistoryPage>();
    let requests = 0;
    const controller = new StoredHistoryController(historyBackend(async (id) => {
      requests += 1;
      if (requests === 1) return historyPage(id, ["first"], "next");
      if (requests === 2) return slow.promise;
      return historyPage(id, ["first", "second"], null);
    }), () => undefined, clock);
    controller.toggle("a");
    await flush();
    controller.more();
    controller.more();
    await clock.advance(HISTORY_DEADLINE);
    expect(requests).toBe(2);
    expect(controller.state.scans?.map((scan) => scan.id)).toEqual(["first"]);
    expect(controller.state.cursor).toBe("next");
    controller.more();
    await flush();
    expect(controller.state.scans?.map((scan) => scan.id)).toEqual(["first", "second"]);
    slow.reject(new Error("late append failure"));
    await flush();
    expect(controller.state.error).toBeNull();
  });

  it("rejects cross-repository data and allows a corrected page to be retried", async () => {
    const clock = new Clock();
    let calls = 0;
    const controller = new StoredHistoryController(historyBackend(async (id) => {
      calls += 1;
      return historyPage(calls === 1 ? "other" : id, ["record"], null);
    }), () => undefined, clock);
    controller.toggle("a");
    await flush();
    expect(controller.state.error).toContain("different repository");
    expect(controller.state.scans).toBeNull();
    await controller.refresh("a");
    expect(controller.state.scans?.map((scan) => scan.id)).toEqual(["record"]);
  });

  it("does not retain cursor or scan indexes after switching repositories", async () => {
    const clock = new Clock();
    const slow = deferred<HistoryPage>();
    const controller = new StoredHistoryController(historyBackend((id) =>
      id === "old" ? slow.promise : Promise.resolve(historyPage(id, ["same"], "next")),
    ), () => undefined, clock);
    controller.toggle("old");
    controller.toggle("new");
    await flush();
    slow.resolve(historyPage("old", ["stale"], "next"));
    await flush();
    expect(controller.state.id).toBe("new");
    expect(controller.state.scans?.map((scan) => scan.id)).toEqual(["same"]);
    controller.toggle("new");
    expect(controller.state.id).toBeNull();
    expect(clock.pending).toBe(0);
  });

  it("rejects empty or duplicate-only nonterminal pages without advancing the cursor", async () => {
    const clock = new Clock();
    const responses = [
      historyPage("a", ["1"], "a"),
      historyPage("a", [], "b"),
      historyPage("a", ["1"], "b"),
      historyPage("a", ["2"], null),
    ];
    const controller = new StoredHistoryController(historyBackend(async () => responses.shift()!), () => undefined, clock);
    controller.toggle("a");
    await flush();
    for (let i = 0; i < 2; i += 1) {
      controller.more();
      await flush();
      expect(controller.state.error).toContain("did not advance");
      expect(controller.state.cursor).toBe("a");
    }
    controller.more();
    await flush();
    expect(controller.state.scans?.map((scan) => scan.id)).toEqual(["1", "2"]);
  });

  it("disposes a pending history request without publishing or leaking its timer", async () => {
    const clock = new Clock();
    const slow = deferred<HistoryPage>();
    let publishes = 0;
    const controller = new StoredHistoryController(historyBackend(() => slow.promise), () => { publishes += 1; }, clock);
    controller.toggle("a");
    controller.dispose();
    slow.resolve(historyPage("a", ["late"], null));
    await flush();
    expect(publishes).toBe(1);
    expect(clock.pending).toBe(0);
  });
});

describe("remote reference controller", () => {
  it("validates byte limits rather than JavaScript code-unit lengths", () => {
    expect(referenceQueryError("a".repeat(512))).toBeNull();
    expect(referenceQueryError("日".repeat(170))).toBeNull();
    expect(referenceQueryError("日".repeat(171))).toContain("512");
    expect(referenceQueryError("a\u0000b")).toContain("control");
    expect(referenceQueryError("a\u0085b")).toContain("control");
    expect(referenceQueryError(" release ")).toBeNull();
  });

  it("paginates every retained choice without issuing more network requests", async () => {
    const clock = new Clock();
    let requests = 0;
    const refs = Array.from({ length: 327 }, (_, index) => reference(`branch-${index}`));
    const controller = new RemoteReferenceController("remote", async () => {
      requests += 1;
      return listing(...refs);
    }, () => undefined, clock);
    expect(requests).toBe(0);
    await controller.load();
    expect(referencePage(controller.state).pages).toBe(3);
    expect(referencePage(controller.state).references.length).toBe(150);
    controller.setPage(2);
    expect(referencePage(controller.state).references.length).toBe(27);
    controller.select("branch-326");
    expect(referencePage(controller.state).choice?.name).toBe("branch-326");
    controller.setPage(1);
    expect(controller.state.selected).toBe("");
    controller.select("branch-326");
    expect(controller.state.selected).toBe("");
    expect(requests).toBe(1);
  });

  it("does not call a scoped response complete for a broader filter or another namespace", async () => {
    const clock = new Clock();
    const controller = new RemoteReferenceController("remote", async () => listing(reference("feature/audio")), () => undefined, clock);
    controller.setQuery("feature");
    await controller.search();
    expect(referencePage(controller.state).complete).toBe(true);
    controller.setQuery("feature/audio");
    expect(referencePage(controller.state).complete).toBe(true);
    controller.setQuery("");
    expect(referencePage(controller.state).complete).toBe(false);
    controller.setKind("tag");
    expect(referencePage(controller.state).complete).toBe(false);
    expect(referencePage(controller.state).total).toBe(0);
  });

  it("preserves namespace identity even when branch and tag names coincide", async () => {
    const clock = new Clock();
    const controller = new RemoteReferenceController("remote", async () => listing(reference("release"), reference("release", "tag")), () => undefined, clock);
    await controller.load();
    controller.select("release");
    expect(referencePage(controller.state).choice?.kind).toBe("branch");
    controller.setKind("tag");
    expect(controller.state.selected).toBe("");
    controller.select("release");
    expect(referencePage(controller.state).choice?.kind).toBe("tag");
  });

  it("retains a successful listing on timeout, retries, and ignores an earlier late reply", async () => {
    const clock = new Clock();
    const slow = deferred<SourceReferences>();
    let count = 0;
    const discover: DiscoverReferences = () => {
      count += 1;
      if (count === 2) return slow.promise;
      return Promise.resolve(listing(reference(count === 1 ? "first" : "new")));
    };
    const controller = new RemoteReferenceController("remote", discover, () => undefined, clock);
    await controller.load();
    const pending = controller.load();
    await controller.load();
    expect(count).toBe(2);
    await clock.advance(REFERENCE_REQUEST_DEADLINE);
    await pending;
    expect(controller.state.error).toContain("did not respond");
    expect(referencePage(controller.state).references.map((ref) => ref.name)).toEqual(["first"]);
    await controller.load();
    slow.resolve(listing(reference("old")));
    await flush();
    expect(referencePage(controller.state).references.map((ref) => ref.name)).toEqual(["new"]);
    expect(controller.state.error).toBeNull();
    expect(clock.pending).toBe(0);
  });

  it("does not call the remote for invalid search or publish after disposal", async () => {
    const clock = new Clock();
    const slow = deferred<SourceReferences>();
    let requests = 0;
    let publications = 0;
    const controller = new RemoteReferenceController("remote", () => {
      requests += 1;
      return slow.promise;
    }, () => { publications += 1; }, clock);
    controller.setQuery("日".repeat(171));
    await controller.search();
    expect(requests).toBe(0);
    expect(controller.state.error).toContain("512");
    const pending = controller.load();
    const before = publications;
    controller.dispose();
    slow.resolve(listing(reference("late")));
    await pending;
    expect(publications).toBe(before);
    expect(clock.pending).toBe(0);
  });

  it("reports truncated listings as incomplete even if all retained matches fit one page", async () => {
    const clock = new Clock();
    const controller = new RemoteReferenceController("remote", async () => ({ references: [reference("main")], truncated: true }), () => undefined, clock);
    await controller.load();
    expect(referencePage(controller.state).total).toBe(1);
    expect(referencePage(controller.state).complete).toBe(false);
  });
});
