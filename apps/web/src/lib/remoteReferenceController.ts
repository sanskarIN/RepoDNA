import type { SourceReference, SourceReferences, SourceRefQuery } from "./backend";
import { browserClock, errorMessage, RequestScope, type RequestClock } from "./requestDeadline";

export type ReferenceKind = SourceReference["kind"];
export type DiscoverReferences = (input: string, query?: SourceRefQuery) => Promise<SourceReferences>;
export const REFERENCE_REQUEST_DEADLINE = 25_000;
export const REFERENCE_PAGE_SIZE = 150;

export interface ReferenceSnapshot {
  listing: SourceReferences | null;
  scope: SourceRefQuery | null;
  loading: boolean;
  error: string | null;
  kind: ReferenceKind;
  query: string;
  page: number;
  selected: string;
}

export const EMPTY_REFERENCES: ReferenceSnapshot = {
  listing: null,
  scope: null,
  loading: false,
  error: null,
  kind: "branch",
  query: "",
  page: 0,
  selected: "",
};

/** Rust validates the trimmed UTF-8 byte length and Unicode control characters. */
export function referenceQueryError(query: string): string | null {
  const trimmed = query.trim();
  if (/[\x00-\x1f\x7f-\x9f]/.test(trimmed)) {
    return "Search must not contain control characters.";
  }
  if (new TextEncoder().encode(trimmed).length > 512) {
    return "Search must be at most 512 UTF-8 bytes.";
  }
  return null;
}

export interface ReferencePage {
  total: number;
  page: number;
  pages: number;
  references: SourceReference[];
  choice: SourceReference | undefined;
  /** Whether this listing can describe all results for the local filter. */
  complete: boolean;
}

export function referencePage(state: ReferenceSnapshot): ReferencePage {
  const needle = state.query.trim().toLowerCase();
  const matching = (state.listing?.references ?? []).filter(
    (item) => item.kind === state.kind && item.name.toLowerCase().includes(needle),
  );
  const pages = Math.max(1, Math.ceil(matching.length / REFERENCE_PAGE_SIZE));
  const page = Math.max(0, Math.min(state.page, pages - 1));
  const references = matching.slice(page * REFERENCE_PAGE_SIZE, (page + 1) * REFERENCE_PAGE_SIZE);
  // A remote search with a narrower substring cannot answer a broader local
  // filter. A result cap also prevents claiming completeness, even if every
  // retained result has already been displayed.
  const scopeKind = state.scope?.kind;
  const scopeNeedle = state.scope?.search?.trim().toLowerCase() ?? "";
  const complete =
    state.listing !== null &&
    !state.listing.truncated &&
    (!scopeKind || scopeKind === state.kind) &&
    needle.includes(scopeNeedle);
  return {
    total: matching.length,
    page,
    pages,
    references,
    choice: references.find((item) => item.name === state.selected),
    complete,
  };
}

/** Explicit remote discovery with a local paginated snapshot. Never initiates
 * network traffic while typing, switching namespaces, or changing pages. */
export class RemoteReferenceController {
  private snapshot = EMPTY_REFERENCES;
  private request: RequestScope | null = null;
  private disposed = false;

  constructor(
    private readonly input: string,
    private readonly discover: DiscoverReferences,
    private readonly publish: (snapshot: ReferenceSnapshot) => void,
    private readonly clock: RequestClock = browserClock,
  ) {}

  get state(): ReferenceSnapshot {
    return this.snapshot;
  }

  private update(change: Partial<ReferenceSnapshot>): void {
    if (this.disposed) return;
    this.snapshot = { ...this.snapshot, ...change };
    this.publish(this.snapshot);
  }

  dispose(): void {
    this.disposed = true;
    this.request?.close();
    this.request = null;
  }

  private current(request: RequestScope): boolean {
    return !this.disposed && this.request === request;
  }

  async load(query?: SourceRefQuery): Promise<void> {
    if (this.disposed || this.request) return;
    const error = referenceQueryError(query?.search ?? "");
    if (error) {
      this.update({ error });
      return;
    }
    const scope = query ? { ...query, search: query.search?.trim() } : null;
    const request = new RequestScope(this.clock);
    this.request = request;
    // Retain the last successful snapshot if the next search fails, but never
    // retain a selected reference across an advertisement refresh.
    this.update({ loading: true, error: null, selected: "" });
    try {
      const listing = await request.run("Remote reference discovery", REFERENCE_REQUEST_DEADLINE, () =>
        scope ? this.discover(this.input, scope) : this.discover(this.input),
      );
      if (this.current(request)) {
        this.update({ listing, scope, page: 0, selected: "", loading: false });
      }
    } catch (reason) {
      if (this.current(request)) {
        this.update({ loading: false, error: errorMessage(reason) });
      }
    } finally {
      if (this.current(request)) {
        request.close();
        this.request = null;
      }
    }
  }

  search(): Promise<void> {
    return this.load({ kind: this.snapshot.kind, search: this.snapshot.query });
  }

  setKind(kind: ReferenceKind): void {
    if (this.snapshot.loading) return;
    this.update({ kind, page: 0, selected: "" });
  }

  setQuery(query: string): void {
    if (this.snapshot.loading) return;
    this.update({ query, page: 0, selected: "" });
  }

  setPage(page: number): void {
    if (this.snapshot.loading || !Number.isInteger(page)) return;
    const { pages } = referencePage(this.snapshot);
    if (page < 0 || page >= pages) return;
    this.update({ page, selected: "" });
  }

  select(name: string): void {
    if (this.snapshot.loading) return;
    const { references } = referencePage(this.snapshot);
    if (name !== "" && !references.some((item) => item.name === name)) return;
    this.update({ selected: name });
  }
}
