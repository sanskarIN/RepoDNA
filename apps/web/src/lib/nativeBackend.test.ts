import { describe, expect, it } from "vitest";
import { BackendError, ServerBackend, checkNativeArtifact, checkRepositories, checkRepository, checkSession } from "./backend";
import { DesktopBackend } from "./desktop";
import { Clock } from "./controllerTestSupport";

const repository = {
  id: "repository-1", name: "project", location: "/project", kind: "local", scans: 1,
  firstScannedAt: "2026-01-01T00:00:00Z", lastScannedAt: "2026-01-01T00:00:00Z",
};
const scan = {
  id: "scan-1", generatedAt: "2026-01-01T00:00:00Z", profile: "standard", dnaHash: "hash",
  toolVersion: "1.5.0", files: 4, codeLines: 10, commits: 2, contributors: 1,
  findings: { critical: 0, warning: 1, attention: 0, info: 0, suppressed: 0 },
};
const job = {
  id: "job-1", input: "/project", status: "running", stages: [], filesDone: 0,
  filesTotal: 0, warnings: [], startedAt: "2026-01-01T00:00:00Z",
};
const artifact = (id: string) => ({
  schemaVersion: "1.0", tool: {}, identity: {}, analysisMetadata: { id },
});

function http(...responses: Response[]) {
  const clock = new Clock();
  const requests: { url: string; init?: RequestInit }[] = [];
  const fetcher: typeof fetch = async (input, init) => {
    requests.push({ url: String(input), init });
    const response = responses.shift();
    if (!response) throw new Error("No fixture response");
    return response;
  };
  return { backend: new ServerBackend("test-token", { fetcher, clock }), requests, clock };
}
function json(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), { status });
}
function ipc(...responses: unknown[]) {
  const calls: { command: string; args: unknown }[] = [];
  const invoke = async <T>(command: string, args?: unknown): Promise<T> => {
    calls.push({ command, args });
    if (!responses.length) throw new Error("No fixture response");
    return responses.shift() as T;
  };
  return { backend: new DesktopBackend(invoke), calls };
}
async function failure(operation: Promise<unknown>, message: string, status = 200) {
  let error: unknown;
  try { await operation; } catch (reason) { error = reason; }
  expect(error).toBeInstanceOf(BackendError);
  if (!(error instanceof BackendError)) throw new Error("Expected backend error");
  expect(error.message).toContain(message);
  expect(error.status).toBe(status);
}
function invalid(operation: () => unknown) {
  let error: unknown;
  try { operation(); } catch (reason) { error = reason; }
  expect(error).toBeInstanceOf(BackendError);
}

describe("shared native response contracts", () => {
  it("rejects duplicate repositories before rendering duplicate rows", () => {
    invalid(() => checkRepositories([repository, repository]));
    expect(checkRepositories([repository])).toEqual([repository]);
  });
  it("rejects a different repository identity", () => {
    invalid(() => checkRepository({ repository, scans: [scan] }, "other"));
    expect(checkRepository({ repository, scans: [scan] }, repository.id).scans).toEqual([scan]);
  });
  it("rejects duplicated scan identities", () => {
    invalid(() => checkRepository({ repository, scans: [scan, scan] }, repository.id));
  });
  it("rejects oversized detail pages", () => {
    const scans = Array.from({ length: 101 }, (_, index) => ({ ...scan, id: String(index) }));
    invalid(() => checkRepository({ repository, scans }, repository.id));
  });
  it("accepts exact artifact identity and rejects a mismatch", () => {
    expect(checkNativeArtifact(artifact("scan-1"), "scan-1").analysisMetadata.id).toBe("scan-1");
    invalid(() => checkNativeArtifact(artifact("scan-2"), "scan-1"));
  });
  it("preserves legacy artifacts without an embedded scan identity", () => {
    expect(checkNativeArtifact(artifact(""), "scan-1").analysisMetadata.id).toBe("");
  });
  it("requires a meaningful session version and boolean permission", () => {
    invalid(() => checkSession({ version: "", allowScans: false }));
    invalid(() => checkSession({ version: "1.0", allowScans: "false" }));
    expect(checkSession({ version: "1.0", allowScans: false })).toEqual({ version: "1.0", allowScans: false });
  });
});

describe("native HTTP backend", () => {
  it("encodes repository identifiers and keeps authentication in headers", async () => {
    const { backend, requests, clock } = http(json({ repository: { ...repository, id: "repo /1" }, scans: [scan] }));
    await backend.repository("repo /1");
    expect(requests[0]?.url).toBe("/api/repositories/repo%20%2F1");
    expect(new Headers(requests[0]?.init?.headers).get("X-RepoDNA-Token")).toBe("test-token");
    expect(requests[0]?.init?.redirect).toBe("error");
    expect(clock.pending).toBe(0);
  });
  it("preserves status and local server error messages", async () => {
    const { backend } = http(json({ error: "Access denied" }, 401));
    await failure(backend.session(), "Access denied", 401);
  });
  it("rejects a successful HTML error page", async () => {
    const { backend } = http(new Response("<html></html>"));
    await failure(backend.session(), "invalid JSON");
  });
  it("requires exact job identity", async () => {
    const { backend } = http(json(job));
    await failure(backend.job("different"), "invalid scan job");
  });
  it("accepts empty HTTP deletion acknowledgements", async () => {
    const { backend, requests } = http(new Response(null, { status: 204 }));
    await backend.deleteScan("repo /1", "scan /2");
    expect(requests[0]?.url).toBe("/api/repositories/repo%20%2F1/scans/scan%20%2F2");
    expect(requests[0]?.init?.method).toBe("DELETE");
  });
  it("rejects empty JSON from a read endpoint", async () => {
    const { backend } = http(new Response(null, { status: 204 }));
    await failure(backend.repositories(), "invalid JSON", 204);
  });
  it("encodes cursor and repository identity independently", async () => {
    const { backend, requests } = http(json({ repositoryId: "repo /1", scans: [scan], nextCursor: null }));
    await backend.history("repo /1", "x:y+z");
    expect(requests[0]?.url).toBe("/api/repositories/repo%20%2F1/history?limit=50&after=x%3Ay%2Bz");
  });
  it("sends explicit tag selection without mixing it into the URL", async () => {
    const { backend, requests } = http(json(job, 202));
    await backend.startScan("https://example.com/project", "deep", { kind: "tag", value: "release/v1" });
    expect(JSON.parse(String(requests[0]?.init?.body))).toEqual({ input: "https://example.com/project", profile: "deep", tag: "release/v1" });
  });
  it("rejects a terminal response to a new scan request", async () => {
    const { backend } = http(json({ ...job, status: "completed" }));
    await failure(backend.startScan("/project"), "invalid new scan job");
  });
  it("requires SVG card markup", async () => {
    const { backend } = http(new Response("<html></html>"));
    await failure(backend.card("repo", false), "SVG card");
  });
  it("returns the correctly requested artifact", async () => {
    const { backend, requests } = http(json(artifact("scan-1")));
    expect((await backend.artifact("repo", "scan-1")).analysisMetadata.id).toBe("scan-1");
    expect(requests[0]?.url).toBe("/api/repositories/repo/artifact?scan=scan-1");
  });
});

describe("native desktop IPC backend", () => {
  it("applies the same session validation as HTTP", async () => {
    const { backend } = ipc({ version: "1.0", allowScans: "true" });
    await failure(backend.session(), "invalid session");
  });
  it("rejects duplicate repository entries", async () => {
    const { backend } = ipc([repository, repository]);
    await failure(backend.repositories(), "invalid repository list");
  });
  it("rejects a repository detail response for a different ID", async () => {
    const { backend } = ipc({ repository, scans: [scan] });
    await failure(backend.repository("other"), "invalid repository details");
  });
  it("does not accept an artifact returned for a different scan", async () => {
    const { backend } = ipc(artifact("wrong"));
    await failure(backend.artifact("repo", "scan-1"), "different stored analysis");
  });
  it("uses separate revision parameters for IPC", async () => {
    const { backend, calls } = ipc(job);
    await backend.startScan("https://example.com/project", "deep", { kind: "branch", value: "feat/a" });
    expect(calls).toEqual([{ command: "start_scan", args: {
      input: "https://example.com/project", profile: "deep", branch: "feat/a", tag: null, commit: null,
    } }]);
  });
  it("rejects a terminal initial scan response", async () => {
    const { backend } = ipc({ ...job, status: "failed" });
    await failure(backend.startScan("/project"), "invalid new scan job");
  });
  it("rejects unexpected job identity", async () => {
    const { backend } = ipc(job);
    await failure(backend.job("wrong"), "invalid scan job");
  });
  it("validates path dialog results and preserves explicit cancellation", async () => {
    const { backend } = ipc(null, "/tmp/report", "", false);
    expect(await backend.pickDirectory()).toBeNull();
    expect(await backend.saveReport("repo", "html")).toBe("/tmp/report");
    await failure(backend.saveCard("repo", false, true), "invalid file dialog");
    await failure(backend.pickDirectory(), "invalid file dialog");
  });
  it("rejects non-SVG cards", async () => {
    const { backend } = ipc({ svg: "<svg/>" });
    await failure(backend.card("repo", false), "SVG card");
  });
  it("preserves exact history arguments and cancellation commands", async () => {
    const { backend, calls } = ipc({ repositoryId: "repo", scans: [scan], nextCursor: null }, undefined);
    await backend.history("repo", "cursor");
    await backend.cancelJob("job-1");
    expect(calls).toEqual([
      { command: "repository_history", args: { id: "repo", limit: 50, after: "cursor" } },
      { command: "cancel_job", args: { id: "job-1" } },
    ]);
  });
});
