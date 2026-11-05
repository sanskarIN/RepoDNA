import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { App } from "../App";
import {
  detectBackend,
  type Backend,
  type RepositorySummary,
  type ScanSummary,
} from "../lib/backend";
import { memoryStore } from "../lib/recent";

vi.mock("../lib/backend", async (original) => ({
  ...(await original<typeof import("../lib/backend")>()),
  detectBackend: vi.fn(),
}));

const repository: RepositorySummary = {
  id: "repo-1",
  name: "example-project",
  location: "/projects/example",
  kind: "local",
  scans: 1,
  firstScannedAt: "2026-10-28T00:00:00Z",
  lastScannedAt: "2026-10-28T00:00:00Z",
};

const scan: ScanSummary = {
  id: "scan-1",
  generatedAt: "2026-10-28T00:00:00Z",
  profile: "standard",
  revision: null,
  dnaHash: "dna",
  toolVersion: "1.5.0",
  files: 2,
  codeLines: 3,
  commits: 1,
  contributors: 1,
  findings: { critical: 0, warning: 0, attention: 0, info: 0, suppressed: 0 },
};

function fixture() {
  let exists = true;
  const deleteRepository = vi.fn(async () => {
    exists = false;
  });
  const deleteScan = vi.fn(async () => {
    exists = false;
  });
  const backend = {
    kind: "desktop",
    session: async () => ({ version: "test", allowScans: true }),
    repositories: async () => (exists ? [repository] : []),
    repository: async () => ({ repository, scans: exists ? [scan] : [] }),
    deleteRepository,
    deleteScan,
  } as unknown as Backend;
  vi.mocked(detectBackend).mockResolvedValue({
    backend,
    session: { version: "test", allowScans: true },
    signInNeeded: false,
  });
  return { deleteRepository, deleteScan };
}

async function start() {
  window.location.hash = "#/";
  await act(async () => {
    render(<App store={memoryStore()} />);
  });
  return screen.findByRole("region", { name: "Stored analyses" });
}

describe("deleting stored analyses", () => {
  beforeEach(() => {
    vi.mocked(detectBackend).mockReset();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("asks before deleting a repository and refreshes the stored list", async () => {
    const { deleteRepository } = fixture();
    const confirm = vi.spyOn(window, "confirm").mockReturnValueOnce(false).mockReturnValue(true);
    const region = await start();
    const remove = within(region).getByRole("button", {
      name: "Delete all stored analyses for example-project",
    });
    await act(async () => {
      fireEvent.click(remove);
    });
    expect(deleteRepository).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.click(remove);
    });
    expect(confirm).toHaveBeenCalledTimes(2);
    expect(deleteRepository).toHaveBeenCalledWith("repo-1");
    expect(await within(region).findByText(/None yet\. Analyze a repository above/)).toBeTruthy();
    expect(within(region).getByRole("status").textContent).toContain("Deleted stored analyses");
  });

  it("allows individual scan deletion after opening history", async () => {
    const { deleteScan } = fixture();
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const region = await start();
    await act(async () => {
      fireEvent.click(
        within(region).getByRole("button", {
          name: "Manage stored analyses for example-project",
        }),
      );
    });
    const button = await within(region).findByRole("button", {
      name: "Delete stored analysis scan-1",
    });
    await act(async () => {
      fireEvent.click(button);
    });
    expect(deleteScan).toHaveBeenCalledWith("repo-1", "scan-1");
    expect(await within(region).findByText(/None yet\. Analyze a repository above/)).toBeTruthy();
    expect(within(region).getByRole("status").textContent).toContain("Deleted analysis scan-1");
  });
});
