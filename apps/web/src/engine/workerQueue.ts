import type { EngineReply, EngineRequest, ProgressEvent } from "./protocol";
import { browserClock, errorMessage, type RequestClock } from "../lib/requestDeadline";

export type WorkerBody = EngineRequest extends infer R
  ? R extends unknown
    ? Omit<R, "id" | "wasm">
    : never
  : never;

export type WorkerOutput = Pick<Extract<EngineReply, { kind: "done" }>, "output" | "size">;

export interface WorkerTask<T> {
  result: Promise<T>;
  cancel(): void;
}

export class CancelledError extends Error {
  override name = "CancelledError";
  constructor() {
    super("Cancelled.");
  }
}

export class WorkerProtocolError extends Error {
  override name = "WorkerProtocolError";
}

/** A small injectable boundary, allowing the queue to be tested without WASM or a DOM worker. */
export interface WorkerPort {
  onmessage: ((event: MessageEvent<unknown>) => void) | null;
  onerror: ((event: ErrorEvent) => void) | null;
  onmessageerror: ((event: MessageEvent<unknown>) => void) | null;
  postMessage(message: EngineRequest): void;
  terminate(): void;
}

export interface WorkerQueueOptions {
  create: () => WorkerPort;
  clock?: RequestClock;
  /** Includes the running request. */
  capacity?: number;
  inactivityMs?: number;
  maxRuntimeMs?: number;
}

const DEFAULT_CAPACITY = 16;
// A complex native/WASM stage can legitimately take several minutes without progress.
// Give it a generous deadline, while making a truly stalled worker recoverable.
const DEFAULT_INACTIVITY = 10 * 60 * 1000;
const DEFAULT_RUNTIME = 60 * 60 * 1000;

interface Queued {
  id: number;
  body: WorkerBody;
  wasm: string;
  progress?: (event: ProgressEvent) => void;
  resolve(output: WorkerOutput): void;
  reject(error: Error): void;
  inactivity: number | null;
  runtime: number | null;
}

function object(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function count(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function boundedText(value: unknown, max: number): value is string {
  return typeof value === "string" && value.length <= max;
}

/** Unknown or damaged progress packets must not reach React state. */
export function parseWorkerProgress(value: unknown): ProgressEvent | null {
  if (!object(value)) return null;
  switch (value.event) {
    case "stage":
      if (!boundedText(value.stage, 256) || !value.stage || !boundedText(value.status, 64) || !value.status) return null;
      if (value.milliseconds !== undefined && !count(value.milliseconds)) return null;
      return {
        event: "stage",
        stage: value.stage,
        status: value.status,
        ...(typeof value.milliseconds === "number" ? { milliseconds: value.milliseconds } : {}),
      };
    case "files":
      if (!count(value.done) || !count(value.total) || value.done > value.total) return null;
      return { event: "files", done: value.done, total: value.total };
    case "message":
      return boundedText(value.text, 16_384) ? { event: "message", text: value.text } : null;
    default:
      return null;
  }
}

function expectsImage(body: WorkerBody): boolean {
  return body.kind === "card" && body.png;
}

/** Check both the wire shape and the specific operation's expected output type. */
function readOutput(value: Record<string, unknown>, body: WorkerBody): WorkerOutput | null {
  const { output, size } = value;
  if (!count(size)) return null;
  if (expectsImage(body)) {
    if (!(output instanceof Uint8Array) || !(output.buffer instanceof ArrayBuffer) || output.byteLength !== size) return null;
    // Reject SharedArrayBuffer-backed views and normalize to the wire type.
    return { output: output as Extract<WorkerOutput["output"], Uint8Array>, size };
  }
  // UTF-8 takes between 1 and 3 bytes per UTF-16 code unit. Avoid duplicating a
  // potentially huge JSON output on the UI thread merely to recompute its size.
  if (typeof output !== "string" || size < output.length || size > output.length * 3) return null;
  return { output, size };
}

function validatePositive(value: number, label: string): void {
  if (!Number.isSafeInteger(value) || value <= 0) throw new RangeError(`${label} must be a positive integer.`);
}

/**
 * Single active WASM operation with bounded FIFO backpressure. Cancelling a
 * queued task removes only that task. Cancelling or timing out the active task
 * terminates its worker (WASM is synchronous); queued work resumes on a fresh one.
 * Events captured from an old worker can never settle a new task.
 */
export class WorkerQueue {
  private worker: WorkerPort | null = null;
  private active: Queued | null = null;
  private readonly queued: Queued[] = [];
  private nextId = 1;
  private closed = false;
  private pumping = false;
  private readonly clock: RequestClock;
  private readonly capacity: number;
  private readonly inactivityMs: number;
  private readonly maxRuntimeMs: number;

  constructor(private readonly options: WorkerQueueOptions) {
    this.clock = options.clock ?? browserClock;
    this.capacity = options.capacity ?? DEFAULT_CAPACITY;
    this.inactivityMs = options.inactivityMs ?? DEFAULT_INACTIVITY;
    this.maxRuntimeMs = options.maxRuntimeMs ?? DEFAULT_RUNTIME;
    validatePositive(this.capacity, "Worker queue capacity");
    validatePositive(this.inactivityMs, "Worker inactivity deadline");
    validatePositive(this.maxRuntimeMs, "Worker runtime deadline");
  }

  get pending(): number {
    return this.queued.length + Number(this.active !== null);
  }

  request(body: WorkerBody, wasm: string, progress?: (event: ProgressEvent) => void): WorkerTask<WorkerOutput> {
    if (this.closed || this.pending >= this.capacity) {
      return {
        result: Promise.reject(new Error(this.closed ? "The analysis worker is closed." : "Too many browser analysis requests. Try again after another finishes.")),
        cancel() {},
      };
    }
    const id = this.nextId++;
    let request: Queued | null = null;
    const result = new Promise<WorkerOutput>((resolve, reject) => {
      request = { id, body, wasm, progress, resolve, reject, inactivity: null, runtime: null };
      this.queued.push(request);
    });
    this.pump();
    return { result, cancel: () => { if (request) this.cancel(request); } };
  }

  private cancel(request: Queued): void {
    if (this.active === request) {
      this.failActive(new CancelledError(), true);
      return;
    }
    const index = this.queued.indexOf(request);
    if (index !== -1) {
      this.queued.splice(index, 1);
      request.reject(new CancelledError());
    }
  }

  private clearTimers(request: Queued): void {
    if (request.inactivity !== null) this.clock.clear(request.inactivity);
    if (request.runtime !== null) this.clock.clear(request.runtime);
    request.inactivity = null;
    request.runtime = null;
  }

  private resetInactivity(request: Queued): void {
    if (request.inactivity !== null) this.clock.clear(request.inactivity);
    request.inactivity = this.clock.set(() => {
      if (this.active === request) this.failActive(new Error("The browser analysis worker stopped reporting progress."), true);
    }, this.inactivityMs);
  }

  private dropWorker(): void {
    const old = this.worker;
    this.worker = null;
    if (old) {
      old.onmessage = null;
      old.onerror = null;
      old.onmessageerror = null;
      old.terminate();
    }
  }

  private port(): WorkerPort {
    if (this.worker) return this.worker;
    const worker = this.options.create();
    this.worker = worker;
    worker.onmessage = (event) => {
      if (this.worker === worker) this.receive(event.data);
    };
    worker.onerror = (event) => {
      event.preventDefault();
      if (this.worker === worker) this.failActive(new Error(event.message || "RepoDNA's analysis program stopped unexpectedly."), true);
    };
    worker.onmessageerror = () => {
      if (this.worker === worker) this.failActive(new WorkerProtocolError("The analysis worker returned an unreadable message."), true);
    };
    return worker;
  }

  private pump(): void {
    if (this.closed || this.pumping) return;
    this.pumping = true;
    try {
      while (!this.active && this.queued.length) {
        const request = this.queued.shift()!;
        this.active = request;
        try {
          const worker = this.port();
          this.resetInactivity(request);
          request.runtime = this.clock.set(() => {
            if (this.active === request) this.failActive(new Error("The browser analysis exceeded its maximum runtime."), true);
          }, this.maxRuntimeMs);
          worker.postMessage({ ...request.body, id: request.id, wasm: request.wasm } as EngineRequest);
        } catch (reason) {
          if (this.active === request) this.failActive(new Error(errorMessage(reason)), true);
        }
      }
    } finally {
      this.pumping = false;
    }
  }

  private receive(value: unknown): void {
    const request = this.active;
    if (!request || !object(value) || value.id !== request.id) return;
    if (value.kind === "progress") {
      const progress = parseWorkerProgress(value.progress);
      if (!progress) return;
      this.resetInactivity(request);
      try {
        request.progress?.(progress);
      } catch {
        // A failing UI observer must not interrupt the active worker.
      }
      return;
    }
    if (value.kind === "done") {
      const output = readOutput(value, request.body);
      if (!output) {
        this.failActive(new WorkerProtocolError("The analysis worker returned an invalid result."), true);
        return;
      }
      this.active = null;
      this.clearTimers(request);
      request.resolve(output);
      this.pump();
      return;
    }
    if (value.kind === "failed" && boundedText(value.message, 65_536)) {
      this.failActive(new Error(value.message || "The browser analysis failed."), false);
      return;
    }
    this.failActive(new WorkerProtocolError("The analysis worker returned an unknown response."), true);
  }

  private failActive(error: Error, reset: boolean): void {
    const request = this.active;
    this.active = null;
    if (reset) this.dropWorker();
    if (request) {
      this.clearTimers(request);
      request.reject(error);
    }
    this.pump();
  }

  /** Dispose the queue; all outstanding callers get a terminal result. */
  close(): void {
    if (this.closed) return;
    this.closed = true;
    const reason = new CancelledError();
    this.failActive(reason, true);
    for (const request of this.queued.splice(0)) request.reject(reason);
  }
}
