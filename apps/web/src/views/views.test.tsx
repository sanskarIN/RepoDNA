// Renders every view with the bundled demo artifact: a real RepoDNA analysis of this
// repository, so the views are exercised against complete data.

import { act } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
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

  it("charts a young repository's commits per day", async () => {
    await renderAt("/history", dataset);
    expect(await screen.findByRole("img", { name: "Commits per day" })).toBeTruthy();
  });

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

  it("shows the privacy policy and links the other legal pages inside the app", async () => {
    await renderAt("/privacy", dataset);
    expect((await screen.findByRole("heading", { level: 1 })).textContent).toBe("Privacy Policy");
    const terms = screen.getAllByRole("link", { name: "Terms of Use" });
    expect(terms.some((link) => link.getAttribute("href") === "#/terms")).toBe(true);
    const legal = screen.getByRole("navigation", { name: "Legal" });
    expect(legal.querySelector('a[href="#/licenses"]')).toBeTruthy();
  });

  it("offers every support link on the About page", async () => {
    await renderAt("/about", null);
    for (const url of [
      "https://github.com/sanskarIN/RepoDNA",
      "https://github.com/sanskarIN",
      "https://sanskarIN.gumroad.com",
      "https://www.buymeacoffee.com/sanskarIN",
      "https://www.razorpay.me/@sanskarIN",
    ]) {
      expect(document.querySelector(`a[href="${url}"]`), url).toBeTruthy();
    }
  });

  describe("licenses", () => {
    afterEach(() => {
      vi.unstubAllGlobals();
    });

    it("shows the license notices published next to the interface", async () => {
      const fetched: string[] = [];
      vi.stubGlobal("fetch", async (url: string) => {
        fetched.push(url);
        return new Response(`text of ${url}`, { status: 200 });
      });
      await renderAt("/licenses", null);
      expect(await screen.findByText("text of ./THIRD-PARTY-NOTICES.txt")).toBeTruthy();
      expect(fetched).toContain("./LICENSE.txt");
    });

    it("points to GitHub when the notices cannot be loaded", async () => {
      vi.stubGlobal("fetch", async () => new Response("", { status: 404 }));
      await renderAt("/licenses", null);
      const link = await screen.findByRole("link", { name: "THIRD-PARTY-NOTICES.txt" });
      expect(link.getAttribute("href")).toBe(
        "https://github.com/sanskarIN/RepoDNA/blob/main/THIRD-PARTY-NOTICES.txt",
      );
    });
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
