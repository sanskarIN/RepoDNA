import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { JobState } from "./backend";
import { useScanJob } from "./useScanJob";

function job(status: JobState["status"] = "running", id = "job-1"): JobState {
  return {
    id,
    input: "/project",
    status,
    stages: [],
    filesDone: 0,
    filesTotal: 0,
    warnings: [],
    startedAt: "2026-11-28T00:00:00Z",
  };
}

function deferred<T>() {
  let resolve: (value: T) => void = () => undefined;
  let reject: (reason: Error) => void = () => undefined;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function backend() {
  return {
    startScan: vi.fn(async () => job()),
    job: vi.fn(async (_id: string) => job()),
    cancelJob: vi.fn(async (_id: string) => undefined),
  };
}

async function advance(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

describe("native analysis lifecycle", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("blocks concurrent starts before a native job ID arrives", async () => {
    const service = backend();
    const pending = deferred<JobState>();
    service.startScan.mockReturnValueOnce(pending.promise);
    const { result, unmount } = renderHook(() => useScanJob(service, vi.fn()));
    act(() => {
      void result.current.start("/project", "standard");
      void result.current.start("/other", "quick");
    });
    expect(service.startScan).toHaveBeenCalledTimes(1);
    expect(result.current.starting).toBe(true);
    expect(result.current.busy).toBe(true);
    await act(async () => pending.resolve(job()));
    expect(result.current.starting).toBe(false);
    expect(result.current.busy).toBe(true);
    unmount();
  });

  it("retries failed progress checks then resumes the same job on request", async () => {
    const service = backend();
    const done = vi.fn();
    service.job
      .mockRejectedValueOnce(new Error("offline"))
      .mockRejectedValueOnce(new Error("offline"))
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce({
        ...job("completed"),
        repositoryId: "repo-1",
        scanId: "scan-1",
      });
    const { result, unmount } = renderHook(() => useScanJob(service, done));
    await act(async () => result.current.start("/project", "standard"));
    await advance(400);
    expect(result.current.pollError).toBe("offline");
    await advance(800);
    await advance(1600);
    expect(result.current.paused).toBe(true);
    expect(result.current.busy).toBe(true);
    await advance(10_000);
    expect(service.job).toHaveBeenCalledTimes(3);
    act(() => result.current.resume());
    await advance(1);
    expect(service.job).toHaveBeenCalledTimes(4);
    expect(service.job.mock.calls.every(([id]) => id === "job-1")).toBe(true);
    expect(done).toHaveBeenCalledWith("repo-1", "scan-1");
    expect(result.current.busy).toBe(false);
    expect(result.current.pollError).toBeNull();
    unmount();
  });

  it("waits for a terminal cancellation and allows retry after a rejected request", async () => {
    const service = backend();
    service.cancelJob.mockRejectedValueOnce(new Error("connection lost"));
    const { result, unmount } = renderHook(() => useScanJob(service, vi.fn()));
    await act(async () => result.current.start("/project", "quick"));
    await act(async () => result.current.stop());
    expect(result.current.cancelling).toBe(false);
    expect(result.current.error).toBe("connection lost");
    await act(async () => result.current.stop());
    expect(result.current.cancelling).toBe(true);
    expect(result.current.job?.status).toBe("running");
    expect(service.cancelJob).toHaveBeenCalledTimes(2);
    service.job.mockResolvedValueOnce(job("cancelled"));
    await advance(400);
    expect(result.current.job?.status).toBe("cancelled");
    expect(result.current.cancelling).toBe(false);
    expect(result.current.busy).toBe(false);
    unmount();
  });

  it("does not navigate or schedule more polls after the view unmounts", async () => {
    const service = backend();
    const pending = deferred<JobState>();
    const done = vi.fn();
    service.job.mockReturnValueOnce(pending.promise);
    const { result, unmount } = renderHook(() => useScanJob(service, done));
    await act(async () => result.current.start("/project", "quick"));
    await advance(400);
    unmount();
    await act(async () => {
      pending.resolve({ ...job("completed"), repositoryId: "repo-1" });
    });
    await advance(10_000);
    expect(done).not.toHaveBeenCalled();
    expect(service.job).toHaveBeenCalledTimes(1);
  });

  it("reattaches to a live native job without submitting another analysis", async () => {
    const service = backend();
    const done = vi.fn();
    service.job
      .mockResolvedValueOnce(job("running", "job-existing"))
      .mockResolvedValueOnce({
        ...job("completed", "job-existing"),
        repositoryId: "saved-repo",
        scanId: "saved-scan",
      });
    const { result, unmount } = renderHook(() => useScanJob(service, done));
    await act(async () => result.current.attach("job-existing"));
    expect(service.startScan).not.toHaveBeenCalled();
    expect(result.current.job?.id).toBe("job-existing");
    expect(result.current.busy).toBe(true);
    await advance(400);
    expect(done).toHaveBeenCalledWith("saved-repo", "saved-scan");
    expect(service.job.mock.calls.every(([id]) => id === "job-existing")).toBe(true);
    unmount();
  });

  it("rejects a stale recovery entry with a mismatched job identity", async () => {
    const service = backend();
    service.job.mockResolvedValueOnce(job("running", "wrong-job"));
    const { result, unmount } = renderHook(() => useScanJob(service, vi.fn()));
    await act(async () => result.current.attach("job-existing"));
    expect(result.current.error).toContain("different analysis job");
    expect(result.current.busy).toBe(false);
    expect(result.current.job).toBeNull();
    unmount();
  });

  it("clears a failed start and permits another submission", async () => {
    const service = backend();
    service.startScan.mockRejectedValueOnce(new Error("native backend unavailable"));
    const { result, unmount } = renderHook(() => useScanJob(service, vi.fn()));
    await act(async () => result.current.start("/project", "standard"));
    expect(result.current.error).toBe("native backend unavailable");
    expect(result.current.busy).toBe(false);
    await act(async () => result.current.start("/project", "standard"));
    expect(result.current.error).toBeNull();
    expect(result.current.busy).toBe(true);
    unmount();
  });

  it("does not navigate for mismatched job IDs or a completed scan without storage", async () => {
    const service = backend();
    const done = vi.fn();
    service.job
      .mockResolvedValueOnce({ ...job("completed", "job-other"), repositoryId: "wrong" })
      .mockResolvedValueOnce(job("completed"));
    const { result, unmount } = renderHook(() => useScanJob(service, done));
    await act(async () => result.current.start("/project", "standard"));
    await advance(400);
    expect(result.current.pollError).toContain("different analysis job");
    expect(done).not.toHaveBeenCalled();
    await advance(800);
    expect(result.current.error).toContain("no saved result");
    expect(result.current.busy).toBe(false);
    expect(done).not.toHaveBeenCalled();
    unmount();
  });
});
