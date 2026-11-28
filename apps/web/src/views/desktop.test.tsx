// Reports & export in the desktop app, which saves files through save dialogs.

import { act } from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { parseArtifact } from "@repodna/schema";
import { App } from "../App";
import { detectBackend, type Backend } from "../lib/backend";
import { memoryStore } from "../lib/recent";
import type { Dataset } from "../state";
import demoText from "../../public/demo/repodna.json?raw";

vi.mock("../lib/backend", async (original) => ({
  ...(await original<typeof import("../lib/backend")>()),
  detectBackend: vi.fn(),
}));

function desktop() {
  const saveReport = vi.fn(async () => "/home/me/repodna-repodna-2026-09-26.html");
  const saveCard = vi.fn(async () => "/home/me/repodna-repodna-2026-09-26-card-dark.png");
  const startScan = vi.fn(async () => {
    throw new Error("test scan stopped");
  });
  const backend = {
    kind: "desktop",
    session: async () => ({ version: "test", allowScans: true }),
    repositories: async () => [],
    card: async (_id: string, dark: boolean) =>
      `<svg xmlns="http://www.w3.org/2000/svg"><title>${dark ? "dark" : "light"}</title></svg>`,
    reportUrl: () => null,
    saveReport,
    saveCard,
    startScan,
  } as unknown as Backend;
  vi.mocked(detectBackend).mockResolvedValue({
    backend,
    session: { version: "test", allowScans: true },
    signInNeeded: false,
  });
  return { saveReport, saveCard, startScan };
}

function stored(): Dataset {
  return {
    dna: parseArtifact(demoText).artifact,
    origin: { kind: "stored", repositoryId: "repo-1", name: "RepoDNA" },
    warnings: [],
  };
}

describe("desktop reports", () => {
  it("names each report it saves, and saves the card as its preview shows it", async () => {
    URL.createObjectURL ??= () => "blob:card";
    URL.revokeObjectURL ??= () => undefined;
    const { saveReport, saveCard } = desktop();
    window.location.hash = "#/reports";
    await act(async () => {
      render(<App initial={stored()} store={memoryStore()} />);
    });
    for (const name of [
      "Save the HTML report…",
      "Save the Markdown report…",
      "Save the analysis as JSON…",
      "Save the full report folder…",
    ]) {
      expect(await screen.findByRole("button", { name })).toBeTruthy();
    }
    expect(screen.queryByRole("button", { name: "Save the DNA card…" })).toBeNull();

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Save the Markdown report…" }));
    });
    expect(saveReport).toHaveBeenCalledWith("repo-1", "markdown", {
      scan: undefined,
      theme: "professional",
      privacy: "local",
    });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Dark card" }));
    });
    await act(async () => {
      fireEvent.click(await screen.findByRole("button", { name: "Save PNG…" }));
    });
    expect(saveCard).toHaveBeenCalledWith("repo-1", true, true, undefined);
    await waitFor(() =>
      expect(
        screen.getByText("Saved to /home/me/repodna-repodna-2026-09-26-card-dark.png"),
      ).toBeTruthy(),
    );
  });
});

describe("desktop repository analysis", () => {
  it("blocks duplicate URLs and submits the corrected GitHub branch link", async () => {
    const { startScan } = desktop();
    window.location.hash = "#/";
    await act(async () => {
      render(<App store={memoryStore()} />);
    });
    const input = await screen.findByLabelText("Repository to analyze");
    const analyze = screen.getByRole("button", { name: "Analyze" });
    const url = "https://github.com/sanskarIN/latebrake/tree/feat/experimental-six-car-runtime";

    fireEvent.change(input, { target: { value: url + url } });
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(screen.getByText(/Multiple repository URLs detected/)).toBeTruthy();
    expect((analyze as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(analyze);
    expect(startScan).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Clear repository input" }));
    expect((input as HTMLInputElement).value).toBe("");
    fireEvent.change(input, { target: { value: url } });
    expect(input.getAttribute("aria-invalid")).toBeNull();
    expect(
      screen.getByText(/branch or tag candidate feat\/experimental-six-car-runtime/),
    ).toBeTruthy();
    expect((analyze as HTMLButtonElement).disabled).toBe(false);
    await act(async () => {
      fireEvent.click(analyze);
    });
    expect(startScan).toHaveBeenCalledWith(url, "standard");
  });
});
