// Reports & export in the desktop app, which saves files through save dialogs.

import { act } from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { parseArtifact } from "@repodna/schema";
import { App } from "../App";
import { detectBackend, type Backend, type SourceReferences } from "../lib/backend";
import { memoryStore } from "../lib/recent";
import type { Dataset } from "../state";
import demoText from "../../public/demo/repodna.json?raw";

vi.mock("../lib/backend", async (original) => ({
  ...(await original<typeof import("../lib/backend")>()),
  detectBackend: vi.fn(),
}));

function desktop(listRemoteReferences?: Backend["listRemoteReferences"]) {
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
    listRemoteReferences,
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
    expect(startScan).toHaveBeenCalledWith(url, "standard", undefined);
  });
});

describe("desktop remote reference discovery", () => {
  const url = "https://github.com/org/repo";
  const listing: SourceReferences = {
    references: [
      { kind: "branch", name: "main", object: "a".repeat(40) },
      { kind: "branch", name: "feat/audio", object: "b".repeat(40) },
      { kind: "tag", name: "feat/audio", object: "c".repeat(40) },
    ],
    truncated: true,
  };

  it("only contacts the remote on request and uses the selected typed reference", async () => {
    const discover = vi.fn(async () => listing);
    const { startScan } = desktop(discover);
    window.location.hash = "#/";
    await act(async () => {
      render(<App store={memoryStore()} />);
    });
    const input = await screen.findByLabelText("Repository to analyze");
    fireEvent.change(input, { target: { value: url } });
    expect(discover).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Find branches and tags" }));
    });
    expect(discover).toHaveBeenCalledWith(url);
    fireEvent.change(screen.getByLabelText("Reference type"), { target: { value: "tag" } });
    fireEvent.change(screen.getByLabelText("Choose a reference"), { target: { value: "feat/audio" } });
    fireEvent.click(screen.getByRole("button", { name: "Use selected reference" }));
    expect((screen.getByLabelText("Tag name") as HTMLInputElement).value).toBe("feat/audio");
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Analyze" }));
    });
    expect(startScan).toHaveBeenCalledWith(url, "standard", { kind: "tag", value: "feat/audio" });
  });

  it("discards choices when the source changes while discovery is pending", async () => {
    let finish: (value: SourceReferences) => void = () => undefined;
    const discover = vi.fn(
      () => new Promise<SourceReferences>((resolve) => { finish = resolve; }),
    );
    desktop(discover);
    window.location.hash = "#/";
    await act(async () => {
      render(<App store={memoryStore()} />);
    });
    const input = await screen.findByLabelText("Repository to analyze");
    fireEvent.change(input, { target: { value: url } });
    fireEvent.click(screen.getByRole("button", { name: "Find branches and tags" }));
    fireEvent.change(input, { target: { value: "https://github.com/other/project" } });
    await act(async () => finish(listing));
    expect(screen.queryByLabelText("Choose a reference")).toBeNull();
    expect(discover).toHaveBeenCalledTimes(1);
  });

  it("keeps manual selection usable when listing fails and resets it for local input", async () => {
    const discover = vi.fn(async () => {
      throw new Error("Network unavailable");
    });
    desktop(discover);
    window.location.hash = "#/";
    await act(async () => {
      render(<App store={memoryStore()} />);
    });
    const input = await screen.findByLabelText("Repository to analyze");
    fireEvent.change(input, { target: { value: url } });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Find branches and tags" }));
    });
    expect(screen.getByText(/Network unavailable/)).toBeTruthy();
    const revisionKind = screen.getByLabelText("Source revision") as HTMLSelectElement;
    fireEvent.change(revisionKind, { target: { value: "branch" } });
    fireEvent.change(screen.getByLabelText("Branch name"), { target: { value: "--invalid" } });
    expect((screen.getByRole("button", { name: "Analyze" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByLabelText("Branch name"), { target: { value: "feat/audio" } });
    expect((screen.getByRole("button", { name: "Analyze" }) as HTMLButtonElement).disabled).toBe(false);
    fireEvent.change(input, { target: { value: "/local/project" } });
    expect(revisionKind.value).toBe("auto");
    expect(revisionKind.disabled).toBe(true);
    expect(screen.queryByLabelText("Branch name")).toBeNull();
  });
});
