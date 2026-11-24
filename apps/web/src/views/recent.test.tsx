// Analyses opened from files are kept in the browser: listed on the start page, and opened
// again when the page is reloaded.

import { act } from "react";
import { describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { App } from "../App";
import {
  memoryStore,
  openPointer,
  setOpenPointer,
  setRememberRecent,
  type RecentEntry,
  type RecentStore,
} from "../lib/recent";
import demoText from "../../public/demo/repodna.json?raw";

async function start(path: string, store: RecentStore) {
  window.location.hash = `#${path}`;
  await act(async () => {
    render(<App store={store} />);
  });
}

async function openFile(name: string) {
  const input = await screen.findByLabelText("Open an analysis file");
  await act(async () => {
    fireEvent.change(input, {
      target: { files: [new File([demoText], name, { type: "application/json" })] },
    });
  });
  await waitFor(() => expect(window.location.hash).toBe("#/overview"));
}

describe("recent analyses", () => {
  it("lists an opened file on the start page and opens it again from there", async () => {
    const store = memoryStore();
    await start("/", store);
    await openFile("repodna.json");
    await waitFor(async () => expect(await store.list()).toHaveLength(1));
    window.location.hash = "#/";
    const recent = await screen.findByRole("region", { name: "Recent analyses" });
    expect(within(recent).getByText("repodna.json")).toBeTruthy();
    await act(async () => {
      fireEvent.click(within(recent).getByRole("button", { name: "Open" }));
    });
    await waitFor(() => expect(window.location.hash).toBe("#/overview"));
  });

  it("opens the same analysis again after a reload, on the same view", async () => {
    const store = memoryStore();
    await start("/", store);
    await openFile("repodna.json");
    await waitFor(async () => {
      const entries = await store.list();
      expect(entries).toHaveLength(1);
      expect(openPointer()).toEqual({ kind: "file", id: entries[0]?.id });
    });
    cleanup();
    await start("/files", store);
    expect((await screen.findByRole("heading", { level: 1 })).textContent).toBe("Files");
    expect(screen.queryByText("No analysis is open")).toBeNull();
  });

  it("starts with nothing open after the analysis was closed", async () => {
    const store = memoryStore();
    await start("/", store);
    await openFile("repodna.json");
    await act(async () => {
      fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    });
    const input = await screen.findByRole("combobox", { name: "Search and run commands" });
    await act(async () => {
      fireEvent.change(input, { target: { value: "close this analysis" } });
    });
    await act(async () => {
      fireEvent.keyDown(input, { key: "Enter" });
    });
    cleanup();
    await start("/files", store);
    expect(await screen.findByText("No analysis is open")).toBeTruthy();
  });

  it("opens a recent analysis from search", async () => {
    const store = memoryStore();
    await start("/", store);
    await openFile("repodna.json");
    await waitFor(async () => expect(await store.list()).toHaveLength(1));
    window.location.hash = "#/settings";
    await act(async () => {
      fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    });
    const input = await screen.findByRole("combobox", { name: "Search and run commands" });
    await act(async () => {
      fireEvent.change(input, { target: { value: "recent analysis" } });
    });
    expect(screen.getByRole("option", { name: /RepoDNA \(repodna\.json\)/ })).toBeTruthy();
    await act(async () => {
      fireEvent.keyDown(input, { key: "Enter" });
    });
    await waitFor(() => expect(window.location.hash).toBe("#/overview"));
  });

  it("does not reopen a recently selected file after it was deleted while loading", async () => {
    const actual = memoryStore();
    const saved: RecentEntry = {
      id: "pending-file",
      name: "pending-file.json",
      repository: "test",
      generatedAt: "2026-11-23T00:00:00Z",
      openedAt: "2026-11-24T00:00:00Z",
      size: 10,
    };
    await actual.add(saved, demoText);
    let release!: (text: string | null) => void;
    const pending = new Promise<string | null>((resolve) => {
      release = resolve;
    });
    let began!: () => void;
    const started = new Promise<void>((resolve) => {
      began = resolve;
    });
    const store: RecentStore = {
      ...actual,
      text(id) {
        if (id === saved.id) {
          began();
          return pending;
        }
        return actual.text(id);
      },
    };
    await start("/", store);
    const section = await screen.findByRole("region", { name: "Recent analyses" });
    await act(async () => {
      fireEvent.click(within(section).getByRole("button", { name: "Open" }));
    });
    await started;
    const remove = await screen.findByRole("button", {
      name: "Remove pending-file.json from recent analyses",
    });
    await act(async () => {
      fireEvent.click(remove);
    });
    await act(async () => {
      release(demoText);
      await pending;
    });
    await waitFor(async () => expect(await actual.list()).toEqual([]));
    expect(window.location.hash).toBe("#/");
    expect(openPointer()).toBeNull();
  });

  it("removes an analysis from the list", async () => {
    const store = memoryStore();
    await start("/", store);
    await openFile("repodna.json");
    window.location.hash = "#/";
    const remove = await screen.findByRole("button", {
      name: "Remove repodna.json from recent analyses",
    });
    await act(async () => {
      fireEvent.click(remove);
    });
    await waitFor(() =>
      expect(screen.queryByRole("heading", { name: "Recent analyses" })).toBeNull(),
    );
    expect(await store.list()).toEqual([]);
    expect(openPointer()).toBeNull();
  });

  it("keeps nothing when turned off in Settings", async () => {
    const store = memoryStore();
    await start("/", store);
    await openFile("repodna.json");
    window.location.hash = "#/settings";
    const keep = await screen.findByRole("checkbox", { name: /Keep the last \d+ analysis files/ });
    await act(async () => {
      fireEvent.click(keep);
    });
    await waitFor(async () => expect(await store.list()).toEqual([]));
    expect(await screen.findByText("None kept.")).toBeTruthy();
  });

  it("does not keep files when turned off before they are opened", async () => {
    setRememberRecent(false);
    const store = memoryStore();
    await start("/", store);
    await openFile("repodna.json");
    expect(await store.list()).toEqual([]);
    cleanup();
    await start("/files", store);
    expect(await screen.findByText("No analysis is open")).toBeTruthy();
  });

  it("finishes clearing cached recent files after an interrupted opt-out on reload", async () => {
    const store = memoryStore();
    await store.add(
      {
        id: "old",
        name: "old-analysis.json",
        repository: "test",
        generatedAt: "2026-11-23T01:00:00Z",
        openedAt: "2026-11-24T01:00:00Z",
        size: 10,
      },
      "old text",
    );
    setOpenPointer({ kind: "file", id: "old" });
    setRememberRecent(false);
    await start("/", store);
    await waitFor(async () => {
      expect(await store.list()).toEqual([]);
      expect(await store.text("old")).toBeNull();
    });
    expect(openPointer()).toBeNull();
    expect(screen.queryByRole("region", { name: "Recent analyses" })).toBeNull();
  });

  it("discards an outdated recent-history read after retention was disabled", async () => {
    const actual = memoryStore();
    const stale: RecentEntry = {
      id: "stale",
      name: "stale-analysis.json",
      repository: "test",
      generatedAt: "2026-11-23T01:00:00Z",
      openedAt: "2026-11-24T01:00:00Z",
      size: 10,
    };
    await actual.add(stale, "old text");
    let resolveFirst!: (entries: RecentEntry[]) => void;
    const firstRead = new Promise<RecentEntry[]>((resolve) => {
      resolveFirst = resolve;
    });
    let first = true;
    const store: RecentStore = {
      ...actual,
      list() {
        if (first) {
          first = false;
          return firstRead;
        }
        return actual.list();
      },
    };
    await start("/", store);
    window.location.hash = "#/settings";
    const keep = await screen.findByRole("checkbox", {
      name: /Keep the last \d+ analysis files/,
    });
    await act(async () => {
      fireEvent.click(keep);
    });
    await waitFor(async () => expect(await actual.list()).toEqual([]));
    await act(async () => {
      resolveFirst([stale]);
      await firstRead;
    });
    window.location.hash = "#/";
    await waitFor(() =>
      expect(screen.queryByRole("region", { name: "Recent analyses" })).toBeNull(),
    );
  });

  it("never persists a file reload pointer when browser storage fails", async () => {
    const store = memoryStore();
    const failedAdd = vi.fn(async () => {
      throw new Error("QuotaExceededError");
    });
    store.add = failedAdd;
    await start("/", store);
    await openFile("unsaved.json");
    await waitFor(() => expect(failedAdd).toHaveBeenCalledTimes(1));
    expect(openPointer()).toBeNull();
    expect((await store.list()).length).toBe(0);
    // The in-memory dataset still opens successfully even without persistence.
    expect(await screen.findByText(/This is a snapshot of the repository/)).toBeTruthy();
  });

  it("persists the tab pointer only after its recent-analysis save resolves", async () => {
    const actual = memoryStore();
    let release!: () => void;
    let began!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const started = new Promise<void>((resolve) => {
      began = resolve;
    });
    const store: RecentStore = {
      ...actual,
      async add(entry, text) {
        began();
        await gate;
        await actual.add(entry, text);
      },
    };
    await start("/", store);
    await openFile("pending.json");
    await started;
    expect(openPointer()).toBeNull();
    await act(async () => {
      release();
      await gate;
    });
    await waitFor(async () => {
      const items = await actual.list();
      expect(items).toHaveLength(1);
      expect(openPointer()).toEqual({ kind: "file", id: items[0]?.id });
    });
  });

  it("does not advertise reload restoration for a volatile browser fallback", async () => {
    const actual = memoryStore();
    const store: RecentStore = { ...actual, persistsAcrossReload: false };
    await start("/", store);
    await openFile("volatile.json");
    await waitFor(async () => expect(await actual.list()).toHaveLength(1));
    expect(openPointer()).toBeNull();
    window.location.hash = "#/settings";
    const keep = await screen.findByRole("checkbox", {
      name: /Keep the last \d+ analysis files/,
    });
    expect(keep.closest("label")?.textContent).toContain("during this visit");
    expect(keep.closest("label")?.textContent).not.toContain("after a reload");
  });

  it("recovers a later successful save after an earlier browser-storage write fails", async () => {
    const actual = memoryStore();
    let calls = 0;
    const store: RecentStore = {
      ...actual,
      async add(entry, text) {
        calls += 1;
        if (calls === 1) throw new Error("QuotaExceededError");
        await actual.add(entry, text);
      },
    };
    await start("/", store);
    await openFile("first-failed.json");
    await waitFor(() => expect(calls).toBe(1));
    expect(openPointer()).toBeNull();
    window.location.hash = "#/";
    await openFile("second-saved.json");
    await waitFor(async () => {
      const entries = await actual.list();
      expect(entries).toHaveLength(1);
      expect(entries[0]?.name).toBe("second-saved.json");
      expect(openPointer()).toEqual({ kind: "file", id: entries[0]?.id });
    });
  });

  it("does not resurrect retained data after disabling history during an in-flight save", async () => {
    const actual = memoryStore();
    let release!: () => void;
    let began!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const started = new Promise<void>((resolve) => {
      began = resolve;
    });
    const clearCalled = vi.fn(async () => {
      await actual.clear();
    });
    const store: RecentStore = {
      ...actual,
      clear: clearCalled,
      async add(entry, text) {
        began();
        await gate;
        await actual.add(entry, text);
      },
    };
    await start("/", store);
    await openFile("pending.json");
    await started;
    expect(openPointer()).toBeNull();
    window.location.hash = "#/settings";
    const keep = await screen.findByRole("checkbox", {
      name: /Keep the last \d+ analysis files/,
    });
    await act(async () => {
      fireEvent.click(keep);
    });
    await act(async () => {
      release();
      await gate;
    });
    await waitFor(async () => {
      expect(clearCalled).toHaveBeenCalledTimes(1);
      expect(await actual.list()).toEqual([]);
      expect(openPointer()).toBeNull();
    });
  });

  it("does not reattach an old session pointer after closing during a pending save", async () => {
    const actual = memoryStore();
    let release!: () => void;
    let began!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const started = new Promise<void>((resolve) => {
      began = resolve;
    });
    const store: RecentStore = {
      ...actual,
      async add(entry, text) {
        began();
        await gate;
        await actual.add(entry, text);
      },
    };
    await start("/", store);
    await openFile("closing.json");
    await started;
    await act(async () => {
      fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    });
    const input = await screen.findByRole("combobox", { name: "Search and run commands" });
    await act(async () => {
      fireEvent.change(input, { target: { value: "close this analysis" } });
    });
    await act(async () => {
      fireEvent.keyDown(input, { key: "Enter" });
    });
    expect(openPointer()).toBeNull();
    await act(async () => {
      release();
      await gate;
    });
    await waitFor(async () => expect(await actual.list()).toHaveLength(1));
    expect(openPointer()).toBeNull();
  });

  it("invalidates a previously saved tab pointer when recent history is cleared", async () => {
    const actual = memoryStore();
    await start("/", actual);
    await openFile("saved.json");
    await waitFor(async () => {
      const items = await actual.list();
      expect(items).toHaveLength(1);
      expect(openPointer()).toEqual({ kind: "file", id: items[0]?.id });
    });
    window.location.hash = "#/settings";
    const keep = await screen.findByRole("checkbox", {
      name: /Keep the last \d+ analysis files/,
    });
    await act(async () => {
      fireEvent.click(keep);
    });
    await waitFor(async () => expect(await actual.list()).toEqual([]));
    expect(openPointer()).toBeNull();
  });
});
