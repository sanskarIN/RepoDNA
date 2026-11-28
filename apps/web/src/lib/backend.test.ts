import { afterEach, describe, expect, it, vi } from "vitest";
import {
  checkScanJob,
  checkScanJobs,
  checkSourceReferences,
  ServerBackend,
  takeToken,
} from "./backend";

const originalUrl = window.location.href;

const validRepository = {
  id: "repo-1",
  name: "Project",
  location: "/projects/example",
  kind: "local",
  scans: 1,
  firstScannedAt: "2026-10-28T00:00:00Z",
  lastScannedAt: "2026-10-28T00:00:00Z",
};

const validScan = {
  id: "scan-1",
  generatedAt: "2026-10-28T00:00:00Z",
  profile: "quick",
  revision: null,
  dnaHash: "hash",
  toolVersion: "1.4.3",
  files: 1,
  codeLines: 2,
  commits: 0,
  contributors: 0,
  findings: { critical: 0, warning: 0, attention: 0, info: 0, suppressed: 0 },
};

const validJob = {
  id: "job-1",
  input: "/projects/example",
  status: "running",
  stage: null,
  stages: [],
  filesDone: 0,
  filesTotal: 1,
  repositoryId: null,
  scanId: null,
  error: null,
  warnings: [],
  startedAt: "2026-10-28T00:00:00Z",
  finishedAt: null,
};

afterEach(() => {
  vi.restoreAllMocks();
  window.sessionStorage.removeItem("repodna-token");
  window.history.replaceState(null, "", originalUrl);
});

describe("local server API responses", () => {
  it("rejects a successful response that contains HTML instead of JSON", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response("<h1>Proxy error</h1>", { status: 200 }),
    );
    await expect(new ServerBackend(null).session()).rejects.toMatchObject({
      name: "BackendError",
      status: 200,
      message: "The server returned an invalid JSON response.",
    });
  });

  it("rejects an empty successful response for a JSON endpoint", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("", { status: 200 }));
    await expect(new ServerBackend(null).repositories()).rejects.toThrow(
      "The server returned an invalid JSON response.",
    );
  });

  it("preserves useful API error messages for unsuccessful responses", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ error: "Access denied" }), { status: 401 }),
    );
    await expect(new ServerBackend(null).session()).rejects.toMatchObject({
      status: 401,
      message: "Access denied",
    });
  });

  it("deletes saved repositories and scans through authenticated encoded URLs", async () => {
    const fetchSpy = vi.spyOn(globalThis, "fetch");
    fetchSpy.mockImplementation(async () => new Response("{}", { status: 200 }));
    const backend = new ServerBackend("local-token");
    await backend.deleteRepository("repo /1");
    await backend.deleteScan("repo /1", "scan /2");
    expect(fetchSpy.mock.calls.map(([url]) => url)).toEqual([
      "/api/repositories/repo%20%2F1",
      "/api/repositories/repo%20%2F1/scans/scan%20%2F2",
    ]);
    for (const [, init] of fetchSpy.mock.calls) {
      expect(init).toMatchObject({
        method: "DELETE",
        credentials: "same-origin",
        cache: "no-store",
      });
      expect((init?.headers as Headers).get("X-RepoDNA-Token")).toBe("local-token");
    }
  });

  it("allows an empty successful deletion response", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(null, { status: 204 }));
    await expect(new ServerBackend(null).deleteRepository("repo-1")).resolves.toBeUndefined();
  });

  it("allows an empty successful cancellation response", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(null, { status: 204 }));
    await expect(new ServerBackend(null).cancelJob("job-1")).resolves.toBeUndefined();
  });

  it("rejects a malformed successful session response", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ version: "1.4.2", allowScans: "yes" }), { status: 200 }),
    );
    await expect(new ServerBackend(null).session()).rejects.toThrow(
      "The server returned an invalid session.",
    );
  });
  it("rejects non-array repository list responses", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ repositories: [] }), { status: 200 }),
    );
    await expect(new ServerBackend(null).repositories()).rejects.toThrow(
      "The server returned an invalid repository list.",
    );
  });

  it("accepts a valid repository list response", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify([]), { status: 200 }),
    );
    await expect(new ServerBackend(null).repositories()).resolves.toEqual([]);
  });

  it("rejects malformed list entries, even inside a valid array", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify([{ ...validRepository, scans: -1 }]), { status: 200 }),
    );
    await expect(new ServerBackend(null).repositories()).rejects.toThrow(
      "The server returned an invalid repository list.",
    );
  });

  it("accepts complete repository metadata", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify([validRepository]), { status: 200 }),
    );
    await expect(new ServerBackend(null).repositories()).resolves.toEqual([validRepository]);
  });

  it("accepts complete repository details and scan summaries", async () => {
    const value = { repository: validRepository, scans: [validScan] };
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify(value), { status: 200 }),
    );
    await expect(new ServerBackend(null).repository("repo-1")).resolves.toEqual(value);
  });

  it("rejects a scan with malformed nested finding counts", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(
        JSON.stringify({
          repository: validRepository,
          scans: [{ ...validScan, findings: { ...validScan.findings, critical: "two" } }],
        }),
        { status: 200 },
      ),
    );
    await expect(new ServerBackend(null).repository("repo-1")).rejects.toThrow(
      "The server returned invalid repository details.",
    );
  });

  it("rejects repository details without an array of scans", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ repository: { id: "repo" }, scans: null }), { status: 200 }),
    );
    await expect(new ServerBackend(null).repository("repo")).rejects.toThrow(
      "The server returned invalid repository details.",
    );
  });

  it("rejects malformed new scan responses", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ id: null, status: "running" }), { status: 202 }),
    );
    await expect(new ServerBackend(null).startScan("project")).rejects.toThrow(
      "The server returned an invalid new scan job.",
    );
  });

  it("rejects unknown scan job statuses", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ id: "job-1", status: "unexpected" }), { status: 200 }),
    );
    await expect(new ServerBackend(null).job("job-1")).rejects.toThrow(
      "The server returned an invalid scan job.",
    );
  });

  it("rejects incomplete nested progress data in scan jobs", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ ...validJob, warnings: [null] }), { status: 200 }),
    );
    await expect(new ServerBackend(null).job("job-1")).rejects.toThrow(
      "The server returned an invalid scan job.",
    );
  });

  it("rejects unsafe or imprecise scan progress counts", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ ...validJob, filesDone: -10 }), { status: 200 }),
    );
    await expect(new ServerBackend(null).job("job-1")).rejects.toThrow(
      "The server returned an invalid scan job.",
    );
  });

  it("accepts a valid running scan job response", async () => {
    const job = validJob;
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify(job), { status: 200 }),
    );
    await expect(new ServerBackend(null).job("job-1")).resolves.toEqual(job);
  });

  it("rejects HTML returned in place of a project DNA SVG card", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response("<html>Sign in</html>", { status: 200 }),
    );
    await expect(new ServerBackend(null).card("repo", false)).rejects.toThrow(
      "The server did not return an SVG card.",
    );
  });

  it("accepts an SVG project card response", async () => {
    const svg = '<svg xmlns="http://www.w3.org/2000/svg"></svg>';
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(svg, { status: 200 }));
    await expect(new ServerBackend(null).card("repo", false)).resolves.toBe(svg);
  });
});

describe("local server token URLs", () => {
  it("removes an empty token and its duplicates without discarding other route parameters", () => {
    window.history.replaceState(null, "", "/?token=&token=unused&view=activity#/files");
    expect(takeToken()).toBeNull();
    expect(window.location.search).toBe("?view=activity");
    expect(window.location.hash).toBe("#/files");
  });

  it("takes a token once, removes it from the URL and retains it for the tab", () => {
    window.history.replaceState(null, "", "/?token=secret-123&view=history#/overview");
    expect(takeToken()).toBe("secret-123");
    expect(window.location.search).toBe("?view=history");
    expect(window.location.hash).toBe("#/overview");
    expect(window.sessionStorage.getItem("repodna-token")).toBe("secret-123");
    expect(takeToken()).toBe("secret-123");
  });
});

describe("explicit native Git revision requests", () => {
  it("passes a slash-containing branch as a separate API field", async () => {
    const spy = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValue(new Response(JSON.stringify(validJob), { status: 202 }));
    await new ServerBackend("local-token").startScan(
      "https://github.com/sanskarIN/sonumio/tree/main",
      "standard",
      { kind: "branch", value: "feat/initial-audio-platform" },
    );
    const init = spy.mock.calls[0]?.[1];
    expect(JSON.parse(init?.body as string)).toEqual({
      input: "https://github.com/sanskarIN/sonumio/tree/main",
      profile: "standard",
      branch: "feat/initial-audio-platform",
    });
  });

  it("passes tags and commits distinctly, never as repository URLs", async () => {
    const spy = vi.spyOn(globalThis, "fetch");
    spy.mockResolvedValueOnce(new Response(JSON.stringify(validJob), { status: 202 }));
    spy.mockResolvedValueOnce(new Response(JSON.stringify(validJob), { status: 202 }));
    const backend = new ServerBackend(null);
    await backend.startScan("https://github.com/org/repo", undefined, {
      kind: "tag",
      value: "v1.5.0",
    });
    await backend.startScan("https://github.com/org/repo", undefined, {
      kind: "commit",
      value: "0123456789abcdef0123456789abcdef01234567",
    });
    expect(JSON.parse(spy.mock.calls[0]?.[1]?.body as string).tag).toBe("v1.5.0");
    expect(JSON.parse(spy.mock.calls[1]?.[1]?.body as string).commit).toBe(
      "0123456789abcdef0123456789abcdef01234567",
    );
  });
});

describe("remote reference listings", () => {
  const branch = { kind: "branch", name: "feat/audio", object: "a".repeat(40) };
  const tag = { kind: "tag", name: "feat/audio", object: "b".repeat(64) };

  it("preserves same-named branches and tags with separate identities", () => {
    expect(checkSourceReferences({ references: [branch, tag], truncated: true })).toEqual({
      references: [branch, tag],
      truncated: true,
    });
  });

  it("rejects malformed objects, duplicate namespace/name pairs and excessive results", () => {
    for (const value of [
      null,
      { references: [], truncated: "false" },
      { references: [{ ...branch, object: "short" }], truncated: false },
      { references: [{ ...branch, kind: "commit" }], truncated: false },
      { references: [{ ...branch, name: "--flag" }], truncated: false },
      { references: [{ ...branch, name: "a/../b" }], truncated: false },
      { references: [branch, branch], truncated: false },
      { references: Array(2001).fill(branch), truncated: true },
    ]) {
      expect(() => checkSourceReferences(value)).toThrow(/remote reference/i);
    }
  });

  it("uses an authenticated POST body and never puts the URL in the API path", async () => {
    const result = { references: [branch], truncated: false };
    const fetchSpy = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify(result), { status: 200 }),
    );
    const input = "https://github.com/org/repo/tree/feat/audio";
    const backend = new ServerBackend("local-token");
    await expect(backend.listRemoteReferences(input)).resolves.toEqual(result);
    const [url, init] = fetchSpy.mock.calls[0] ?? [];
    expect(url).toBe("/api/remote-references");
    expect(init?.method).toBe("POST");
    expect(JSON.parse(String(init?.body))).toEqual({ input });
    expect((init?.headers as Headers).get("X-RepoDNA-Token")).toBe("local-token");
  });
});

describe("native analysis recovery", () => {
  it("validates distinct job identities and rejects malformed or oversized lists", () => {
    const completed = { ...validJob, id: "job-2", status: "completed" };
    expect(checkScanJobs([validJob, completed])).toEqual([validJob, completed]);
    expect(checkScanJobs([])).toEqual([]);
    expect(() => checkScanJobs([validJob, validJob])).toThrow(/duplicate scan job IDs/);
    expect(() => checkScanJobs(Array(51).fill(validJob))).toThrow(/invalid scan job list/);
    const invalid = { ...validJob, status: "unknown" };
    expect(() => checkScanJobs([validJob, invalid])).toThrow(/invalid scan job/);
    expect(() => checkScanJob(validJob, "wrong-id")).toThrow(/invalid scan job/);
  });

  it("lists native jobs through the authenticated scans endpoint", async () => {
    const spy = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify([validJob]), { status: 200 }),
    );
    const backend = new ServerBackend("local-token");
    await expect(backend.listJobs()).resolves.toEqual([validJob]);
    const [url, init] = spy.mock.calls[0] ?? [];
    expect(url).toBe("/api/scans");
    expect((init?.headers as Headers).get("X-RepoDNA-Token")).toBe("local-token");
  });
});
