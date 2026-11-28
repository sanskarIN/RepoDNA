import { describe, expect, it } from "vitest";
import { Clock, deferred, flush } from "../lib/controllerTestSupport";
import { EngineCatalog, isInfo, MAX_ENGINE_MANIFEST_BYTES } from "./engineCatalog";

const BASE = "https://example.test/RepoDNA/";
const INFO = { version: "1.5.5", bytes: 1024 };

function manifest(value: unknown = INFO) {
  return new Response(JSON.stringify(value), { headers: { "Content-Type": "application/json" } });
}

function setup() {
  const clock = new Clock();
  const calls: { url: string; init?: RequestInit; response: ReturnType<typeof deferred<Response>> }[] = [];
  const fetcher: typeof fetch = (input, init) => {
    const response = deferred<Response>();
    calls.push({ url: String(input), init, response });
    return response.promise;
  };
  return { clock, calls, catalog: new EngineCatalog({ clock, fetcher, deadlineMs: 100 }) };
}

describe("engine manifest discovery", () => {
  it("deduplicates concurrent callers and caches a successful version", async () => {
    const { catalog, calls, clock } = setup();
    const first = catalog.load(BASE);
    const second = catalog.load(BASE);
    expect(first).toBe(second);
    expect(calls.length).toBe(1);
    expect(calls[0]?.url).toBe(`${BASE}engine/engine.json`);
    calls[0]?.response.resolve(manifest());
    expect(await first).toEqual(INFO);
    expect(await catalog.load(BASE)).toEqual(INFO);
    expect(calls.length).toBe(1);
    expect(clock.pending).toBe(0);
    expect(catalog.wasmUrl(BASE)).toBe(`${BASE}engine/repodna.wasm?v=1.5.5`);
  });

  it("requests same-origin metadata without allowing redirects", async () => {
    const { catalog, calls } = setup();
    const pending = catalog.load(BASE);
    expect(calls[0]?.init?.credentials).toBe("same-origin");
    expect(calls[0]?.init?.redirect).toBe("error");
    expect(calls[0]?.init?.cache).toBe("no-cache");
    calls[0]?.response.resolve(manifest());
    await pending;
  });

  it("copies only the version and size and protects the cache from mutation", async () => {
    const { catalog, calls } = setup();
    const pending = catalog.load(BASE);
    calls[0]?.response.resolve(manifest({ ...INFO, download: "https://other.test" }));
    const value = await pending;
    expect(value).toEqual(INFO);
    if (value) value.version = "9.9.9";
    expect(await catalog.load(BASE)).toEqual(INFO);
    expect(catalog.wasmUrl(BASE)).toBe(`${BASE}engine/repodna.wasm?v=1.5.5`);
  });

  it("retries after the static interface does not have an engine", async () => {
    const { catalog, calls } = setup();
    const first = catalog.load(BASE);
    calls[0]?.response.resolve(new Response("Not found", { status: 404 }));
    expect(await first).toBeNull();
    const second = catalog.load(BASE);
    calls[1]?.response.resolve(manifest());
    expect(await second).toEqual(INFO);
  });

  it("retries after network rejection", async () => {
    const { catalog, calls } = setup();
    const first = catalog.load(BASE);
    calls[0]?.response.reject(new Error("offline"));
    expect(await first).toBeNull();
    const second = catalog.load(BASE);
    calls[1]?.response.resolve(manifest());
    expect(await second).toEqual(INFO);
  });

  it("retries malformed JSON rather than caching a permanent failure", async () => {
    const { catalog, calls } = setup();
    const first = catalog.load(BASE);
    calls[0]?.response.resolve(new Response("<html>Proxy error</html>"));
    expect(await first).toBeNull();
    const second = catalog.load(BASE);
    calls[1]?.response.resolve(manifest());
    expect(await second).toEqual(INFO);
  });

  it("rejects malformed metadata without using its version for a WASM URL", async () => {
    const { catalog, calls } = setup();
    const pending = catalog.load(BASE);
    calls[0]?.response.resolve(manifest({ version: "../../other", bytes: 123 }));
    expect(await pending).toBeNull();
    expect(catalog.wasmUrl(BASE)).toBe(`${BASE}engine/repodna.wasm?v=`);
  });

  it("aborts a stalled manifest and ignores the old response after retry", async () => {
    const { catalog, clock, calls } = setup();
    const first = catalog.load(BASE);
    await clock.advance(100);
    expect(await first).toBeNull();
    expect(calls[0]?.init?.signal?.aborted).toBe(true);
    const second = catalog.load(BASE);
    calls[0]?.response.resolve(manifest({ version: "1.0.0", bytes: 12 }));
    calls[1]?.response.resolve(manifest());
    expect(await second).toEqual(INFO);
    expect(await catalog.load(BASE)).toEqual(INFO);
  });

  it("times out a response whose body stops producing data", async () => {
    const { catalog, calls, clock } = setup();
    let cancelled = false;
    const body = new ReadableStream<Uint8Array>({ cancel() { cancelled = true; } });
    const pending = catalog.load(BASE);
    calls[0]?.response.resolve(new Response(body));
    await flush();
    await clock.advance(100);
    expect(await pending).toBeNull();
    expect(cancelled).toBe(true);
    expect(clock.pending).toBe(0);
  });

  it("bounds actual decoded bytes even without Content-Length", async () => {
    const { catalog, calls } = setup();
    const pending = catalog.load(BASE);
    calls[0]?.response.resolve(new Response(new Uint8Array(MAX_ENGINE_MANIFEST_BYTES + 1)));
    expect(await pending).toBeNull();
  });

  it("rejects invalid UTF-8", async () => {
    const { catalog, calls } = setup();
    const pending = catalog.load(BASE);
    calls[0]?.response.resolve(new Response(new Uint8Array([0xff, 0xfe])));
    expect(await pending).toBeNull();
  });

  it("rejects a redirect even if a custom fetcher followed it", async () => {
    const { catalog, calls } = setup();
    const pending = catalog.load(BASE);
    const response = manifest();
    Object.defineProperty(response, "redirected", { value: true });
    calls[0]?.response.resolve(response);
    expect(await pending).toBeNull();
  });

  it("does not retain the previous engine version at a different deployment root", async () => {
    const { catalog, calls } = setup();
    const first = catalog.load(BASE);
    calls[0]?.response.resolve(manifest());
    await first;
    const nextBase = "https://example.test/preview/";
    expect(catalog.wasmUrl(nextBase)).toBe(`${nextBase}engine/repodna.wasm?v=`);
    const second = catalog.load(nextBase);
    calls[1]?.response.resolve(manifest({ version: "2.0.0-rc.1", bytes: 11 }));
    expect(await second).toEqual({ version: "2.0.0-rc.1", bytes: 11 });
    expect(catalog.wasmUrl(nextBase)).toBe(`${nextBase}engine/repodna.wasm?v=2.0.0-rc.1`);
  });

  it("abandons in-flight work on a deployment-root change", async () => {
    const { catalog, calls } = setup();
    const first = catalog.load(BASE);
    const second = catalog.load("https://example.test/other/");
    expect(await first).toBeNull();
    expect(calls[0]?.init?.signal?.aborted).toBe(true);
    calls[0]?.response.resolve(manifest({ version: "8.0.0", bytes: 123 }));
    calls[1]?.response.resolve(manifest());
    expect(await second).toEqual(INFO);
  });

  it("clears pending work and the version on reset", async () => {
    const { catalog, calls } = setup();
    const first = catalog.load(BASE);
    catalog.reset();
    expect(await first).toBeNull();
    expect(calls[0]?.init?.signal?.aborted).toBe(true);
    const second = catalog.load(BASE);
    calls[1]?.response.resolve(manifest());
    await second;
    catalog.reset();
    expect(catalog.wasmUrl(BASE)).toBe(`${BASE}engine/repodna.wasm?v=`);
  });

  it("does not issue a request for an unsupported engine base", async () => {
    const { catalog, calls } = setup();
    expect(await catalog.load("file:///tmp/index.html")).toBeNull();
    expect(await catalog.load("https://user:password@example.test/")).toBeNull();
    expect(calls.length).toBe(0);
  });

  it("retains the existing engine metadata format while bounding version length", () => {
    expect(isInfo({ version: "1.2.3-beta.4", bytes: 123 })).toBe(true);
    for (const invalid of [null, [], { ...INFO, bytes: 0 }, { ...INFO, bytes: 1.5 }, { ...INFO, bytes: Infinity }, { ...INFO, version: `1.2.3-${"a".repeat(129)}` }]) {
      expect(isInfo(invalid)).toBe(false);
    }
  });
});
