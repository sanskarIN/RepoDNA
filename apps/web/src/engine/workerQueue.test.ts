import { describe, expect, it } from "vitest";
import type { EngineRequest } from "./protocol";
import { Clock, flush } from "../lib/controllerTestSupport";
import { CancelledError, parseWorkerProgress, WorkerQueue, type WorkerPort, type WorkerQueueOptions } from "./workerQueue";

class FakeWorker implements WorkerPort {
  onmessage: WorkerPort["onmessage"] = null;
  onerror: WorkerPort["onerror"] = null;
  onmessageerror: WorkerPort["onmessageerror"] = null;
  sent: EngineRequest[] = [];
  terminated = false;
  failPost: Error | null = null;

  postMessage(message: EngineRequest): void {
    if (this.failPost) throw this.failPost;
    this.sent.push(message);
  }

  terminate(): void { this.terminated = true; }

  deliver(data: unknown): void { this.onmessage?.({ data } as MessageEvent<unknown>); }

  done(output = "{}", id = this.sent.at(-1)?.id): void {
    this.deliver({ id, kind: "done", output, size: new TextEncoder().encode(output).length });
  }

  crash(message: string): void {
    this.onerror?.({ message, preventDefault() {} } as ErrorEvent);
  }
}

const wasm = "https://example.invalid/engine.wasm";
const scan = { kind: "analyze-folder" as const, name: "repo", files: [], profile: "quick" as const };
const report = { kind: "report", artifact: "{}", format: "html", theme: "dark", privacy: "local" } as const;
const png = { kind: "card", artifact: "{}", dark: true, png: true } as const;

function setup(overrides: Partial<WorkerQueueOptions> = {}) {
  const clock = new Clock();
  const workers: FakeWorker[] = [];
  const queue = new WorkerQueue({
    clock, capacity: 3, inactivityMs: 100, maxRuntimeMs: 300,
    create: () => { const worker = new FakeWorker(); workers.push(worker); return worker; },
    ...overrides,
  });
  const last = () => workers.at(-1)!;
  return { queue, workers, clock, last };
}

async function rejected<T>(promise: Promise<T>): Promise<Error> {
  try { await promise; } catch (reason) { if (reason instanceof Error) return reason; throw reason; }
  throw new Error("Expected failure");
}

describe("single-flight browser worker queue", () => {
  it("runs requests in FIFO order and reuses the compiled worker", async () => {
    const { queue, workers, last, clock } = setup();
    const a = queue.request(scan, wasm);
    const b = queue.request(report, wasm + "?v=2");
    expect(queue.pending).toBe(2);
    expect(last().sent.map((request) => request.kind)).toEqual(["analyze-folder"]);
    last().done("first");
    expect(await a.result).toEqual({ output: "first", size: 5 });
    expect(last().sent.map((request) => request.kind)).toEqual(["analyze-folder", "report"]);
    expect(last().sent[1]?.wasm).toBe(wasm + "?v=2");
    last().done("second");
    expect(await b.result).toEqual({ output: "second", size: 6 });
    expect(queue.pending).toBe(0);
    expect(workers.length).toBe(1);
    expect(clock.pending).toBe(0);
  });

  it("cancels one queued task without terminating the active worker", async () => {
    const { queue, last, workers } = setup();
    const active = queue.request(scan, wasm);
    const queued = queue.request(report, wasm);
    const failure = rejected(queued.result);
    queued.cancel();
    queued.cancel();
    expect(await failure).toBeInstanceOf(CancelledError);
    expect(queue.pending).toBe(1);
    expect(last().terminated).toBe(false);
    last().done();
    await active.result;
    expect(workers.length).toBe(1);
    expect(last().sent.length).toBe(1);
  });

  it("cancels an active task and starts unrelated queued work on a new worker", async () => {
    const { queue, last, workers, clock } = setup();
    const active = queue.request(scan, wasm);
    const old = last();
    const queued = queue.request(report, wasm);
    const failure = rejected(active.result);
    active.cancel();
    expect(await failure).toBeInstanceOf(CancelledError);
    expect(old.terminated).toBe(true);
    expect(workers.length).toBe(2);
    expect(last().sent[0]?.kind).toBe("report");
    last().done("report");
    expect((await queued.result).output).toBe("report");
    expect(clock.pending).toBe(0);
  });

  it("does not allow an old worker's captured events to settle a newer task", async () => {
    const { queue, last } = setup();
    const first = queue.request(scan, wasm);
    const oldMessage = last().onmessage;
    const oldError = last().onerror;
    const fail = rejected(first.result);
    first.cancel();
    await fail;
    const next = queue.request(scan, wasm);
    const id = last().sent[0]!.id;
    oldMessage?.({ data: { id, kind: "done", output: "stale", size: 5 } } as MessageEvent);
    oldError?.({ message: "old crash", preventDefault() {} } as ErrorEvent);
    await flush();
    expect(queue.pending).toBe(1);
    expect(last().terminated).toBe(false);
    last().done("new");
    expect((await next.result).output).toBe("new");
  });

  it("bounds queued work and allows new requests after capacity is released", async () => {
    const { queue, last } = setup({ capacity: 2 });
    const a = queue.request(scan, wasm);
    const b = queue.request(report, wasm);
    const overflow = queue.request(scan, wasm);
    expect((await rejected(overflow.result)).message).toContain("Too many");
    expect(queue.pending).toBe(2);
    last().done();
    await a.result;
    const c = queue.request(scan, wasm);
    last().done();
    await b.result;
    last().done();
    await c.result;
    expect(queue.pending).toBe(0);
  });

  it("recovers from a worker constructor failure", async () => {
    const worker = new FakeWorker();
    let attempts = 0;
    const { queue, clock } = setup({ create: () => { if (++attempts === 1) throw new Error("worker blocked"); return worker; } });
    expect((await rejected(queue.request(scan, wasm).result)).message).toBe("worker blocked");
    expect(clock.pending).toBe(0);
    const next = queue.request(scan, wasm);
    worker.done();
    await next.result;
    expect(queue.pending).toBe(0);
  });

  it("recovers from postMessage cloning failures without keeping a damaged worker", async () => {
    const broken = new FakeWorker();
    broken.failPost = new Error("Cannot clone file");
    const healthy = new FakeWorker();
    let attempts = 0;
    const { queue, clock } = setup({ create: () => ++attempts === 1 ? broken : healthy });
    expect((await rejected(queue.request(scan, wasm).result)).message).toBe("Cannot clone file");
    expect(broken.terminated).toBe(true);
    expect(clock.pending).toBe(0);
    const next = queue.request(scan, wasm);
    healthy.done();
    await next.result;
  });

  it("recovers from a worker runtime error and continues queued requests", async () => {
    const { queue, last, workers } = setup();
    const a = queue.request(scan, wasm);
    const b = queue.request(report, wasm);
    const failed = rejected(a.result);
    last().crash("WASM trapped");
    expect((await failed).message).toBe("WASM trapped");
    expect(workers[0]?.terminated).toBe(true);
    last().done("report");
    expect((await b.result).output).toBe("report");
  });

  it("handles message deserialization errors", async () => {
    const { queue, last, clock } = setup();
    const task = queue.request(scan, wasm);
    const failure = rejected(task.result);
    last().onmessageerror?.({ data: undefined } as MessageEvent);
    expect((await failure).message).toContain("unreadable message");
    expect(last().terminated).toBe(true);
    expect(clock.pending).toBe(0);
  });

  it("retains the worker for ordinary operation failures", async () => {
    const { queue, last, workers } = setup();
    const a = queue.request(scan, wasm);
    const b = queue.request(report, wasm);
    const failed = rejected(a.result);
    last().deliver({ id: last().sent.at(-1)?.id, kind: "failed", message: "invalid archive" });
    expect((await failed).message).toBe("invalid archive");
    expect(workers.length).toBe(1);
    last().done();
    await b.result;
  });

  it("does not allow duplicate or unrelated response IDs to affect the active request", async () => {
    const { queue, last } = setup();
    const a = queue.request(scan, wasm);
    const b = queue.request(report, wasm);
    const firstId = last().sent[0]!.id;
    last().done("unrelated", firstId + 500);
    expect(queue.pending).toBe(2);
    last().done("first");
    await a.result;
    last().done("duplicate", firstId);
    expect(queue.pending).toBe(1);
    last().done("second");
    expect((await b.result).output).toBe("second");
  });

  it("validates operation-specific output kinds and PNG byte lengths", async () => {
    const { queue, last } = setup();
    const image = queue.request(png, wasm);
    const bytes = new Uint8Array([1, 2, 3]);
    last().deliver({ id: last().sent.at(-1)?.id, kind: "done", output: bytes, size: 3 });
    expect(await image.result).toEqual({ output: bytes, size: 3 });
    const invalid = queue.request(png, wasm);
    last().done("text");
    expect((await rejected(invalid.result)).message).toContain("invalid result");
    const wrongSize = queue.request(png, wasm);
    last().deliver({ id: last().sent.at(-1)?.id, kind: "done", output: bytes, size: 2 });
    expect((await rejected(wrongSize.result)).message).toContain("invalid result");
  });

  it("accepts multibyte text and rejects impossible text lengths and image results", async () => {
    const { queue, last } = setup();
    const valid = queue.request(scan, wasm);
    last().done("日本語😀");
    expect((await valid.result).output).toBe("日本語😀");
    for (const [output, size] of [["long", 2], ["a", 100], ["x", -1], [new Uint8Array([1]), 1]]) {
      const invalid = queue.request(scan, wasm);
      last().deliver({ id: last().sent.at(-1)?.id, kind: "done", output, size });
      expect((await rejected(invalid.result)).message).toContain("invalid result");
    }
  });

  it("rejects unknown terminal reply kinds", async () => {
    const { queue, last } = setup();
    const task = queue.request(scan, wasm);
    last().deliver({ id: last().sent.at(-1)?.id, kind: "unexpected" });
    expect((await rejected(task.result)).message).toContain("unknown response");
  });

  it("does not lose terminal results when progress subscribers throw", async () => {
    const { queue, last } = setup();
    let count = 0;
    const task = queue.request(scan, wasm, () => { count++; throw new Error("UI failed"); });
    last().deliver({ id: last().sent.at(-1)?.id, kind: "progress", progress: { event: "files", done: 1, total: 2 } });
    last().done();
    await task.result;
    expect(count).toBe(1);
  });

  it("extends inactivity deadlines only for valid progress", async () => {
    const { queue, last, clock } = setup();
    const events: unknown[] = [];
    const task = queue.request(scan, wasm, (progress) => events.push(progress));
    const failure = rejected(task.result);
    const id = last().sent.at(-1)?.id;
    await clock.advance(80);
    last().deliver({ id, kind: "progress", progress: { event: "files", done: 1, total: 2 } });
    await clock.advance(80);
    last().deliver({ id, kind: "progress", progress: { event: "files", done: -1, total: 2 } });
    expect(queue.pending).toBe(1);
    expect(events.length).toBe(1);
    await clock.advance(20);
    expect((await failure).message).toContain("stopped reporting progress");
    expect(clock.pending).toBe(0);
  });

  it("enforces an absolute runtime even when progress continues", async () => {
    const { queue, last, clock } = setup({ maxRuntimeMs: 250 });
    const task = queue.request(scan, wasm);
    const failure = rejected(task.result);
    for (let step = 0; step < 3; step++) {
      await clock.advance(80);
      last().deliver({ id: last().sent.at(-1)?.id, kind: "progress", progress: { event: "files", done: step, total: 4 } });
    }
    await clock.advance(10);
    expect((await failure).message).toContain("maximum runtime");
    expect(clock.pending).toBe(0);
  });

  it("rejects every outstanding task on disposal and never creates another worker", async () => {
    const { queue, last, clock } = setup();
    const a = rejected(queue.request(scan, wasm).result);
    const b = rejected(queue.request(report, wasm).result);
    queue.close();
    queue.close();
    expect(await a).toBeInstanceOf(CancelledError);
    expect(await b).toBeInstanceOf(CancelledError);
    expect(last().terminated).toBe(true);
    expect(clock.pending).toBe(0);
    expect((await rejected(queue.request(scan, wasm).result)).message).toContain("closed");
  });

  it("validates every supported progress shape", () => {
    expect(parseWorkerProgress({ event: "stage", stage: "scan", status: "completed", milliseconds: 0 })).toEqual({ event: "stage", stage: "scan", status: "completed", milliseconds: 0 });
    expect(parseWorkerProgress({ event: "files", done: 0, total: 0 })).toEqual({ event: "files", done: 0, total: 0 });
    expect(parseWorkerProgress({ event: "message", text: "working" })).toEqual({ event: "message", text: "working" });
    for (const value of [null, [], {}, { event: "files", done: 3, total: 2 }, { event: "files", done: 0.5, total: 2 }, { event: "stage", stage: "", status: "started" }, { event: "stage", stage: "scan", status: "completed", milliseconds: NaN }, { event: "message", text: "x".repeat(16385) }]) expect(parseWorkerProgress(value)).toBeNull();
  });
});
