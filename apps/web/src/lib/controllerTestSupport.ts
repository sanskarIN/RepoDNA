import type { RequestClock } from "./requestDeadline";

/** Deterministic clock also accounts for the microtasks between timer callbacks. */
export class Clock implements RequestClock {
  now = 0;
  private next = 0;
  private readonly timers = new Map<number, { at: number; callback: () => void }>();

  get pending(): number {
    return this.timers.size;
  }

  set(callback: () => void, delay: number): number {
    const handle = ++this.next;
    this.timers.set(handle, { at: this.now + delay, callback });
    return handle;
  }

  clear(handle: number): void {
    this.timers.delete(handle);
  }

  async advance(delay: number): Promise<void> {
    const end = this.now + delay;
    await flush();
    while (true) {
      const next = [...this.timers.entries()]
        .filter(([, timer]) => timer.at <= end)
        .sort(([a, x], [b, y]) => x.at - y.at || a - b)[0];
      if (!next) break;
      const [handle, timer] = next;
      this.now = timer.at;
      this.timers.delete(handle);
      timer.callback();
      await flush();
    }
    this.now = end;
    await flush();
  }
}

export async function flush(): Promise<void> {
  for (let step = 0; step < 12; step += 1) await Promise.resolve();
}

export function deferred<T>() {
  let resolve: (value: T) => void = () => undefined;
  let reject: (reason: unknown) => void = () => undefined;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
