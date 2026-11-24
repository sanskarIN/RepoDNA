// Analyses opened in this browser: the one open in this tab, so that a reload keeps it, and
// the last few opened from files, so that they can be opened again from the start page.
// Everything stays in this browser's storage; nothing is sent anywhere.

/** What is open in this tab, enough to open it again after a reload. */
export type OpenPointer =
  | { kind: "stored"; repositoryId: string; scanId?: string; name: string }
  | { kind: "file"; id: string }
  | { kind: "demo"; file: string; title: string };

/** An analysis file opened recently, as the start page lists it. */
export interface RecentEntry {
  /** The analysis id. */
  id: string;
  /** The file name. */
  name: string;
  /** The analyzed repository. */
  repository: string;
  /** When the analysis was made (ISO 8601). */
  generatedAt: string;
  /** When it was last opened here (ISO 8601). */
  openedAt: string;
  /** Size of the file in bytes. */
  size: number;
}

/** Where recent analyses are kept. */
export interface RecentStore {
  /** False for transient browser fallbacks; injected stores may omit this flag. */
  readonly persistsAcrossReload?: boolean;
  list(): Promise<RecentEntry[]>;
  /** The text of an entry, or null when it is gone. */
  text(id: string): Promise<string | null>;
  /** Adds or refreshes an entry and keeps only the newest `RECENT_LIMIT`. */
  add(entry: RecentEntry, text?: string): Promise<void>;
  remove(id: string): Promise<void>;
  clear(): Promise<void>;
}

export const RECENT_LIMIT = 5;
/** Files larger than this are not kept: they would crowd out everything else. */
export const RECENT_MAX_BYTES = 64 * 1024 * 1024;

const OPEN_KEY = "repodna-open";
export const REMEMBER_KEY = "repodna-remember-recent";

/**
 * Stored metadata is external input, even when it comes from IndexedDB. Corrupted or
 * legacy records must not break sorting or prevent valid analyses from opening.
 */
export function validRecentEntry(value: unknown): value is RecentEntry {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const item = value as Record<string, unknown>;
  return (
    typeof item.id === "string" &&
    item.id.length > 0 &&
    typeof item.name === "string" &&
    item.name.length > 0 &&
    typeof item.repository === "string" &&
    typeof item.generatedAt === "string" &&
    Number.isFinite(Date.parse(item.generatedAt)) &&
    typeof item.openedAt === "string" &&
    Number.isFinite(Date.parse(item.openedAt)) &&
    typeof item.size === "number" &&
    Number.isSafeInteger(item.size) &&
    item.size >= 0 &&
    item.size <= RECENT_MAX_BYTES
  );
}

/** Newest first, at most `RECENT_LIMIT`; silently ignore damaged records. */
export function newest(entries: readonly RecentEntry[]): RecentEntry[] {
  return entries
    .filter(validRecentEntry)
    // ISO dates with differing UTC offsets do not sort chronologically as strings.
    .sort((a, b) => Date.parse(b.openedAt) - Date.parse(a.openedAt) || a.id.localeCompare(b.id))
    .slice(0, RECENT_LIMIT);
}

/** Keeps entries and texts in memory: for tests, and browsers without IndexedDB. */
export function memoryStore(): RecentStore {
  const entries = new Map<string, RecentEntry>();
  const texts = new Map<string, string>();
  return {
    async list() {
      return newest([...entries.values()]);
    },
    async text(id) {
      return texts.get(id) ?? null;
    },
    async add(entry, text) {
      if (!validRecentEntry(entry)) {
        throw new TypeError("Invalid recent-analysis metadata.");
      }
      entries.set(entry.id, entry);
      if (text !== undefined) {
        texts.set(entry.id, text);
      }
      const keep = new Set(newest([...entries.values()]).map((item) => item.id));
      for (const id of [...entries.keys()]) {
        if (!keep.has(id)) {
          entries.delete(id);
          texts.delete(id);
        }
      }
    },
    async remove(id) {
      entries.delete(id);
      texts.delete(id);
    },
    async clear() {
      entries.clear();
      texts.clear();
    },
  };
}

function request<T>(req: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error ?? new Error("The browser storage could not be read."));
  });
}

function done(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () =>
      reject(transaction.error ?? new Error("The browser storage could not be written."));
    transaction.onabort = () =>
      reject(transaction.error ?? new Error("The browser storage could not be written."));
  });
}

/** Keeps recent analyses in IndexedDB: entries in one store, the file texts in another. */
export function indexedDbStore(factory: IDBFactory): RecentStore {
  let database: Promise<IDBDatabase> | null = null;
  const open = () => {
    database ??= new Promise<IDBDatabase>((resolve, reject) => {
      const req = factory.open("repodna", 1);
      req.onupgradeneeded = () => {
        req.result.createObjectStore("recent", { keyPath: "id" });
        req.result.createObjectStore("texts", { keyPath: "id" });
      };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error ?? new Error("The browser storage is not available."));
    }).catch((error: unknown) => {
      // The browser can temporarily block IndexedDB (privacy settings, quota, or
      // an interrupted upgrade). Do not cache a rejected opening attempt forever.
      database = null;
      throw error;
    });
    return database;
  };
  const all = async (): Promise<RecentEntry[]> => {
    const db = await open();
    return request(
      db.transaction("recent").objectStore("recent").getAll() as IDBRequest<RecentEntry[]>,
    );
  };
  const removeIds = async (ids: readonly string[]) => {
    if (ids.length === 0) {
      return;
    }
    const db = await open();
    const transaction = db.transaction(["recent", "texts"], "readwrite");
    for (const id of ids) {
      transaction.objectStore("recent").delete(id);
      transaction.objectStore("texts").delete(id);
    }
    await done(transaction);
  };
  return {
    async list() {
      return newest(await all());
    },
    async text(id) {
      const db = await open();
      const row = (await request(db.transaction("texts").objectStore("texts").get(id))) as
        { id: string; text: string } | undefined;
      return typeof row?.text === "string" ? row.text : null;
    },
    async add(entry, text) {
      if (!validRecentEntry(entry)) {
        throw new TypeError("Invalid recent-analysis metadata.");
      }
      const db = await open();
      const transaction = db.transaction(["recent", "texts"], "readwrite");
      const recent = transaction.objectStore("recent");
      const texts = transaction.objectStore("texts");

      // Read, save, and evict in one transaction. A separate post-save prune
      // could race with another tab's add/remove and delete its newer entry.
      const completion = done(transaction);
      const scan = recent.getAll() as IDBRequest<RecentEntry[]>;
      // IDB transactions are active during request success handlers, but not
      // in arbitrary later Promise continuations. Queue every write here.
      scan.onsuccess = () => {
        const rows = scan.result;
        // Reopening an existing ID replaces its old metadata. Counting both
        // revisions as separate candidates would evict a sixth entry early.
        const candidates = rows.filter((row) => row?.id !== entry.id);
        const keep = new Set(newest([...candidates, entry]).map((item) => item.id));
        recent.put(entry);
        if (text !== undefined) {
          texts.put({ id: entry.id, text });
        }
        // Purge damaged legacy metadata as well as normal expired entries.
        for (const row of rows) {
          // A legacy row can have a valid IndexedDB key of the wrong TS type
          // (e.g. a number). It must not be retained just because it is hidden.
          if (row && row.id !== undefined && !keep.has(row.id)) {
            recent.delete(row.id);
            texts.delete(row.id);
          }
        }
        // A newly opened but old-dated entry might not belong in the top five.
        if (!keep.has(entry.id)) {
          recent.delete(entry.id);
          texts.delete(entry.id);
        }
      };
      await completion;
    },
    async remove(id) {
      await removeIds([id]);
    },
    async clear() {
      const db = await open();
      const transaction = db.transaction(["recent", "texts"], "readwrite");
      transaction.objectStore("recent").clear();
      transaction.objectStore("texts").clear();
      await done(transaction);
    },
  };
}

/** The store this browser offers: IndexedDB when it can be used, memory otherwise. */
export function browserStore(): RecentStore {
  try {
    if (typeof indexedDB !== "undefined" && indexedDB) {
      return indexedDbStore(indexedDB);
    }
  } catch {
    // Some browsers refuse storage in private windows; fall through to memory.
  }
  // With IndexedDB unavailable, this store is session-only. The UI must not
  // write a file pointer to sessionStorage that a reload cannot resolve.
  return { ...memoryStore(), persistsAcrossReload: false };
}

// Keep Settings effective for the current tab when storage is blocked.
let ephemeralRememberChoice: boolean | null = null;

/** Whether recent analyses are kept in this browser; on unless turned off in Settings. */
export function rememberRecent(): boolean {
  try {
    return window.localStorage.getItem(REMEMBER_KEY) !== "off";
  } catch {
    return ephemeralRememberChoice ?? true;
  }
}

export function setRememberRecent(on: boolean): void {
  ephemeralRememberChoice = on;
  try {
    if (on) {
      window.localStorage.removeItem(REMEMBER_KEY);
    } else {
      window.localStorage.setItem(REMEMBER_KEY, "off");
    }
  } catch {
    // The choice still applies until the page is reloaded.
  }
}

/** The analysis open in this tab, from the tab's session storage. */
export function openPointer(): OpenPointer | null {
  try {
    const raw = window.sessionStorage.getItem(OPEN_KEY);
    if (!raw) {
      return null;
    }
    const value: unknown = JSON.parse(raw);
    if (!value || typeof value !== "object" || Array.isArray(value)) {
      return null;
    }
    const pointer = value as Record<string, unknown>;
    switch (pointer.kind) {
      case "stored":
        if (
          typeof pointer.repositoryId !== "string" ||
          !pointer.repositoryId ||
          typeof pointer.name !== "string" ||
          (pointer.scanId !== undefined && typeof pointer.scanId !== "string")
        ) {
          return null;
        }
        return {
          kind: "stored",
          repositoryId: pointer.repositoryId,
          name: pointer.name,
          ...(typeof pointer.scanId === "string" ? { scanId: pointer.scanId } : {}),
        };
      case "file":
        return typeof pointer.id === "string" && pointer.id.length > 0
          ? { kind: "file", id: pointer.id }
          : null;
      case "demo":
        return typeof pointer.file === "string" &&
          pointer.file.length > 0 &&
          typeof pointer.title === "string"
          ? { kind: "demo", file: pointer.file, title: pointer.title }
          : null;
      default:
        return null;
    }
  } catch {
    return null;
  }
}

export function setOpenPointer(pointer: OpenPointer | null): void {
  try {
    if (pointer) {
      window.sessionStorage.setItem(OPEN_KEY, JSON.stringify(pointer));
    } else {
      window.sessionStorage.removeItem(OPEN_KEY);
    }
  } catch {
    // Without session storage, a reload starts over.
  }
}
