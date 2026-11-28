import { browserClock, type RequestClock } from "../lib/requestDeadline";
import { readBoundedText } from "../lib/nativeTransport";

export interface EngineInfo {
  version: string;
  bytes: number;
}

/** The manifest is metadata, not the downloaded binary. */
export const MAX_ENGINE_MANIFEST_BYTES = 64 * 1024;
export const ENGINE_MANIFEST_DEADLINE_MS = 10_000;

export function isInfo(value: unknown): value is EngineInfo {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const info = value as Record<string, unknown>;
  return (
    typeof info.version === "string" &&
    info.version.length <= 128 &&
    /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(info.version) &&
    typeof info.bytes === "number" &&
    Number.isSafeInteger(info.bytes) &&
    info.bytes > 0
  );
}

interface CatalogEntry {
  base: string;
  info: EngineInfo | null;
  pending: Promise<EngineInfo | null> | null;
  abandon: (() => void) | null;
}

export interface EngineCatalogOptions {
  fetcher?: typeof fetch;
  clock?: RequestClock;
  deadlineMs?: number;
}

function engineBase(base: string): string {
  const url = new URL("engine/", base);
  if (url.protocol !== "http:" && url.protocol !== "https:" && url.protocol !== "tauri:") {
    throw new Error("Unsupported engine location.");
  }
  if (url.username || url.password) throw new Error("Invalid engine location.");
  return url.href;
}

/** One manifest request per engine location. A successful manifest is cached;
 * unavailable engines, offline errors and timeouts are retryable. Changing the
 * page location invalidates both the cached version and outstanding request. */
export class EngineCatalog {
  private entry: CatalogEntry | null = null;
  private readonly fetcher: typeof fetch;
  private readonly clock: RequestClock;
  private readonly deadline: number;

  constructor(options: EngineCatalogOptions = {}) {
    // Resolve fetch at request time so browser/test overrides remain effective.
    this.fetcher = options.fetcher ?? ((input, init) => fetch(input, init));
    this.clock = options.clock ?? browserClock;
    this.deadline = options.deadlineMs ?? ENGINE_MANIFEST_DEADLINE_MS;
    if (!Number.isSafeInteger(this.deadline) || this.deadline <= 0) {
      throw new RangeError("Invalid engine manifest deadline.");
    }
  }

  private forBase(base: string): CatalogEntry {
    const location = engineBase(base);
    if (this.entry?.base === location) return this.entry;
    this.entry?.abandon?.();
    const entry: CatalogEntry = { base: location, info: null, pending: null, abandon: null };
    this.entry = entry;
    return entry;
  }

  load(base: string): Promise<EngineInfo | null> {
    let entry: CatalogEntry;
    try {
      entry = this.forBase(base);
    } catch {
      return Promise.resolve(null);
    }
    if (entry.info) return Promise.resolve({ ...entry.info });
    if (entry.pending) return entry.pending;
    const controller = new AbortController();
    // Publish the pending promise before invoking fetch, even if a mock or
    // embedding environment invokes another load synchronously from fetch.
    let resolve: (value: EngineInfo | null) => void = () => undefined;
    const pending = new Promise<EngineInfo | null>((done) => { resolve = done; });
    entry.pending = pending;
    let settled = false;
    let timer: number | null = null;
    const finish = (value: EngineInfo | null) => {
      if (settled) return;
      settled = true;
      if (timer !== null) this.clock.clear(timer);
      entry.pending = null;
      entry.abandon = null;
      if (value && this.entry === entry) entry.info = { ...value };
      resolve(value && this.entry === entry ? { ...value } : null);
    };
    const abandon = () => {
      finish(null);
      controller.abort();
    };
    entry.abandon = abandon;
    timer = this.clock.set(abandon, this.deadline);
    const read = async () => {
      const url = new URL("engine.json", entry.base);
      const response = await this.fetcher(url, {
        cache: "no-cache",
        credentials: "same-origin",
        redirect: "error",
        signal: controller.signal,
      });
      if (settled || !response.ok || response.redirected || response.type === "opaqueredirect") {
        void response.body?.cancel().catch(() => undefined);
        finish(null);
        return;
      }
      const raw = await readBoundedText(response, MAX_ENGINE_MANIFEST_BYTES, controller.signal);
      if (settled) return;
      const value: unknown = JSON.parse(raw);
      // Copy only fields used by the engine. Never retain arbitrary metadata.
      finish(isInfo(value) ? { version: value.version, bytes: value.bytes } : null);
    };
    void read().catch(() => finish(null));
    return pending;
  }

  /** Resolve the WASM URL against the same base as its manifest. The previous
   * location's version cannot become a cache key for a different engine. */
  wasmUrl(base: string): string {
    const entry = this.forBase(base);
    const url = new URL("repodna.wasm", entry.base);
    url.searchParams.set("v", entry.info?.version ?? "");
    return url.href;
  }

  reset(): void {
    const old = this.entry;
    this.entry = null;
    old?.abandon?.();
  }
}
