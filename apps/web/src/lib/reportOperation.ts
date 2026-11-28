import { browserClock, errorMessage, type RequestClock } from "./requestDeadline";

export interface ReportTask<T> {
  result: Promise<T>;
  /** Optional physical cancellation, such as cancelling a queued WASM request. */
  cancel?: () => void;
}
export type ReportOperationState<T> =
  | { status: "idle" }
  | { status: "running"; label: string; cancellable: boolean }
  | { status: "ready"; label: string; value: T }
  | { status: "cancelled"; label: string }
  | { status: "failed"; label: string; message: string };
export interface OperationOptions {
  /** Native save dialogs have no deadline: the user chooses when to finish. */
  deadlineMs?: number | null;
  cancellable?: boolean;
}
interface Active<T> {
  label: string;
  task: ReportTask<T> | null;
  timer: number | null;
  cancellable: boolean;
  /** True for a physically non-cancellable native action that was abandoned. */
  abandoned: boolean;
}
const GENERATION_DEADLINE = 10 * 60 * 1000;

/** Single-flight lifetime of one report, preview or save dialog. On source
 * changes or unmount, obsolete results are discarded and cancellable workers
 * are stopped. Non-cancellable native dialogs remain physical operations: a
 * logical cancellation is never presented as cancelling a native save. */
export class ReportOperation<T> {
  private snapshot: ReportOperationState<T> = { status: "idle" };
  private active: Active<T> | null = null;
  private disposed = false;
  constructor(
    private readonly publish: (state: ReportOperationState<T>) => void,
    private readonly clock: RequestClock = browserClock,
    /** Dispose stale resources such as object URLs created by a task. */
    private readonly discard: (value: T) => void = () => undefined,
  ) {}

  get state(): ReportOperationState<T> { return this.snapshot; }
  get busy(): boolean { return this.active !== null; }
  private update(next: ReportOperationState<T>): void {
    const previous = this.snapshot;
    this.snapshot = next;
    if (previous.status === "ready" && (next.status !== "ready" || previous.value !== next.value)) this.release(previous.value);
    if (!this.disposed) this.publish(next);
  }
  private release(value: T): void {
    try { this.discard(value); } catch { /* Disposal must not corrupt operation state. */ }
  }
  private clearTimer(operation: Active<T>): void {
    if (operation.timer !== null) this.clock.clear(operation.timer);
    operation.timer = null;
  }
  private stopTask(operation: Active<T>): void {
    try { operation.task?.cancel?.(); } catch { /* Results will be ignored regardless. */ }
  }
  private current(operation: Active<T>): boolean {
    return !this.disposed && this.active === operation && !operation.abandoned;
  }
  private finish(operation: Active<T>, state: ReportOperationState<T>): boolean {
    if (!this.current(operation)) return false;
    this.clearTimer(operation);
    this.active = null;
    this.update(state);
    return true;
  }

  /** `onReady` is invoked only for a result still owned by this operation.
   * Returning false means a second launch was rejected, not an operation error. */
  start(label: string, make: () => ReportTask<T>, options: OperationOptions = {}, onReady?: (value: T) => void): boolean {
    if (this.disposed || this.active) return false;
    const delay = options.deadlineMs === undefined ? GENERATION_DEADLINE : options.deadlineMs;
    if (delay !== null && (!Number.isSafeInteger(delay) || delay <= 0)) throw new RangeError("Invalid report operation deadline.");
    const operation: Active<T> = { label, task: null, timer: null, cancellable: options.cancellable !== false, abandoned: false };
    this.active = operation;
    this.update({ status: "running", label, cancellable: operation.cancellable });
    // A synchronous subscriber can cancel or dispose the operation before
    // the underlying task has even started.
    if (!this.current(operation)) return true;
    let task: ReportTask<T>;
    try { task = make(); }
    catch (reason) {
      this.finish(operation, { status: "failed", label, message: errorMessage(reason) });
      return true;
    }
    operation.task = task;
    if (delay !== null && this.current(operation)) operation.timer = this.clock.set(() => {
      if (this.finish(operation, { status: "failed", label, message: `${label} did not finish in time.` })) this.stopTask(operation);
    }, delay);
    void task.result.then(
      (value) => {
        if (!this.current(operation)) { this.release(value); return; }
        // Side effects such as downloading must be guarded before publishing
        // a ready state: subscribers may synchronously start another action.
        try { onReady?.(value); }
        catch (reason) {
          this.release(value);
          this.finish(operation, { status: "failed", label, message: errorMessage(reason) });
          return;
        }
        if (!this.finish(operation, { status: "ready", label, value })) this.release(value);
      },
      (reason: unknown) => { this.finish(operation, { status: "failed", label, message: errorMessage(reason) }); },
    );
    if (!this.current(operation)) this.stopTask(operation);
    return true;
  }

  cancel(): boolean {
    const operation = this.active;
    if (!operation || !operation.cancellable || this.disposed) return false;
    operation.abandoned = true;
    this.active = null;
    this.clearTimer(operation);
    this.stopTask(operation);
    if (!this.active) this.update({ status: "cancelled", label: operation.label });
    return true;
  }
  /** Source change: release preview resources and ignore old results. */
  reset(): void {
    if (this.disposed) return;
    const operation = this.active;
    this.active = null;
    if (operation) {
      operation.abandoned = true;
      this.clearTimer(operation);
      this.stopTask(operation);
    }
    this.update({ status: "idle" });
  }
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    const operation = this.active;
    this.active = null;
    if (operation) {
      operation.abandoned = true;
      this.clearTimer(operation);
      this.stopTask(operation);
    }
    this.update({ status: "idle" });
  }
}

/** Shared guard for native report/card dialogs on the same desktop backend.
 * Native dialogs are not cancellable from JavaScript; releasing a component
 * must not let a second save dialog overlap the first one. */
export class NativeSaveGate {
  private active = false;
  get busy(): boolean { return this.active; }
  run<T>(save: () => Promise<T>): Promise<T> {
    if (this.active) return Promise.reject(new Error("Another native save dialog is already open."));
    this.active = true;
    let result: Promise<T>;
    try { result = Promise.resolve(save()); }
    catch (reason) { this.active = false; return Promise.reject(reason); }
    return result.finally(() => { this.active = false; });
  }
}
const nativeSaveGates = new WeakMap<object, NativeSaveGate>();
export function nativeSaveGate(owner: object): NativeSaveGate {
  let gate = nativeSaveGates.get(owner);
  if (!gate) { gate = new NativeSaveGate(); nativeSaveGates.set(owner, gate); }
  return gate;
}

/** Validate a preview's SVG framing before it is wrapped in a Blob. Rendering
 * uses an image object URL, never inline HTML or SVG injection. */
export function svgPreview(text: string): string {
  if (!/^\s*(?:<\?xml[^>]*>\s*)?<svg\b/i.test(text)) throw new Error("The analysis program did not return an SVG card.");
  return text;
}
export interface PreviewResource {
  svg: string;
  url: string;
}
export interface ObjectUrlFactory {
  createObjectURL(blob: Blob): string;
  revokeObjectURL(url: string): void;
}
export function createPreview(svg: string, urls: ObjectUrlFactory = URL): PreviewResource {
  const text = svgPreview(svg);
  return { svg: text, url: urls.createObjectURL(new Blob([text], { type: "image/svg+xml" })) };
}
export function releasePreview(preview: PreviewResource, urls: ObjectUrlFactory = URL): void {
  urls.revokeObjectURL(preview.url);
}

/** A promise-only native renderer cannot physically cancel. Its result is
 * still subject to the owning ReportOperation's lifetime. */
export function asReportTask<T>(result: Promise<T>): ReportTask<T> { return { result }; }
export function mapReportTask<T, U>(task: ReportTask<T>, map: (value: T) => U): ReportTask<U> {
  return { result: task.result.then(map), cancel: task.cancel ? () => task.cancel?.() : undefined };
}
