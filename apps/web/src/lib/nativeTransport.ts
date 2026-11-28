import { browserClock, errorMessage, type RequestClock } from "./requestDeadline";

export class NativeTransportError extends Error {
  override name = "NativeTransportError";
  constructor(message: string, readonly status: number) {
    super(message);
  }
}

export interface NativeTransportOptions {
  /** Maximum decoded HTTP body size in UTF-8 bytes. */
  maxBytes: number;
  /** Includes both waiting for headers and consuming the response body. */
  deadlineMs: number;
  clock?: RequestClock;
  fetcher?: typeof fetch;
}

export interface NativeTextResponse {
  status: number;
  ok: boolean;
  text: string;
}

/** Small endpoints must not inherit an artifact's much larger response budget. */
export const NATIVE_LIMITS = {
  small: { maxBytes: 1 * 1024 * 1024, deadlineMs: 15_000 },
  list: { maxBytes: 32 * 1024 * 1024, deadlineMs: 30_000 },
  artifact: { maxBytes: 512 * 1024 * 1024, deadlineMs: 120_000 },
  discovery: { maxBytes: 40 * 1024 * 1024, deadlineMs: 25_000 },
  card: { maxBytes: 16 * 1024 * 1024, deadlineMs: 60_000 },
} as const;
const MAX_ERROR_BYTES = 65_536;

function positive(value: number): boolean {
  return Number.isSafeInteger(value) && value > 0;
}

function parseContentLength(value: string | null): number | null {
  if (!value || !/^\d+$/.test(value)) return null;
  const size = Number(value);
  return Number.isSafeInteger(size) ? size : Infinity;
}

/** Read a bounded response body incrementally. TextDecoder maintains UTF-8 state
 * between chunks, so a Unicode codepoint split across buffers remains intact.
 */
export async function readBoundedText(response: Response, maxBytes: number, signal?: AbortSignal): Promise<string> {
  if (!positive(maxBytes)) throw new RangeError("HTTP body limit must be a positive safe integer.");
  const declared = parseContentLength(response.headers.get("content-length"));
  // Content-Length on encoded responses describes the wire bytes, not the
  // decoded body. The stream below is always independently checked.
  if (!response.headers.has("content-encoding") && declared !== null && declared > maxBytes) {
    void response.body?.cancel().catch(() => undefined);
    throw new NativeTransportError("The local server response exceeds the supported size.", response.status);
  }
  if (!response.body) return "";
  const reader = response.body.getReader();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const chunks: string[] = [];
  let bytes = 0;
  let done = false;
  const abort = () => { void reader.cancel().catch(() => undefined); };
  signal?.addEventListener("abort", abort, { once: true });
  try {
    if (signal?.aborted) throw new NativeTransportError("The local API request was cancelled.", response.status);
    while (true) {
      const next = await reader.read();
      if (signal?.aborted) throw new NativeTransportError("The local API request was cancelled.", response.status);
      if (next.done) {
        done = true;
        chunks.push(decoder.decode());
        return chunks.join("");
      }
      bytes += next.value.byteLength;
      if (!Number.isSafeInteger(bytes) || bytes > maxBytes) {
        throw new NativeTransportError("The local server response exceeds the supported size.", response.status);
      }
      chunks.push(decoder.decode(next.value, { stream: true }));
    }
  } catch (reason) {
    if (reason instanceof NativeTransportError) throw reason;
    throw new NativeTransportError("The local server response could not be read as UTF-8 text.", response.status);
  } finally {
    // Cancellation is intentionally not awaited: an underlying stream's cancel
    // hook can itself stall. Releasing the lock remains safe after read settled.
    signal?.removeEventListener("abort", abort);
    if (!done) void reader.cancel().catch(() => undefined);
    reader.releaseLock();
  }
}

/** Make a local API request without following redirects. A redirect could send
 * a custom authentication header to a different origin. Deadlines are logical
 * as well as physical: a fetch implementation that ignores abort cannot keep
 * the UI waiting, and its eventual response is discarded.
 */
export function requestNativeText(path: string, init: RequestInit, options: NativeTransportOptions): Promise<NativeTextResponse> {
  if (!path.startsWith("/api/") || path.startsWith("//") || /[\\\x00-\x1f\x7f]/.test(path) ||
    path.split("?")[0]?.split("/").some((segment) => /^(?:\.|%2e){1,2}$/i.test(segment))) {
    return Promise.reject(new NativeTransportError("Invalid local API request path.", 0));
  }
  if (!positive(options.maxBytes) || !positive(options.deadlineMs)) {
    return Promise.reject(new RangeError("HTTP response limits must be positive safe integers."));
  }
  const clock = options.clock ?? browserClock;
  const fetcher = options.fetcher ?? fetch;
  const controller = new AbortController();
  const external = init.signal;
  return new Promise((resolve, reject) => {
    let settled = false;
    let timer: number | null = null;
    let responseStatus = 0;
    const finish = (complete: () => void) => {
      if (settled) return;
      settled = true;
      if (timer !== null) clock.clear(timer);
      external?.removeEventListener("abort", abort);
      complete();
    };
    const abort = () => {
      finish(() => reject(new NativeTransportError("The local API request was cancelled.", responseStatus)));
      controller.abort();
    };
    if (external?.aborted) {
      abort();
      return;
    }
    external?.addEventListener("abort", abort, { once: true });
    timer = clock.set(() => {
      finish(() => reject(new NativeTransportError("The local server did not respond in time.", responseStatus)));
      controller.abort();
    }, options.deadlineMs);
    const read = async () => {
      const response = await fetcher(path, {
        ...init,
        signal: controller.signal,
        redirect: "error",
        cache: "no-store",
        credentials: "same-origin",
      });
      if (settled) {
        void response.body?.cancel().catch(() => undefined);
        return;
      }
      responseStatus = response.status;
      if (response.redirected || response.type === "opaqueredirect") {
        void response.body?.cancel().catch(() => undefined);
        throw new NativeTransportError("The local API unexpectedly redirected the request.", response.status);
      }
      // Bound even unsuccessful responses. A proxy's giant HTML error page is
      // not useful diagnostics and must not consume the artifact budget.
      let text: string;
      try {
        text = await readBoundedText(response, response.ok ? options.maxBytes : Math.min(options.maxBytes, MAX_ERROR_BYTES), controller.signal);
      } catch (reason) {
        if (!response.ok) text = "";
        else throw reason;
      }
      finish(() => resolve({ status: response.status, ok: response.ok, text }));
    };
    void read().catch((reason: unknown) => {
      finish(() => reject(reason instanceof NativeTransportError ? reason : new NativeTransportError(`The local API request failed: ${errorMessage(reason)}`, responseStatus)));
    });
  });
}

/** Parse only well-formed API JSON, retaining HTTP status when an error page is not JSON. */
export function decodeNativeJson(response: NativeTextResponse, allowEmpty = false): unknown {
  let value: unknown;
  try { value = response.text ? JSON.parse(response.text) : undefined; }
  catch { value = undefined; }
  if (!response.ok) {
    const detail = value && typeof value === "object" && !Array.isArray(value) && "error" in value && typeof value.error === "string"
      ? value.error.slice(0, 4096) : "";
    throw new NativeTransportError(detail || `The server answered ${response.status}.`, response.status);
  }
  if (value === undefined || value === null) {
    if (allowEmpty && response.status === 204 && response.text === "") return undefined;
    throw new NativeTransportError("The server returned an invalid JSON response.", response.status);
  }
  return value;
}
