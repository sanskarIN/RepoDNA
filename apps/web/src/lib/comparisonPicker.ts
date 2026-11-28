import type { RepositoryDna } from "@repodna/schema";
import type { Backend, RepositorySummary, ScanSummary } from "./backend";
import type { DemoEntry } from "./demo";
import { browserClock, errorMessage, RequestScope, type RequestClock } from "./requestDeadline";

export const PICKER_DEADLINES = { inventory: 15_000, selection: 30_000 } as const;
export const REPOSITORY_PAGE_SIZE = 12;
export const SCAN_PAGE_SIZE = 8;

export interface ComparisonSide {
  dna: RepositoryDna;
  label: string;
  repositoryId?: string;
}

export interface ComparisonPickerState {
  repositories: RepositorySummary[];
  demos: DemoEntry[];
  inventoryLoading: boolean;
  inventoryError: string | null;
  selection: string | null;
  selectionError: string | null;
}

export const EMPTY_COMPARISON_PICKER: ComparisonPickerState = {
  repositories: [],
  demos: [],
  inventoryLoading: false,
  inventoryError: null,
  selection: null,
  selectionError: null,
};

type InventoryBackend = Pick<Backend, "repositories" | "artifact">;

export function otherStoredScans(scans: readonly ScanSummary[], currentScanId?: string | null): ScanSummary[] {
  // The store uses analysisMetadata.id as its scan identifier. Distinct scans
  // may share the same second, so timestamps must not be used as identities.
  return scans.filter((scan) => !currentScanId || scan.id !== currentScanId);
}

export function matchingRepositories(
  repositories: readonly RepositorySummary[],
  currentRepositoryId: string | null,
  query: string,
): RepositorySummary[] {
  const needle = query.trim().toLowerCase();
  const seen = new Set<string>();
  return repositories.filter((repository) => {
    if (repository.id === currentRepositoryId || seen.has(repository.id) || repository.scans === 0) return false;
    seen.add(repository.id);
    return !needle || `${repository.name} ${repository.location}`.toLowerCase().includes(needle);
  });
}

export interface PickerPage<T> {
  items: T[];
  page: number;
  pages: number;
  total: number;
}

export function pickerPage<T>(items: readonly T[], requested: number, size: number): PickerPage<T> {
  if (!Number.isSafeInteger(size) || size <= 0) throw new Error("Invalid picker page size.");
  const pages = Math.max(1, Math.ceil(items.length / size));
  const page = Math.min(pages - 1, Math.max(0, Number.isSafeInteger(requested) ? requested : 0));
  return { items: items.slice(page * size, (page + 1) * size), page, pages, total: items.length };
}

/** Loads lists and one explicitly selected baseline. Each list/selection scope
 * is independent: a slow list must not block file selection, and a previous
 * selection cannot overwrite a later one. No writes or network retries occur. */
export class ComparisonPickerController {
  private stateValue = EMPTY_COMPARISON_PICKER;
  private inventory: RequestScope | null = null;
  private selection: RequestScope | null = null;
  private disposed = false;

  constructor(
    private readonly backend: InventoryBackend | null,
    private readonly listDemos: () => Promise<DemoEntry[]>,
    private readonly onPick: (side: ComparisonSide) => void,
    private readonly publish: (state: ComparisonPickerState) => void,
    private readonly clock: RequestClock = browserClock,
  ) {}

  get state(): ComparisonPickerState {
    return this.stateValue;
  }

  private update(change: Partial<ComparisonPickerState>): void {
    if (this.disposed) return;
    this.stateValue = { ...this.stateValue, ...change };
    this.publish(this.stateValue);
  }

  dispose(): void {
    this.disposed = true;
    this.inventory?.close();
    this.selection?.close();
    this.inventory = null;
    this.selection = null;
  }

  async loadInventory(): Promise<void> {
    if (this.disposed || this.inventory) return;
    const scope = new RequestScope(this.clock);
    this.inventory = scope;
    this.update({ inventoryLoading: true, inventoryError: null });
    const demos = scope.run("Demo listing", PICKER_DEADLINES.inventory, this.listDemos);
    const repositories = this.backend
      ? scope.run("Stored repository listing", PICKER_DEADLINES.inventory, () => this.backend!.repositories())
      : Promise.resolve([] as RepositorySummary[]);
    const [demosResult, repositoryResult] = await Promise.allSettled([demos, repositories]);
    if (this.disposed || this.inventory !== scope) return;
    const errors: string[] = [];
    const change: Partial<ComparisonPickerState> = { inventoryLoading: false };
    if (demosResult.status === "fulfilled") change.demos = demosResult.value;
    else errors.push(errorMessage(demosResult.reason));
    if (repositoryResult.status === "fulfilled") change.repositories = repositoryResult.value;
    else errors.push(errorMessage(repositoryResult.reason));
    change.inventoryError = errors.length ? errors.join("; ") : null;
    this.inventory = null;
    scope.close();
    this.update(change);
  }

  cancelSelection(): void {
    if (!this.selection) return;
    const scope = this.selection;
    this.selection = null;
    scope.close();
    this.update({ selection: null, selectionError: null });
  }

  async select(
    label: string,
    load: () => Promise<RepositoryDna>,
    repositoryId?: string,
    expectedScanId?: string,
  ): Promise<void> {
    if (this.disposed) return;
    this.cancelSelection();
    const scope = new RequestScope(this.clock);
    this.selection = scope;
    this.update({ selection: label, selectionError: null });
    try {
      const dna = await scope.run("Comparison analysis", PICKER_DEADLINES.selection, load);
      if (this.disposed || this.selection !== scope) return;
      // Legacy artifacts may have no embedded analysis ID, but a conflicting
      // nonempty ID is never acceptable for an explicitly requested scan.
      if (expectedScanId && dna.analysisMetadata.id && dna.analysisMetadata.id !== expectedScanId) {
        throw new Error("The backend returned a different stored analysis.");
      }
      this.onPick({ dna, label, repositoryId });
    } catch (reason) {
      if (this.disposed || this.selection !== scope) return;
      this.update({ selectionError: `Could not open ${label}: ${errorMessage(reason)}` });
    } finally {
      if (!this.disposed && this.selection === scope) {
        this.selection = null;
        scope.close();
        this.update({ selection: null });
      }
    }
  }

  selectRepository(repository: RepositorySummary): Promise<void> {
    if (!this.backend || repository.scans === 0) return Promise.resolve();
    return this.select(repository.name, () => this.backend!.artifact(repository.id), repository.id);
  }

  selectScan(repositoryId: string, scan: ScanSummary, label: string): Promise<void> {
    if (!this.backend) return Promise.resolve();
    return this.select(label, () => this.backend!.artifact(repositoryId, scan.id), repositoryId, scan.id);
  }
}
