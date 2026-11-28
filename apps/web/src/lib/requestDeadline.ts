/** Injectable clock so request deadlines and polling can be tested without real delays. */
export interface RequestClock {
  set(callback: () => void, delay: number): number;
  clear(handle: number): void;
}

export const browserClock: RequestClock = {
  set: (callback, delay) => window.setTimeout(callback, delay),
  clear: (handle) => window.clearTimeout(handle),
};

/** A deadline ends observation of a request, not necessarily the native operation. */
export class RequestDeadlineError extends Error {
  override name = "RequestDeadlineError";
  constructor(readonly operation: string) {
    super(`${operation} did not respond in time.`);
  }
}

export class RequestAbandonedError extends Error {
  override name = "RequestAbandonedError";
  constructor() {
    super("The request is no longer being observed.");
  }
}

/**
 * Owns logical deadlines for one UI operation. Tauri commands cannot always be
 * aborted; a late resolution or rejection is consumed without publishing it.
 * Disposing a scope immediately releases its timers and pending observers.
 */
export class RequestScope {
  private readonly pending = new Set<() => void>();
  private closed = false;

  constructor(private readonly clock: RequestClock) {}

  run<T>(operation: string, delay: number, task: () => Promise<T>): Promise<T> {
    if (this.closed) return Promise.reject(new RequestAbandonedError());
    return new Promise<T>((resolve, reject) => {
      let settled = false;
      let timer: number | null = null;
      const finish = (complete: () => void) => {
        if (settled) return;
        settled = true;
        if (timer !== null) this.clock.clear(timer);
        this.pending.delete(abandon);
        complete();
      };
      const abandon = () => finish(() => reject(new RequestAbandonedError()));
      this.pending.add(abandon);
      timer = this.clock.set(
        () => finish(() => reject(new RequestDeadlineError(operation))),
        delay,
      );
      try {
        Promise.resolve(task()).then(
          (value) => finish(() => resolve(value)),
          (reason: unknown) => finish(() => reject(reason)),
        );
      } catch (reason) {
        finish(() => reject(reason));
      }
    });
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    for (const abandon of this.pending) abandon();
    this.pending.clear();
  }
}

export function errorMessage(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason);
}
