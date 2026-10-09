// Analysis files dropped on the page: anywhere to open them, on the start page's drop zone,
// and on the Compare view's drop zone to compare with them. And the file an installed web
// version is started with.

import { act } from "react";
import { describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { parseArtifact } from "@repodna/schema";
import { App } from "../App";
import { memoryStore, type RecentStore } from "../lib/recent";
import type { Dataset } from "../state";
import demoText from "../../public/demo/repodna.json?raw";

function analysisFile(name = "repodna.json"): File {
  return new File([demoText], name, { type: "application/json" });
}

/** What a drag of `files` carries, as the page sees it. */
function carrying(...files: File[]) {
  return { dataTransfer: { types: ["Files"], files, dropEffect: "none" } };
}

/** A store that counts the analyses added to it. */
function countingStore(): RecentStore & { added: number } {
  const store = memoryStore();
  const counting = {
    ...store,
    added: 0,
    async add(...args: Parameters<RecentStore["add"]>) {
      counting.added += 1;
      await store.add(...args);
    },
  };
  return counting;
}

async function start(path: string, options: { initial?: Dataset; store?: RecentStore } = {}) {
  window.location.hash = `#${path}`;
  await act(async () => {
    render(<App initial={options.initial ?? null} store={options.store ?? memoryStore()} />);
  });
}

function demo(): Dataset {
  return {
    dna: parseArtifact(demoText).artifact,
    origin: { kind: "demo", title: "RepoDNA", file: "repodna.json" },
    warnings: [],
  };
}

describe("dropping files", () => {
  it("shows where to drop while a file is dragged over the page", async () => {
    await start("/settings");
    const main = await screen.findByRole("main");
    fireEvent.dragEnter(main, carrying(analysisFile()));
    expect(screen.getByText("Drop the file to open it")).toBeTruthy();
    fireEvent.dragLeave(main, carrying(analysisFile()));
    expect(screen.queryByText("Drop the file to open it")).toBeNull();
  });

  it("stops showing where to drop once the mouse moves again", async () => {
    await start("/settings");
    const main = await screen.findByRole("main");
    fireEvent.dragEnter(main, carrying(analysisFile()));
    expect(screen.getByText("Drop the file to open it")).toBeTruthy();
    fireEvent.mouseMove(main);
    expect(screen.queryByText("Drop the file to open it")).toBeNull();
  });

  it("does not offer to open text dragged on the page", async () => {
    await start("/settings");
    const main = await screen.findByRole("main");
    fireEvent.dragEnter(main, { dataTransfer: { types: ["text/plain"], files: [] } });
    expect(screen.queryByText("Drop the file to open it")).toBeNull();
  });

  it("opens a file dropped anywhere", async () => {
    await start("/settings");
    const main = await screen.findByRole("main");
    fireEvent.dragEnter(main, carrying(analysisFile()));
    await act(async () => {
      fireEvent.drop(main, carrying(analysisFile()));
    });
    await waitFor(() => expect(window.location.hash).toBe("#/overview"));
    expect(screen.queryByText("Drop the file to open it")).toBeNull();
    expect(screen.getAllByText(/^Snapshot from repodna\.json/).length).toBeGreaterThan(0);
  });

  it("says why a dropped file could not be opened", async () => {
    await start("/settings");
    const main = await screen.findByRole("main");
    await act(async () => {
      fireEvent.drop(main, carrying(new File(["not an analysis"], "notes.txt")));
    });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toMatch(/^Could not open notes\.txt: /);
    expect(window.location.hash).toBe("#/settings");
    fireEvent.click(within(alert).getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("opens a file dropped on the start page's drop zone once", async () => {
    const store = countingStore();
    await start("/", { store });
    const zone = (await screen.findByText("Drop a .repodna or repodna.json file here")).closest(
      ".dropzone",
    ) as HTMLElement;
    fireEvent.dragEnter(zone, carrying(analysisFile()));
    fireEvent.dragOver(zone, carrying(analysisFile()));
    // The drop zone shows what a drop does there.
    expect(screen.queryByText("Drop the file to open it")).toBeNull();
    await act(async () => {
      fireEvent.drop(zone, carrying(analysisFile()));
    });
    await waitFor(() => expect(window.location.hash).toBe("#/overview"));
    await waitFor(() => expect(store.added).toBe(1));
  });

  it("compares with a file dropped on the Compare view's drop zone", async () => {
    await start("/compare", { initial: demo() });
    const zone = (await screen.findByText("Drop an analysis file here to compare with it")).closest(
      ".dropzone",
    ) as HTMLElement;
    await act(async () => {
      fireEvent.drop(zone, carrying(analysisFile("other.json")));
    });
    expect(
      await screen.findByRole("heading", { name: "This analysis and other.json" }),
    ).toBeTruthy();
    expect(window.location.hash).toBe("#/compare");
  });

  it("offers only a file to compare with when the demo is open", async () => {
    await start("/compare", { initial: demo() });
    expect(await screen.findByRole("heading", { name: "A file" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "A file or demo" })).toBeNull();
  });

  it("opens the file an installed web version is started with", async () => {
    type Consumer = (params: { files: unknown[] }) => void;
    let consumer: Consumer | undefined;
    const launching = window as { launchQueue?: unknown };
    launching.launchQueue = { setConsumer: (given: Consumer) => (consumer = given) };
    try {
      await start("/");
      await waitFor(() => expect(consumer).toBeDefined());
      await act(async () => {
        consumer?.({
          files: [{ kind: "file", getFile: async () => analysisFile("launched.repodna") }],
        });
      });
      await waitFor(() => expect(window.location.hash).toBe("#/overview"));
      expect(screen.getAllByText(/^Snapshot from launched\.repodna/).length).toBeGreaterThan(0);
    } finally {
      delete launching.launchQueue;
    }
  });
});
