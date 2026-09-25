// Renders every view with the bundled demo artifact: a real RepoDNA analysis of this
// repository, so the views are exercised against complete data.

import { act } from "react";
import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { parseArtifact } from "@repodna/schema";
import { App } from "../App";
import { NAV } from "../lib/nav";
import type { Dataset } from "../state";
import demoText from "../../public/demo/repodna.json?raw";

function demo(): Dataset {
  const loaded = parseArtifact(demoText);
  return {
    dna: loaded.artifact,
    origin: { kind: "demo", title: "RepoDNA" },
    warnings: loaded.warnings,
  };
}

async function renderAt(path: string, dataset: Dataset | null) {
  window.location.hash = `#${path}`;
  await act(async () => {
    render(<App initial={dataset} />);
  });
}

describe("views", () => {
  const dataset = demo();

  for (const item of NAV) {
    it(`renders ${item.label}`, async () => {
      await renderAt(item.path, dataset);
      const heading = await screen.findByRole("heading", { level: 1 });
      expect(heading.textContent).toBeTruthy();
      expect(screen.queryByText("This view could not be shown.")).toBeNull();
    });
  }

  it("asks for an analysis before showing data views", async () => {
    await renderAt("/architecture", null);
    expect((await screen.findByRole("heading", { level: 1 })).textContent).toBe(
      "No analysis is open",
    );
  });

  it("says when an address has no view", async () => {
    await renderAt("/nowhere", dataset);
    expect((await screen.findByRole("heading", { level: 1 })).textContent).toBe("Page not found");
  });

  it("filters findings by severity and search", async () => {
    await renderAt("/findings", dataset);
    const search = await screen.findByLabelText("Search findings");
    fireEvent.change(search, { target: { value: "no finding has this text" } });
    expect(screen.getByText("No findings match these filters.")).toBeTruthy();
  });

  it("opens the command palette with Ctrl+K and navigates with the keyboard", async () => {
    await renderAt("/overview", dataset);
    await act(async () => {
      fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    });
    const input = await screen.findByRole("combobox", { name: "Search and run commands" });
    await act(async () => {
      fireEvent.change(input, { target: { value: "time machine" } });
    });
    await act(async () => {
      fireEvent.keyDown(input, { key: "Enter" });
    });
    expect(window.location.hash).toBe("#/time-machine");
  });
});
