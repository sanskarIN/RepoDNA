import type { Backend, HistoryPage, ScanSummary } from "./backend";
import { browserClock, errorMessage, RequestScope, type RequestClock } from "./requestDeadline";

export type HistoryBackend = Pick<Backend, "history" | "repository">;
export const HISTORY_DEADLINE = 15_000;

export interface HistoryState {
  id: string | null;
  scans: ScanSummary[] | null;
  cursor: string | null;
  loading: boolean;
  error: string | null;
}

export const EMPTY_HISTORY: HistoryState = {
  id: null,
  scans: null,
  cursor: null,
  loading: false,
  error: null,
};

/** Owns a single selected repository's paginated history. Cursor and scan
 * indexes are committed only after a successful page; failed pages are retryable. */
export class StoredHistoryController {
  private snapshot = EMPTY_HISTORY;
  private request: RequestScope | null = null;
  private disposed = false;
  private readonly cursors = new Set<string>();
  private readonly scanIds = new Set<string>();

  constructor(
    private readonly backend: HistoryBackend,
    private readonly publish: (state: HistoryState) => void,
    private readonly clock: RequestClock = browserClock,
  ) {}

  get state(): HistoryState {
    return this.snapshot;
  }

  private update(state: HistoryState): void {
    if (this.disposed) return;
    this.snapshot = state;
    this.publish(state);
  }

  private abandon(): void {
    this.request?.close();
    this.request = null;
  }

  private clearIndexes(): void {
    this.cursors.clear();
    this.scanIds.clear();
  }

  reset(): void {
    this.abandon();
    this.clearIndexes();
    this.update(EMPTY_HISTORY);
  }

  dispose(): void {
    this.disposed = true;
    this.abandon();
    this.clearIndexes();
  }

  private current(scope: RequestScope): boolean {
    return !this.disposed && this.request === scope;
  }

  private async fetchPage(id: string, cursor: string | null): Promise<HistoryPage> {
    if (this.backend.history) return this.backend.history(id, cursor ?? undefined);
    const detail = await this.backend.repository(id);
    // Older backends may not implement pagination, but their response is still
    // required to identify the requested repository if the summary is present.
    if (detail.repository && detail.repository.id !== id) {
      throw new Error("The backend returned history for a different repository.");
    }
    return { repositoryId: id, scans: detail.scans, nextCursor: null };
  }

  private validatePage(page: HistoryPage, id: string): ScanSummary[] {
    if (page.repositoryId !== id) {
      throw new Error("The backend returned history for a different repository.");
    }
    const pageIds = new Set<string>();
    const added: ScanSummary[] = [];
    for (const scan of page.scans) {
      if (this.scanIds.has(scan.id) || pageIds.has(scan.id)) continue;
      pageIds.add(scan.id);
      added.push(scan);
    }
    if (page.nextCursor !== null && (this.cursors.has(page.nextCursor) || added.length === 0)) {
      throw new Error("History pagination did not advance. Refresh the history to retry.");
    }
    return added;
  }

  private async load(id: string, append: boolean): Promise<void> {
    if (this.disposed) return;
    const previous = this.snapshot;
    if (append && (previous.id !== id || previous.loading || !previous.cursor)) return;
    this.abandon();
    if (!append) this.clearIndexes();
    const cursor = append ? previous.cursor : null;
    const scope = new RequestScope(this.clock);
    this.request = scope;
    const initial: HistoryState = {
      id,
      scans: append ? previous.scans : null,
      cursor,
      loading: true,
      error: null,
    };
    this.update(initial);
    try {
      const page = await scope.run("Stored history request", HISTORY_DEADLINE, () =>
        this.fetchPage(id, cursor),
      );
      if (!this.current(scope)) return;
      const added = this.validatePage(page, id);
      for (const scan of added) this.scanIds.add(scan.id);
      if (page.nextCursor !== null) this.cursors.add(page.nextCursor);
      this.update({
        id,
        scans: [...(initial.scans ?? []), ...added],
        cursor: page.nextCursor,
        loading: false,
        error: null,
      });
    } catch (reason) {
      if (!this.current(scope)) return;
      this.update({ ...initial, loading: false, error: errorMessage(reason) });
    } finally {
      if (this.current(scope)) this.abandon();
    }
  }

  select(id: string): void {
    if (this.snapshot.id !== id) void this.load(id, false);
  }

  toggle(id: string): void {
    if (this.snapshot.id === id) this.reset();
    else this.select(id);
  }

  more(): void {
    if (this.snapshot.id) void this.load(this.snapshot.id, true);
  }

  async refresh(id: string): Promise<void> {
    if (this.snapshot.id === id) await this.load(id, false);
  }
}
