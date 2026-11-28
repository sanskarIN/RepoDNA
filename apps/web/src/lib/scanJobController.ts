import type { Backend, JobState, RemoteRevision } from "./backend";
import {
  browserClock,
  errorMessage,
  RequestDeadlineError,
  RequestScope,
  type RequestClock,
} from "./requestDeadline";

export type ScanBackend = Pick<Backend, "startScan" | "job" | "cancelJob">;
export type OpenScanResult = (repositoryId: string, scanId?: string) => void | Promise<void>;

const POLL_DELAY = 400;
const MAX_FAILURES = 3;
export const SCAN_REQUEST_DEADLINES = {
  start: 30_000,
  poll: 10_000,
  cancel: 15_000,
} as const;

export interface ScanJobSnapshot {
  job: JobState | null;
  starting: boolean;
  busy: boolean;
  cancelling: boolean;
  paused: boolean;
  error: string | null;
  pollError: string | null;
}

export const EMPTY_SCAN_JOB: ScanJobSnapshot = {
  job: null,
  starting: false,
  busy: false,
  cancelling: false,
  paused: false,
  error: null,
  pollError: null,
};

interface ActiveScan {
  id: string | null;
  scope: RequestScope;
  timer: number | null;
  polling: boolean;
  failures: number;
  cancelling: boolean;
  opening: boolean;
}

/**
 * Single-flight lifecycle shared by the native scan form and recovery entry.
 * The job identity is fixed after launch. A cancellation acknowledgement is
 * not terminal, and a poll timeout does not mean the analysis stopped.
 */
export class ScanJobController {
  private active: ActiveScan | null = null;
  private disposed = false;
  private snapshot = EMPTY_SCAN_JOB;

  constructor(
    private readonly backend: ScanBackend,
    private readonly openResult: OpenScanResult,
    private readonly publish: (snapshot: ScanJobSnapshot) => void,
    private readonly clock: RequestClock = browserClock,
  ) {}

  get state(): ScanJobSnapshot {
    return this.snapshot;
  }

  private update(change: Partial<ScanJobSnapshot>): void {
    if (this.disposed) return;
    this.snapshot = { ...this.snapshot, ...change };
    this.publish(this.snapshot);
  }

  private current(scan: ActiveScan): boolean {
    return !this.disposed && this.active === scan;
  }

  private clearTimer(scan: ActiveScan): void {
    if (scan.timer !== null) this.clock.clear(scan.timer);
    scan.timer = null;
  }

  private release(scan: ActiveScan): void {
    if (this.active !== scan) return;
    this.active = null;
    this.clearTimer(scan);
    scan.scope.close();
  }

  dispose(): void {
    this.disposed = true;
    if (this.active) this.release(this.active);
  }

  private verifyIdentity(scan: ActiveScan, next: JobState): void {
    if (!next.id || (scan.id !== null && scan.id !== next.id)) {
      throw new Error("The backend returned a different analysis job.");
    }
  }

  private accept(scan: ActiveScan, next: JobState): void {
    if (!this.current(scan)) return;
    this.verifyIdentity(scan, next);
    scan.id = next.id;
    scan.failures = 0;
    if (next.status === "running") {
      this.update({ job: next, starting: false, pollError: null, paused: false });
      this.schedule(scan, POLL_DELAY);
      return;
    }

    this.clearTimer(scan);
    scan.scope.close();
    // Keep the operation reserved while its result is being opened. Otherwise
    // a second launch can race navigation or receive the first result's error.
    if (next.status === "completed" && next.repositoryId) {
      scan.opening = true;
      this.update({
        job: next,
        starting: false,
        cancelling: false,
        pollError: null,
        paused: false,
      });
      void this.open(scan, next.repositoryId, next.scanId ?? undefined);
      return;
    }

    this.release(scan);
    this.update({
      job: next,
      starting: false,
      busy: false,
      cancelling: false,
      pollError: null,
      paused: false,
      error:
        next.status === "completed"
          ? "Analysis finished but no saved result is available. Check its warnings or local storage."
          : next.status === "failed"
            ? (next.error ?? "The analysis failed.")
            : null,
    });
  }

  private async open(scan: ActiveScan, repositoryId: string, scanId?: string): Promise<void> {
    let failure: string | null = null;
    try {
      await this.openResult(repositoryId, scanId);
    } catch (reason) {
      failure = errorMessage(reason);
    }
    if (!this.current(scan)) return;
    this.release(scan);
    this.update({ busy: false, error: failure });
  }

  private schedule(scan: ActiveScan, delay: number): void {
    if (
      !this.current(scan) ||
      !scan.id ||
      scan.opening ||
      scan.polling ||
      scan.timer !== null
    ) {
      return;
    }
    scan.timer = this.clock.set(() => {
      scan.timer = null;
      void this.poll(scan);
    }, delay);
  }

  private async poll(scan: ActiveScan): Promise<void> {
    if (!this.current(scan) || !scan.id || scan.opening || scan.polling) return;
    const id = scan.id;
    scan.polling = true;
    try {
      const next = await scan.scope.run("Progress check", SCAN_REQUEST_DEADLINES.poll, () =>
        this.backend.job(id),
      );
      if (!this.current(scan)) return;
      scan.polling = false;
      this.accept(scan, next);
    } catch (reason) {
      if (!this.current(scan)) return;
      scan.polling = false;
      scan.failures += 1;
      const paused = scan.failures >= MAX_FAILURES;
      this.update({ pollError: errorMessage(reason), paused });
      if (!paused) this.schedule(scan, POLL_DELAY * 2 ** scan.failures);
    }
  }

  private async launch(operation: () => Promise<JobState>, expectedId?: string): Promise<void> {
    if (this.disposed || this.active) return;
    const scan: ActiveScan = {
      id: expectedId ?? null,
      scope: new RequestScope(this.clock),
      timer: null,
      polling: false,
      failures: 0,
      cancelling: false,
      opening: false,
    };
    this.active = scan;
    this.update({ ...EMPTY_SCAN_JOB, starting: true, busy: true });
    try {
      const next = await scan.scope.run(
        expectedId === undefined ? "Analysis submission" : "Job recovery",
        SCAN_REQUEST_DEADLINES.start,
        operation,
      );
      if (this.current(scan)) this.accept(scan, next);
    } catch (reason) {
      if (!this.current(scan)) return;
      this.release(scan);
      // A start request may have reached the server even if its response was
      // lost. Never retry a write automatically or claim it did not run.
      const error =
        reason instanceof RequestDeadlineError && expectedId === undefined
          ? `${reason.message} It may still have started. Check recent native analyses before submitting again.`
          : errorMessage(reason);
      this.update({ starting: false, busy: false, error });
    }
  }

  start(input: string, profile: string, revision?: RemoteRevision): Promise<void> {
    return this.launch(() => this.backend.startScan(input, profile, revision));
  }

  attach(id: string): Promise<void> {
    return this.launch(() => this.backend.job(id), id);
  }

  resume(): void {
    const scan = this.active;
    if (!scan || !this.current(scan) || !scan.id || scan.opening || this.snapshot.starting) return;
    scan.failures = 0;
    this.update({ paused: false, pollError: null });
    this.clearTimer(scan);
    this.schedule(scan, 0);
  }

  async stop(): Promise<void> {
    const scan = this.active;
    if (
      !scan ||
      !this.current(scan) ||
      !scan.id ||
      scan.opening ||
      scan.cancelling ||
      this.snapshot.starting
    ) {
      return;
    }
    const id = scan.id;
    scan.cancelling = true;
    this.update({ cancelling: true, error: null });
    try {
      await scan.scope.run("Cancellation request", SCAN_REQUEST_DEADLINES.cancel, () =>
        this.backend.cancelJob(id),
      );
      if (this.current(scan) && !scan.opening) this.resume();
    } catch (reason) {
      if (!this.current(scan) || scan.opening) return;
      scan.cancelling = false;
      this.update({ cancelling: false, error: errorMessage(reason) });
      // A timed-out acknowledgement does not imply cancellation failed. The
      // normal progress loop is still active, or can be resumed manually.
    }
  }
}
