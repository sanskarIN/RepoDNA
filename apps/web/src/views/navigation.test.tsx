// Ways through an analysis: tiles that open their views, links to the panels of a long view,
// the previous and next view, the findings count, and findings shown in parts.

import { act } from "react";
import { describe, expect, it } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { parseArtifact } from "@repodna/schema";
import { App } from "../App";
import type { Dataset } from "../state";
import { FINDINGS_PAGE } from "./Findings";
import demoText from "../../public/demo/repodna.json?raw";

function demo(): Dataset {
  const loaded = parseArtifact(demoText);
  return { dna: loaded.artifact, origin: { kind: "demo", title: "RepoDNA" }, warnings: [] };
}

async function renderAt(path: string, dataset: Dataset | null = demo()) {
  window.location.hash = `#${path}`;
  await act(async () => {
    render(<App initial={dataset} />);
  });
}

describe("navigation", () => {
  it("links the overview's numbers to the views about them", async () => {
    await renderAt("/overview");
    const main = await screen.findByRole("main");
    expect(
      within(main)
        .getByRole("link", { name: /^Commits/ })
        .getAttribute("href"),
    ).toBe("#/history");
    expect(
      within(main)
        .getByRole("link", { name: /^Findings/ })
        .getAttribute("href"),
    ).toBe("#/findings");
  });

  it("lists the panels of a long view and moves to one", async () => {
    await renderAt("/time-machine");
    const onPage = await screen.findByRole("navigation", { name: "On this page" });
    const links = within(onPage).getAllByRole("button");
    expect(links.length).toBeGreaterThanOrEqual(4);
    const target = links[1] as HTMLElement;
    await act(async () => {
      fireEvent.click(target);
    });
    expect(document.activeElement?.tagName).toBe("H2");
    expect(document.activeElement?.textContent).toBe(target.textContent);
  });

  it("leaves short views without the list of panels", async () => {
    await renderAt("/settings");
    await screen.findByRole("heading", { level: 1, name: "Settings" });
    expect(screen.queryByRole("navigation", { name: "On this page" })).toBeNull();
  });

  it("links each view to the previous and next one, and [ and ] go there", async () => {
    await renderAt("/history");
    const pager = await screen.findByRole("navigation", { name: "Previous and next view" });
    expect(
      within(pager)
        .getByRole("link", { name: /Dependencies/ })
        .getAttribute("href"),
    ).toBe("#/dependencies");
    expect(
      within(pager)
        .getByRole("link", { name: /Time Machine/ })
        .getAttribute("href"),
    ).toBe("#/time-machine");
    await act(async () => {
      fireEvent.keyDown(window, { key: "]" });
    });
    expect(window.location.hash).toBe("#/time-machine");
    await act(async () => {
      fireEvent.keyDown(window, { key: "[" });
    });
    expect(window.location.hash).toBe("#/history");
  });

  it("does not change the view when [ is typed in a search box", async () => {
    await renderAt("/files");
    const search = await screen.findByLabelText(/Search/);
    await act(async () => {
      fireEvent.keyDown(search, { key: "[" });
    });
    expect(window.location.hash).toBe("#/files");
  });

  it("counts the findings beside their view in the navigation", async () => {
    const dataset = demo();
    const active = dataset.dna.findings.filter((f) => !f.suppressed).length;
    await renderAt("/overview", dataset);
    const nav = screen.getByRole("complementary", { name: "Navigation" });
    const link = within(nav).getByRole("link", { name: new RegExp(`^Findings:\\s*${active}$`) });
    expect(link.getAttribute("href")).toBe("#/findings");
  });

  it("shows findings in parts and more on request", async () => {
    const dataset = demo();
    const active = dataset.dna.findings.filter((f) => !f.suppressed).length;
    expect(active).toBeGreaterThan(FINDINGS_PAGE);
    await renderAt("/findings", dataset);
    expect(await screen.findByText(`Showing ${FINDINGS_PAGE} of ${active} findings`)).toBeTruthy();
    expect(screen.getAllByRole("heading", { level: 2 }).length).toBeLessThanOrEqual(
      FINDINGS_PAGE + 1,
    );
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: `Show all ${active}` }));
    });
    expect(screen.getByText(`${active} findings`)).toBeTruthy();
  });

  it("opens findings filtered by severity from the address", async () => {
    await renderAt("/findings?severity=info");
    const select = (await screen.findByLabelText("Severity")) as HTMLSelectElement;
    expect(select.value).toBe("info");
  });
});
