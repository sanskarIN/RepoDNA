// Stored analyses on the start page, as `repodna serve` and the desktop app list them.

import { act } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { App } from "../App";
import { detectBackend, type Backend, type RepositorySummary } from "../lib/backend";
import { memoryStore } from "../lib/recent";
import { STORED_SHOWN } from "./Home";

vi.mock("../lib/backend", async (original) => ({
  ...(await original<typeof import("../lib/backend")>()),
  detectBackend: vi.fn(),
}));

function repositories(count: number): RepositorySummary[] {
  return Array.from({ length: count }, (_, index) => ({
    id: `id-${index}`,
    name: index === 3 ? "payments-service" : `project-${index}`,
    location: `/work/${index === 3 ? "payments" : `project-${index}`}`,
    kind: "local",
    scans: 1,
    firstScannedAt: "2026-10-01T00:00:00Z",
    lastScannedAt: "2026-10-01T00:00:00Z",
  }));
}

function serving(list: RepositorySummary[]) {
  const backend = {
    kind: "server",
    session: async () => ({ version: "test", allowScans: true }),
    repositories: async () => list,
  } as unknown as Backend;
  vi.mocked(detectBackend).mockResolvedValue({
    backend,
    session: { version: "test", allowScans: true },
    signInNeeded: false,
  });
}

async function start() {
  window.location.hash = "#/";
  await act(async () => {
    render(<App store={memoryStore()} />);
  });
  return screen.findByRole("region", { name: "Stored analyses" });
}

function names(region: HTMLElement): string[] {
  return within(region)
    .getAllByRole("listitem")
    .map((item) => item.querySelector("strong")?.textContent ?? "");
}

describe("stored analyses", () => {
  beforeEach(() => {
    vi.mocked(detectBackend).mockReset();
  });

  it("lists a few without a filter", async () => {
    serving(repositories(3));
    const region = await start();
    expect(await within(region).findAllByRole("listitem")).toHaveLength(3);
    expect(within(region).queryByRole("searchbox")).toBeNull();
  });

  it("lists the newest of many and the rest on request", async () => {
    serving(repositories(12));
    const region = await start();
    await within(region).findAllByRole("listitem");
    expect(names(region)).toHaveLength(STORED_SHOWN);
    fireEvent.click(within(region).getByRole("button", { name: "Show all 12 stored analyses" }));
    expect(names(region)).toHaveLength(12);
  });

  it("filters many by name or location", async () => {
    serving(repositories(12));
    const region = await start();
    await within(region).findAllByRole("listitem");
    const filter = within(region).getByRole("searchbox", { name: "Filter stored analyses" });
    fireEvent.change(filter, { target: { value: "PAYMENTS" } });
    expect(names(region)).toEqual(["payments-service"]);
    fireEvent.change(filter, { target: { value: "project-11" } });
    expect(names(region)).toEqual(["project-11"]);
    fireEvent.change(filter, { target: { value: "nothing like it" } });
    expect(within(region).getByText("No stored analysis matches “nothing like it”.")).toBeTruthy();
  });
});
