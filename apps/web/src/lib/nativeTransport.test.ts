import { describe, expect, it } from "vitest";
import { Clock, deferred, flush } from "./controllerTestSupport";
import { decodeNativeJson, NativeTransportError, readBoundedText, requestNativeText } from "./nativeTransport";

async function rejected<T>(promise: Promise<T>): Promise<Error> {
  try { await promise; } catch (reason) { if (reason instanceof Error) return reason; throw reason; }
  throw new Error("Expected rejection");
}
function failure(callback: () => unknown): Error {
  try { callback(); } catch (reason) { if (reason instanceof Error) return reason; throw reason; }
  throw new Error("Expected failure");
}
function streaming(chunks: Uint8Array[], cancelled?: () => void): Response {
  let index = 0;
  return new Response(new ReadableStream<Uint8Array>({
    pull(controller) {
      const chunk = chunks[index++];
      if (chunk) controller.enqueue(chunk);
      else controller.close();
    },
    cancel() { cancelled?.(); },
  }));
}
const encoder = new TextEncoder();
const path = "/api/session";
const init = { headers: { "X-RepoDNA-Token": "local-secret" } };
const options = (fetcher: typeof fetch, clock = new Clock()) => ({ fetcher, clock, deadlineMs: 100, maxBytes: 1024 });

describe("bounded native HTTP body reader", () => {
  it("decodes UTF-8 codepoints split across arbitrary chunk boundaries", async () => {
    const value = '{"name":"日本語😀"}';
    const bytes = encoder.encode(value);
    const chunks = Array.from(bytes, (byte) => new Uint8Array([byte]));
    expect(await readBoundedText(streaming(chunks), bytes.length)).toBe(value);
  });

  it("allows exactly the size limit including multibyte characters", async () => {
    expect(await readBoundedText(new Response("é"), 2)).toBe("é");
    expect((await rejected(readBoundedText(new Response("é"), 1))).message).toContain("exceeds");
  });

  it("rejects a declared excessive Content-Length before reading body chunks", async () => {
    let cancelled = false;
    const response = new Response(new ReadableStream({ cancel() { cancelled = true; } }), { headers: { "Content-Length": "100000" } });
    expect((await rejected(readBoundedText(response, 32))).message).toContain("exceeds");
    await flush();
    expect(cancelled).toBe(true);
  });

  it("enforces the actual streaming budget despite a misleading length header", async () => {
    const response = new Response("abcdef", { headers: { "Content-Length": "1" } });
    expect((await rejected(readBoundedText(response, 5))).message).toContain("exceeds");
  });

  it("does not treat compressed wire length as decoded body length", async () => {
    const response = new Response("short", { headers: { "Content-Length": "1000000", "Content-Encoding": "gzip" } });
    expect(await readBoundedText(response, 5)).toBe("short");
  });

  it("treats a malformed content length as unknown rather than bypassing the stream cap", async () => {
    for (const length of ["abc", "1.5", "-2", "1, 2"]) {
      const response = new Response("abcdef", { headers: { "Content-Length": length } });
      expect((await rejected(readBoundedText(response, 5))).message).toContain("exceeds");
    }
  });

  it("cancels the underlying reader on overflow", async () => {
    let cancelled = 0;
    const response = streaming([encoder.encode("aaa"), encoder.encode("bbb"), encoder.encode("ccc")], () => { cancelled++; });
    expect((await rejected(readBoundedText(response, 4))).message).toContain("exceeds");
    expect(cancelled).toBe(1);
    expect(response.body?.locked).toBe(false);
  });

  it("rejects malformed UTF-8 rather than silently substituting characters", async () => {
    const response = streaming([new Uint8Array([0xc3]), new Uint8Array([0x28])]);
    expect((await rejected(readBoundedText(response, 10))).message).toContain("UTF-8");
    expect(response.body?.locked).toBe(false);
  });

  it("rejects incomplete multibyte sequences at end-of-stream", async () => {
    expect((await rejected(readBoundedText(streaming([new Uint8Array([0xf0, 0x9f])]), 10))).message).toContain("UTF-8");
  });

  it("handles empty and bodyless HTTP responses", async () => {
    expect(await readBoundedText(new Response(null, { status: 204 }), 10)).toBe("");
    expect(await readBoundedText(new Response(""), 10)).toBe("");
  });
});

describe("authenticated local API transport", () => {
  it("forces same-origin credentials, no-store and redirect rejection", async () => {
    let captured: RequestInit | undefined;
    let requested: unknown;
    const fetcher: typeof fetch = async (input, request) => { requested = input; captured = request; return new Response('{"ok":true}'); };
    const clock = new Clock();
    const response = await requestNativeText(path, { ...init, redirect: "follow", cache: "force-cache", credentials: "include" }, options(fetcher, clock));
    expect(requested).toBe(path);
    expect(captured?.redirect).toBe("error");
    expect(captured?.credentials).toBe("same-origin");
    expect(captured?.cache).toBe("no-store");
    expect(captured?.headers).toEqual(init.headers);
    expect(response).toEqual({ status: 200, ok: true, text: '{"ok":true}' });
    expect(clock.pending).toBe(0);
  });

  it("rejects non-local or malformed paths without calling fetch", async () => {
    let calls = 0;
    const fetcher: typeof fetch = async () => { calls++; return new Response("{}"); };
    for (const value of ["https://example.invalid/api", "//example.invalid/api", "/other", "/api/\\remote", "/api/a\n"]) {
      expect((await rejected(requestNativeText(value, init, options(fetcher)))).message).toContain("Invalid local API");
    }
    expect(calls).toBe(0);
  });

  it("does not follow or accept a redirected response even from a custom fetch implementation", async () => {
    const response = new Response("{}", { status: 200 });
    Object.defineProperty(response, "redirected", { value: true });
    const error = await rejected(requestNativeText(path, init, options(async () => response)));
    expect(error.message).toContain("unexpectedly redirected");
  });

  it("aborts at the deadline even when fetching headers never resolves", async () => {
    const clock = new Clock();
    const pending = deferred<Response>();
    let signal: AbortSignal | null | undefined;
    const fetcher: typeof fetch = async (_input, request) => { signal = request?.signal; return pending.promise; };
    const task = rejected(requestNativeText(path, init, options(fetcher, clock)));
    await clock.advance(100);
    expect((await task).message).toContain("did not respond");
    expect(signal?.aborted).toBe(true);
    expect(clock.pending).toBe(0);
    pending.resolve(new Response("late"));
    await flush();
  });

  it("covers streaming body reads with the same deadline", async () => {
    const clock = new Clock();
    const body = new ReadableStream<Uint8Array>({ start(controller) { controller.enqueue(encoder.encode("{")); } });
    let signal: AbortSignal | null | undefined;
    const fetcher: typeof fetch = async (_input, request) => { signal = request?.signal; return new Response(body); };
    const task = rejected(requestNativeText(path, init, options(fetcher, clock)));
    await flush();
    await clock.advance(100);
    const error = await task;
    expect(error.message).toContain("did not respond");
    expect((error as NativeTransportError).status).toBe(200);
    expect(signal?.aborted).toBe(true);
  });

  it("supports caller cancellation before fetch starts", async () => {
    const controller = new AbortController();
    controller.abort();
    let calls = 0;
    const error = await rejected(requestNativeText(path, { signal: controller.signal }, options(async () => { calls++; return new Response("{}"); })));
    expect(error.message).toContain("cancelled");
    expect(calls).toBe(0);
  });

  it("supports caller cancellation while waiting for headers and consumes late rejection", async () => {
    const controller = new AbortController();
    const clock = new Clock();
    const pending = deferred<Response>();
    const task = rejected(requestNativeText(path, { signal: controller.signal }, options(async () => pending.promise, clock)));
    controller.abort();
    expect((await task).message).toContain("cancelled");
    expect(clock.pending).toBe(0);
    pending.reject(new Error("late failure"));
    await flush();
  });

  it("preserves HTTP failure status when a proxy error body exceeds its smaller limit", async () => {
    const response = await requestNativeText(path, init, { ...options(async () => new Response("x".repeat(100_000), { status: 401 })), maxBytes: 200_000 });
    expect(response.status).toBe(401);
    expect(response.ok).toBe(false);
    expect(response.text).toBe("");
    expect((failure(() => decodeNativeJson(response)) as NativeTransportError).status).toBe(401);
  });

  it("normalizes synchronous and asynchronous fetch failures and clears deadlines", async () => {
    const clock = new Clock();
    for (const fetcher of [() => { throw new Error("network blocked"); }, async () => { throw new Error("network blocked"); }]) {
      const reason = await rejected(requestNativeText(path, init, options(fetcher as typeof fetch, clock)));
      expect(reason.message).toContain("network blocked");
      expect((reason as NativeTransportError).status).toBe(0);
      expect(clock.pending).toBe(0);
    }
  });
});

describe("native API JSON decoding", () => {
  it("accepts well-formed JSON values without coercing their types", () => {
    expect(decodeNativeJson({ ok: true, status: 200, text: '{"items":[]}' })).toEqual({ items: [] });
    expect(decodeNativeJson({ ok: true, status: 200, text: "false" })).toBe(false);
  });

  it("does not mistake a successful HTML proxy page for API data", () => {
    const reason = failure(() => decodeNativeJson({ ok: true, status: 200, text: "<html>" }));
    expect(reason.message).toContain("invalid JSON");
  });

  it("rejects null and empty JSON except an explicitly allowed 204", () => {
    for (const text of ["", "null", " "]) expect(failure(() => decodeNativeJson({ ok: true, status: 200, text })).message).toContain("invalid JSON");
    expect(decodeNativeJson({ ok: true, status: 204, text: "" }, true)).toBe(undefined);
    expect(failure(() => decodeNativeJson({ ok: true, status: 204, text: "null" }, true)).message).toContain("invalid JSON");
  });

  it("preserves a bounded string API error and its HTTP status", () => {
    const reason = failure(() => decodeNativeJson({ ok: false, status: 401, text: '{"error":"Access denied"}' }));
    expect(reason.message).toBe("Access denied");
    expect((reason as NativeTransportError).status).toBe(401);
    const long = failure(() => decodeNativeJson({ ok: false, status: 500, text: JSON.stringify({ error: "x".repeat(5000) }) }));
    expect(long.message.length).toBe(4096);
  });

  it("does not stringify objects or echo raw proxy HTML in errors", () => {
    for (const text of ["<html>secret</html>", '{"error":{"secret":"token"}}', '{"error":["secret"]}']) {
      expect(failure(() => decodeNativeJson({ ok: false, status: 502, text })).message).toBe("The server answered 502.");
    }
  });
});
