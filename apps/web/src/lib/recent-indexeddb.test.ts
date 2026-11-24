import { describe, expect, it, vi } from "vitest";
import { indexedDbStore, memoryStore, newest, RECENT_LIMIT, type RecentEntry } from "./recent";

function entry(id: string, openedAt: string): RecentEntry {
  return {
    id,
    name: id + ".json",
    repository: "test-repository",
    generatedAt: "2026-10-01T00:00:00Z",
    openedAt,
    size: 16,
  };
}

/** A small asynchronous IndexedDB stand-in to check transaction boundaries.
 * The fake does not claim to reproduce the browser's concurrency scheduler. */
function fakeIndexedDb() {
  const rows = {
    recent: new Map<string, unknown>(),
    texts: new Map<string, unknown>(),
  };
  const transactions: Array<{ stores: string[]; mode: IDBTransactionMode }> = [];

  function resultRequest<T>(result: T): IDBRequest<T> {
    const req = {
      result,
      error: null,
      onsuccess: null as IDBRequest<T>["onsuccess"],
      onerror: null as IDBRequest<T>["onerror"],
    };
    queueMicrotask(() =>
      req.onsuccess?.call(req as unknown as IDBRequest<T>, new Event("success")),
    );
    return req as unknown as IDBRequest<T>;
  }

  const db = {
    createObjectStore: vi.fn(),
    transaction(names: string | string[], mode: IDBTransactionMode = "readonly") {
      const stores = typeof names === "string" ? [names] : [...names];
      transactions.push({ stores, mode });
      const tx = {
        oncomplete: null as IDBTransaction["oncomplete"],
        onerror: null as IDBTransaction["onerror"],
        onabort: null as IDBTransaction["onabort"],
        error: null,
        objectStore(name: keyof typeof rows) {
          const map = rows[name];
          return {
            getAll: () => resultRequest([...map.values()]),
            get: (id: string) => resultRequest(map.get(id)),
            put: (record: { id: string }) => {
              map.set(record.id, record);
              return resultRequest(undefined);
            },
            delete: (id: string) => {
              map.delete(id);
              return resultRequest(undefined);
            },
            clear: () => {
              map.clear();
              return resultRequest(undefined);
            },
          } as unknown as IDBObjectStore;
        },
      };
      setTimeout(
        () => tx.oncomplete?.call(tx as unknown as IDBTransaction, new Event("complete")),
        0,
      );
      return tx as unknown as IDBTransaction;
    },
  };
  const factory = {
    open: vi.fn(() => {
      const req = {
        result: db,
        onupgradeneeded: null as IDBOpenDBRequest["onupgradeneeded"],
        onsuccess: null as IDBOpenDBRequest["onsuccess"],
        onerror: null as IDBOpenDBRequest["onerror"],
        error: null,
      };
      queueMicrotask(() => {
        req.onupgradeneeded?.call(
          req as unknown as IDBOpenDBRequest,
          new Event("upgradeneeded") as IDBVersionChangeEvent,
        );
        req.onsuccess?.call(req as unknown as IDBOpenDBRequest, new Event("success"));
      });
      return req as unknown as IDBOpenDBRequest;
    }),
  };
  return { factory: factory as unknown as IDBFactory, rows, transactions };
}

describe("local recent-analysis persistence integrity", () => {
  it("sorts actual timestamps, not their different timezone spellings", () => {
    const older = entry("older", "2026-11-23T23:30:00Z");
    const newer = entry("newer", "2026-11-24T00:30:00+00:30");
    expect(newest([older, newer]).map((item) => item.id)).toEqual(["newer", "older"]);
    expect(
      newest([
        entry("z", "2026-11-24T00:00:00Z"),
        entry("a", "2026-11-24T01:00:00+01:00"),
      ]).map((item) => item.id),
    ).toEqual(["a", "z"]);
  });

  it("saves and evicts recent metadata and file text in one readwrite transaction", async () => {
    const fake = fakeIndexedDb();
    const store = indexedDbStore(fake.factory);
    for (let day = 1; day <= RECENT_LIMIT + 2; day += 1) {
      const before = fake.transactions.length;
      await store.add(
        entry("analysis-" + day, `2026-11-${String(day).padStart(2, "0")}T00:00:00Z`),
        "content " + day,
      );
      expect(fake.transactions.slice(before)).toEqual([
        { stores: ["recent", "texts"], mode: "readwrite" },
      ]);
    }
    expect(fake.rows.recent.size).toBe(RECENT_LIMIT);
    expect(fake.rows.texts.size).toBe(RECENT_LIMIT);
    expect((await store.list()).map((item) => item.id)).toEqual(
      ["analysis-7", "analysis-6", "analysis-5", "analysis-4", "analysis-3"],
    );
    expect(await store.text("analysis-1")).toBeNull();
    expect(await store.text("analysis-7")).toBe("content 7");
  });

  it("does not evict a sixth analysis when refreshing an existing ID at capacity", async () => {
    const fake = fakeIndexedDb();
    const store = indexedDbStore(fake.factory);
    for (let day = 1; day <= RECENT_LIMIT; day += 1) {
      await store.add(
        entry("analysis-" + day, `2026-11-${String(day).padStart(2, "0")}T00:00:00Z`),
        "original " + day,
      );
    }
    await store.add(entry("analysis-4", "2026-11-09T00:00:00Z"));
    expect((await store.list()).map((item) => item.id)).toEqual([
      "analysis-4",
      "analysis-5",
      "analysis-3",
      "analysis-2",
      "analysis-1",
    ]);
    expect(fake.rows.recent.size).toBe(RECENT_LIMIT);
    expect(fake.rows.texts.size).toBe(RECENT_LIMIT);
    expect(await store.text("analysis-4")).toBe("original 4");
    expect(await store.text("analysis-1")).toBe("original 1");
  });

  it("refreshes entries without erasing their existing saved text", async () => {
    const fake = fakeIndexedDb();
    const store = indexedDbStore(fake.factory);
    await store.add(entry("first", "2026-11-01T00:00:00Z"), "original");
    await store.add(entry("second", "2026-11-02T00:00:00Z"), "second text");
    await store.add(entry("first", "2026-11-03T00:00:00Z"));
    expect((await store.list()).map((item) => item.id)).toEqual(["first", "second"]);
    expect(await store.text("first")).toBe("original");
  });

  it("purges corrupt cached rows and rejects invalid new metadata consistently", async () => {
    const fake = fakeIndexedDb();
    const indexed = indexedDbStore(fake.factory);
    const memory = memoryStore();
    const invalid = { ...entry("invalid", "2026-11-01T00:00:00Z"), size: -1 };
    await expect(indexed.add(invalid, "bad")).rejects.toThrow(TypeError);
    await expect(memory.add(invalid, "bad")).rejects.toThrow(TypeError);
    expect(fake.rows.recent.size).toBe(0);
    fake.rows.recent.set("corrupt", { ...entry("corrupt", "broken"), openedAt: "bad" });
    fake.rows.texts.set("corrupt", { id: "corrupt", text: "stale" });
    await indexed.add(entry("valid", "2026-11-02T00:00:00Z"), "fresh");
    expect(fake.rows.recent.has("corrupt")).toBe(false);
    expect(fake.rows.texts.has("corrupt")).toBe(false);
    expect(await indexed.text("valid")).toBe("fresh");
    fake.rows.texts.set("valid", { id: "valid", text: 42 });
    expect(await indexed.text("valid")).toBeNull();
  });

  it("removes both stores together and clears retained data", async () => {
    const fake = fakeIndexedDb();
    const store = indexedDbStore(fake.factory);
    await store.add(entry("one", "2026-11-01T00:00:00Z"), "one");
    await store.add(entry("two", "2026-11-02T00:00:00Z"), "two");
    await store.remove("one");
    expect(fake.rows.recent.has("one")).toBe(false);
    expect(fake.rows.texts.has("one")).toBe(false);
    await store.clear();
    expect(fake.rows.recent.size).toBe(0);
    expect(fake.rows.texts.size).toBe(0);
  });
});
