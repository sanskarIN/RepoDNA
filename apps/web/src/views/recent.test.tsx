// Analyses opened from files are kept in the browser: listed on the start page, and opened
// again when the page is reloaded.

import { act } from "react";
import { describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { App } from "../App";
import { memoryStore, setRememberRecent, type RecentStore } from "../lib/recent";
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
    await waitFor(async () => expect(await store.list()).toHaveLength(1));
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
});
