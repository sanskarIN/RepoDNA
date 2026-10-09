import { afterEach, describe, expect, it } from "vitest";
import {
  memoryStore,
  newest,
  openPointer,
  RECENT_LIMIT,
  rememberRecent,
  setOpenPointer,
  setRememberRecent,
  type RecentEntry,
} from "./recent";

function entry(id: string, openedAt: string): RecentEntry {
  return {
    id,
    name: `${id}.json`,
    repository: "example",
    generatedAt: "2026-10-01T00:00:00Z",
    openedAt,
    size: 10,
  };
}

afterEach(() => {
  window.sessionStorage.clear();
  window.localStorage.clear();
});

describe("recent analyses", () => {
  it("lists the newest first and keeps only the last few", async () => {
    const store = memoryStore();
    for (let day = 1; day <= RECENT_LIMIT + 2; day += 1) {
      await store.add(entry(`a${day}`, `2026-10-0${day}T00:00:00Z`), `text ${day}`);
    }
    const list = await store.list();
    expect(list).toHaveLength(RECENT_LIMIT);
    expect(list[0]?.id).toBe(`a${RECENT_LIMIT + 2}`);
    expect(await store.text("a1")).toBeNull();
    expect(await store.text(`a${RECENT_LIMIT + 2}`)).toBe(`text ${RECENT_LIMIT + 2}`);
  });

  it("refreshes an entry opened again instead of adding it twice", async () => {
    const store = memoryStore();
    await store.add(entry("a", "2026-10-01T00:00:00Z"), "one");
    await store.add(entry("b", "2026-10-02T00:00:00Z"), "two");
    await store.add(entry("a", "2026-10-03T00:00:00Z"));
    expect((await store.list()).map((item) => item.id)).toEqual(["a", "b"]);
    expect(await store.text("a")).toBe("one");
  });

  it("removes one entry or all of them", async () => {
    const store = memoryStore();
    await store.add(entry("a", "2026-10-01T00:00:00Z"), "one");
    await store.add(entry("b", "2026-10-02T00:00:00Z"), "two");
    await store.remove("b");
    expect((await store.list()).map((item) => item.id)).toEqual(["a"]);
    await store.clear();
    expect(await store.list()).toEqual([]);
    expect(await store.text("a")).toBeNull();
  });

  it("orders by when they were opened", () => {
    expect(
      newest([entry("old", "2026-01-01T00:00:00Z"), entry("new", "2026-02-01T00:00:00Z")]).map(
        (item) => item.id,
      ),
    ).toEqual(["new", "old"]);
  });

  it("remembers what is open in the tab, and ignores what it cannot read", () => {
    expect(openPointer()).toBeNull();
    setOpenPointer({ kind: "demo", file: "repodna.json", title: "RepoDNA" });
    expect(openPointer()).toEqual({ kind: "demo", file: "repodna.json", title: "RepoDNA" });
    setOpenPointer(null);
    expect(openPointer()).toBeNull();
    window.sessionStorage.setItem("repodna-open", "{not json");
    expect(openPointer()).toBeNull();
    window.sessionStorage.setItem("repodna-open", JSON.stringify({ kind: "file" }));
    expect(openPointer()).toBeNull();
  });

  it("keeps recent analyses unless turned off", () => {
    expect(rememberRecent()).toBe(true);
    setRememberRecent(false);
    expect(rememberRecent()).toBe(false);
    setRememberRecent(true);
    expect(rememberRecent()).toBe(true);
  });
});
