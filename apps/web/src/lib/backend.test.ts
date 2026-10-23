import { afterEach, describe, expect, it, vi } from "vitest";
import { ServerBackend, takeToken } from "./backend";

const originalUrl = window.location.href;

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

  it("allows an empty successful cancellation response", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(null, { status: 204 }));
    await expect(new ServerBackend(null).cancelJob("job-1")).resolves.toBeUndefined();
  });

  it("rejects repository details without an array of scans", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ repository: { id: "repo" }, scans: null }), { status: 200 }),
    );
    await expect(new ServerBackend(null).repository("repo")).rejects.toThrow(
      "The server returned invalid repository details.",
    );
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
