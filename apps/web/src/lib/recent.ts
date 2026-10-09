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
const REMEMBER_KEY = "repodna-remember-recent";

/** Newest first, at most `RECENT_LIMIT`. */
export function newest(entries: readonly RecentEntry[]): RecentEntry[] {
  return [...entries].sort((a, b) => b.openedAt.localeCompare(a.openedAt)).slice(0, RECENT_LIMIT);
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
    database ??= new Promise((resolve, reject) => {
      const req = factory.open("repodna", 1);
      req.onupgradeneeded = () => {
        req.result.createObjectStore("recent", { keyPath: "id" });
        req.result.createObjectStore("texts", { keyPath: "id" });
      };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error ?? new Error("The browser storage is not available."));
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
      return row?.text ?? null;
    },
    async add(entry, text) {
      const db = await open();
      const transaction = db.transaction(["recent", "texts"], "readwrite");
      transaction.objectStore("recent").put(entry);
      if (text !== undefined) {
        transaction.objectStore("texts").put({ id: entry.id, text });
      }
      await done(transaction);
      const keep = new Set(newest(await all()).map((item) => item.id));
      await removeIds((await all()).map((item) => item.id).filter((id) => !keep.has(id)));
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
  return memoryStore();
}

/** Whether recent analyses are kept in this browser; on unless turned off in Settings. */
export function rememberRecent(): boolean {
  try {
    return window.localStorage.getItem(REMEMBER_KEY) !== "off";
  } catch {
    return true;
  }
}

export function setRememberRecent(on: boolean): void {
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
    const value = JSON.parse(raw) as Partial<OpenPointer> & { kind?: string };
    switch (value.kind) {
      case "stored":
        return typeof value.repositoryId === "string" ? (value as OpenPointer) : null;
      case "file":
        return typeof value.id === "string" ? (value as OpenPointer) : null;
      case "demo":
        return typeof value.file === "string" ? (value as OpenPointer) : null;
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
