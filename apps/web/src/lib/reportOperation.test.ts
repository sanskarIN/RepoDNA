import { describe, expect, it } from "vitest";
import { Clock, deferred, flush } from "./controllerTestSupport";
import {
  ReportOperation, NativeSaveGate, nativeSaveGate, createPreview, releasePreview,
  mapReportTask, asReportTask, svgPreview, type ReportOperationState,
} from "./reportOperation";

function setup<T = string>() {
  const states: ReportOperationState<T>[] = [];
  const discarded: T[] = [];
  const clock = new Clock();
  const operation = new ReportOperation<T>((state) => states.push(state), clock, (value) => discarded.push(value));
  return { states, discarded, clock, operation };
}

function task<T>() {
  const pending = deferred<T>();
  let cancellations = 0;
  return {
    ...pending,
    request: { result: pending.promise, cancel: () => { cancellations += 1; } },
    get cancellations() { return cancellations; },
  };
}

describe("scoped report generation", () => {
  it("publishes a result and clears the deadline", async () => {
    const { operation, clock, states } = setup();
    const work = task<string>();
    expect(operation.start("HTML", () => work.request)).toBe(true);
    expect(operation.busy).toBe(true);
    work.resolve("document");
    await flush();
    expect(states).toEqual([
      { status: "running", label: "HTML", cancellable: true },
      { status: "ready", label: "HTML", value: "document" },
    ]);
    expect(operation.busy).toBe(false);
    expect(clock.pending).toBe(0);
  });

  it("does not start a second operation while the first owns the slot", async () => {
    const { operation } = setup();
    const work = task<string>();
    let starts = 0;
    operation.start("HTML", () => work.request);
    expect(operation.start("PNG", () => { starts++; return work.request; })).toBe(false);
    expect(starts).toBe(0);
    work.resolve("first");
    await flush();
    expect(operation.start("PNG", () => asReportTask(Promise.resolve("second")))).toBe(true);
    await flush();
    expect(operation.state).toEqual({ status: "ready", label: "PNG", value: "second" });
  });

  it("cancels the owned task and discards a late result", async () => {
    const { operation, discarded, clock } = setup();
    const work = task<string>();
    let downloads = 0;
    operation.start("HTML", () => work.request, {}, () => { downloads++; });
    expect(operation.cancel()).toBe(true);
    expect(work.cancellations).toBe(1);
    expect(clock.pending).toBe(0);
    work.resolve("obsolete");
    await flush();
    expect(discarded).toEqual(["obsolete"]);
    expect(downloads).toBe(0);
    expect(operation.state).toEqual({ status: "cancelled", label: "HTML" });
  });

  it("a late rejection cannot overwrite the next report", async () => {
    const { operation } = setup();
    const first = task<string>();
    operation.start("first", () => first.request);
    operation.cancel();
    operation.start("second", () => asReportTask(Promise.resolve("new")));
    first.reject(new Error("old failure"));
    await flush();
    expect(operation.state).toEqual({ status: "ready", label: "second", value: "new" });
  });

  it("times out a stalled renderer and releases a late resource", async () => {
    const { operation, clock, discarded } = setup();
    const work = task<string>();
    operation.start("SVG", () => work.request, { deadlineMs: 50 });
    await clock.advance(49);
    expect(operation.busy).toBe(true);
    await clock.advance(1);
    expect(operation.state).toEqual({ status: "failed", label: "SVG", message: "SVG did not finish in time." });
    expect(work.cancellations).toBe(1);
    work.resolve("late");
    await flush();
    expect(discarded).toEqual(["late"]);
  });

  it("retains a ready resource until reset", async () => {
    const { operation, discarded } = setup();
    operation.start("preview", () => asReportTask(Promise.resolve("blob:one")));
    await flush();
    expect(discarded).toEqual([]);
    operation.reset();
    expect(discarded).toEqual(["blob:one"]);
    operation.reset();
    expect(discarded).toEqual(["blob:one"]);
  });

  it("releases an old preview when a new generation starts", async () => {
    const { operation, discarded } = setup();
    operation.start("light", () => asReportTask(Promise.resolve("light")));
    await flush();
    const dark = task<string>();
    operation.start("dark", () => dark.request);
    expect(discarded).toEqual(["light"]);
    dark.resolve("dark");
    await flush();
    operation.dispose();
    expect(discarded).toEqual(["light", "dark"]);
  });

  it("does not publish after unmount", async () => {
    const { operation, states, discarded } = setup();
    const work = task<string>();
    operation.start("preview", () => work.request);
    operation.dispose();
    work.resolve("stale");
    await flush();
    expect(states.length).toBe(1);
    expect(discarded).toEqual(["stale"]);
    expect(operation.start("next", () => work.request)).toBe(false);
  });

  it("converts synchronous renderer exceptions to an actionable failure", () => {
    const { operation, clock } = setup();
    operation.start("report", () => { throw new Error("cannot render"); });
    expect(operation.state).toEqual({ status: "failed", label: "report", message: "cannot render" });
    expect(clock.pending).toBe(0);
    expect(operation.busy).toBe(false);
  });

  it("reports failed rendering without performing its download", async () => {
    const { operation } = setup();
    const work = task<string>();
    let saved = false;
    operation.start("report", () => work.request, {}, () => { saved = true; });
    work.reject(new Error("renderer failed"));
    await flush();
    expect(saved).toBe(false);
    expect(operation.state).toEqual({ status: "failed", label: "report", message: "renderer failed" });
  });

  it("reports a synchronous download failure and discards its resource", async () => {
    const { operation, discarded } = setup();
    operation.start("report", () => asReportTask(Promise.resolve("text")), {}, () => { throw new Error("download failed"); });
    await flush();
    expect(operation.state).toEqual({ status: "failed", label: "report", message: "download failed" });
    expect(discarded).toEqual(["text"]);
  });

  it("allows the user to take their time in a native save dialog", async () => {
    const { operation, clock } = setup<string | null>();
    const work = task<string | null>();
    operation.start("save", () => work.request, { cancellable: false, deadlineMs: null });
    expect(operation.cancel()).toBe(false);
    await clock.advance(60 * 60 * 1000);
    expect(operation.busy).toBe(true);
    expect(work.cancellations).toBe(0);
    work.resolve(null);
    await flush();
    expect(operation.state).toEqual({ status: "ready", label: "save", value: null });
  });

  it("rejects invalid deadlines before reserving the operation", () => {
    const { operation } = setup();
    let error: unknown;
    try { operation.start("bad", () => asReportTask(Promise.resolve("")), { deadlineMs: -1 }); }
    catch (reason) { error = reason; }
    expect(error).toBeInstanceOf(RangeError);
    expect(operation.busy).toBe(false);
  });

  it("does not start a renderer cancelled by a synchronous subscriber", () => {
    const clock = new Clock();
    let starts = 0;
    const operation = new ReportOperation<string>((state) => {
      if (state.status === "running") operation.cancel();
    }, clock);
    operation.start("preview", () => { starts++; return asReportTask(Promise.resolve("svg")); });
    expect(starts).toBe(0);
    expect(clock.pending).toBe(0);
    expect(operation.state).toEqual({ status: "cancelled", label: "preview" });
  });

  it("stops a task abandoned during its synchronous creation", async () => {
    const { operation, discarded } = setup();
    const work = task<string>();
    operation.start("preview", () => { operation.reset(); return work.request; });
    expect(work.cancellations).toBe(1);
    work.resolve("late");
    await flush();
    expect(discarded).toEqual(["late"]);
    expect(operation.state).toEqual({ status: "idle" });
  });

  it("does not let a ready callback reset and then republish a stale result", async () => {
    const { operation, discarded } = setup();
    operation.start("preview", () => asReportTask(Promise.resolve("old")), {}, () => operation.reset());
    await flush();
    expect(operation.state).toEqual({ status: "idle" });
    expect(discarded).toEqual(["old"]);
  });
});

describe("native dialog admission", () => {
  it("shares admission for one backend and isolates different backends", () => {
    const a = {};
    const b = {};
    expect(nativeSaveGate(a)).toBe(nativeSaveGate(a));
    expect(nativeSaveGate(a)).not.toBe(nativeSaveGate(b));
  });

  it("rejects overlap without invoking the second save", async () => {
    const gate = new NativeSaveGate();
    const work = deferred<string>();
    const first = gate.run(() => work.promise);
    let starts = 0;
    const second = gate.run(() => { starts++; return Promise.resolve("other"); });
    const error = await second.catch((reason: unknown) => reason);
    expect(error).toBeInstanceOf(Error);
    expect(starts).toBe(0);
    expect(gate.busy).toBe(true);
    work.resolve("saved");
    expect(await first).toBe("saved");
    expect(gate.busy).toBe(false);
  });

  it("releases admission after a synchronous failure", async () => {
    const gate = new NativeSaveGate();
    const failure = await gate.run(() => { throw new Error("dialog unavailable"); }).catch((reason: unknown) => reason);
    expect(failure).toBeInstanceOf(Error);
    expect(await gate.run(() => Promise.resolve("saved"))).toBe("saved");
  });

  it("releases admission after asynchronous rejection", async () => {
    const gate = new NativeSaveGate();
    const failure = await gate.run(() => Promise.reject(new Error("disk full"))).catch((reason: unknown) => reason);
    expect(failure).toBeInstanceOf(Error);
    expect(gate.busy).toBe(false);
  });

  it("does not treat discarding a component as closing its native dialog", async () => {
    const gate = new NativeSaveGate();
    const { operation } = setup<string>();
    const pending = deferred<string>();
    operation.start("save", () => asReportTask(gate.run(() => pending.promise)), { deadlineMs: null, cancellable: false });
    operation.dispose();
    expect(gate.busy).toBe(true);
    pending.resolve("saved");
    await flush();
    expect(gate.busy).toBe(false);
  });
});

describe("card resources", () => {
  it("wraps SVG as an image blob and revokes its exact URL", async () => {
    const blobs: Blob[] = [];
    const revoked: string[] = [];
    const urls = { createObjectURL: (blob: Blob) => { blobs.push(blob); return "blob:card"; }, revokeObjectURL: (url: string) => { revoked.push(url); } };
    const svg = '<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"/>';
    const result = createPreview(svg, urls);
    expect(result).toEqual({ svg, url: "blob:card" });
    expect(blobs[0]?.type).toBe("image/svg+xml");
    expect(await blobs[0]?.text()).toBe(svg);
    releasePreview(result, urls);
    expect(revoked).toEqual(["blob:card"]);
  });

  it("rejects a proxy HTML page instead of exposing it as a card", () => {
    let failure: unknown;
    try { svgPreview("<html>not a card</html>"); }
    catch (reason) { failure = reason; }
    expect(failure).toBeInstanceOf(Error);
  });

  it("preserves underlying cancellation through a mapped task", async () => {
    const source = task<string>();
    const mapped = mapReportTask(source.request, (value) => value.length);
    mapped.cancel?.();
    expect(source.cancellations).toBe(1);
    source.resolve("text");
    expect(await mapped.result).toBe(4);
  });

  it("propagates conversion failures to the owning operation", async () => {
    const { operation } = setup<number>();
    operation.start("preview", () => mapReportTask(asReportTask(Promise.resolve("bad")), () => { throw new Error("invalid preview"); }));
    await flush();
    expect(operation.state).toEqual({ status: "failed", label: "preview", message: "invalid preview" });
  });
});
