import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Backend, HistoryPage, ScanSummary } from "./backend";
import { useStoredHistory } from "./useStoredHistory";

const sample: ScanSummary = {
  id: "scan-1",
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

function page(repositoryId: string, ids: string[], nextCursor: string | null): HistoryPage {
  return { repositoryId, scans: ids.map((id) => ({ ...sample, id })), nextCursor };
}

function backend() {
  const history = vi.fn(async (id: string, _after?: string) => page(id, ["first"], "next"));
  const repository = vi.fn(async () => ({ scans: [sample] }));
  return { service: { history, repository } as unknown as Backend, history, repository };
}

function deferred<T>() {
  let resolve: (value: T) => void = () => undefined;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe("stored history lifecycle", () => {
  it("loads older pages, deduplicates records and guards concurrent requests", async () => {
    const { service, history } = backend();
    history.mockResolvedValueOnce(page("a", ["first", "second"], "next"));
    history.mockResolvedValueOnce(page("a", ["second", "third"], null));
    const { result } = renderHook(() => useStoredHistory(service));
    await act(async () => result.current.toggle("a"));
    await act(async () => {
      result.current.more();
      result.current.more();
    });
    expect(history.mock.calls).toEqual([
      ["a", undefined],
      ["a", "next"],
    ]);
    expect(result.current.scans?.map((scan) => scan.id)).toEqual(["first", "second", "third"]);
    expect(result.current.cursor).toBeNull();
    act(() => result.current.more());
    expect(history).toHaveBeenCalledTimes(2);
  });

  it("ignores a stale page from another repository", async () => {
    const { service, history } = backend();
    const delayed = deferred<HistoryPage>();
    history.mockReturnValueOnce(delayed.promise);
    const { result } = renderHook(() => useStoredHistory(service));
    act(() => result.current.toggle("old"));
    await act(async () => result.current.toggle("new"));
    await act(async () => delayed.resolve(page("old", ["stale"], null)));
    expect(result.current.id).toBe("new");
    expect(result.current.scans?.map((scan) => scan.id)).toEqual(["first"]);
  });

  it("retains the page and retries a failed older-page request", async () => {
    const { service, history } = backend();
    history.mockResolvedValueOnce(page("a", ["first"], "next"));
    history.mockRejectedValueOnce(new Error("offline"));
    history.mockResolvedValueOnce(page("a", ["second"], null));
    const { result } = renderHook(() => useStoredHistory(service));
    await act(async () => result.current.toggle("a"));
    await act(async () => result.current.more());
    expect(result.current.error).toBe("offline");
    expect(result.current.cursor).toBe("next");
    expect(result.current.scans?.map((scan) => scan.id)).toEqual(["first"]);
    await act(async () => result.current.more());
    expect(result.current.error).toBeNull();
    expect(result.current.scans?.map((scan) => scan.id)).toEqual(["first", "second"]);
  });

  it("resets the pagination after a delete or explicit refresh", async () => {
    const { service, history } = backend();
    const { result } = renderHook(() => useStoredHistory(service));
    await act(async () => result.current.toggle("a"));
    history.mockResolvedValueOnce(page("a", ["replacement"], null));
    await act(async () => result.current.refresh("a"));
    expect(history).toHaveBeenLastCalledWith("a", undefined);
    expect(result.current.scans?.map((scan) => scan.id)).toEqual(["replacement"]);
    act(() => result.current.toggle("a"));
    expect(result.current.id).toBeNull();
  });

  it("supports a legacy backend without the paginated endpoint", async () => {
    const repository = vi.fn(async () => ({ scans: [sample] }));
    const service = { repository } as unknown as Backend;
    const { result } = renderHook(() => useStoredHistory(service));
    await act(async () => result.current.toggle("legacy"));
    expect(repository).toHaveBeenCalledWith("legacy");
    expect(result.current.scans).toEqual([sample]);
    expect(result.current.cursor).toBeNull();
  });
});
