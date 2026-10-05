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

  it("counts one module in the singular and leaves out an unavailable confidence", async () => {
    const single = demo();
    single.dna.architecture.modules = single.dna.architecture.modules.slice(0, 1);
    single.dna.architecture.styleConfidence = "unavailable";
    await renderAt("/overview", single);
    expect(await screen.findByText("1 module")).toBeTruthy();
    expect(screen.queryByText(/Unavailable confidence/)).toBeNull();
  });

  it("describes a one-file module in the singular", async () => {
    const single = demo();
    const [module] = single.dna.architecture.modules;
    if (!module) throw new Error("the demo has modules");
    module.files = 1;
    module.codeLines = 1;
    await renderAt(`/architecture?module=${encodeURIComponent(module.id)}`, single);
    expect(await screen.findByText(/^1 file · 1 code line/)).toBeTruthy();
  });

  it("names the inferred architecture style with its confidence", async () => {
    const data = demo();
    data.dna.architecture.style = "Layered";
    data.dna.architecture.styleConfidence = "medium";
    await renderAt("/architecture", data);
    const header = await screen.findByText(/Inferred style:/);
    expect(header.textContent).toContain("Inferred style: Layered (medium confidence). Modules");

    data.dna.architecture.style = "Unknown";
    data.dna.architecture.styleConfidence = "unavailable";
    await renderAt("/architecture", data);
    const unknown = (await screen.findAllByText(/Inferred style:/)).at(-1);
    expect(unknown?.textContent).toContain("Inferred style: Unknown. Modules");
  });

  it("labels a single import between modules in the singular", async () => {
    const data = demo();
    for (const edge of data.dna.architecture.moduleEdges) edge.weight = 1;
    await renderAt("/architecture", data);
    expect((await screen.findAllByText(/ \(1 import\)$/)).length).toBeGreaterThan(0);
    expect(screen.queryAllByText(/ \(1 imports\)$/)).toHaveLength(0);
  });

  it("counts one commit and one event in the singular on the Time Machine", async () => {
    const data = demo();
    const { evolution } = data.dna;
    evolution.events = evolution.events.slice(0, 1);
    for (const epoch of evolution.epochs) epoch.commits = 1;
    await renderAt("/time-machine", data);
    expect(await screen.findByText("1 event, oldest first.")).toBeTruthy();
    expect(screen.getAllByText(/ · 1 commit by /).length).toBeGreaterThan(0);
  });

  it("counts a single file in the singular on the Files view", async () => {
    const data = demo();
    data.dna.structure.files = data.dna.structure.files.slice(0, 1);
    await renderAt("/files", data);
    const shown = await screen.findAllByText("1 file");
    expect(shown.some((element) => element.getAttribute("aria-live") === "polite")).toBe(true);
  });

  it("counts a single dependency in the singular", async () => {
    const data = demo();
    const { dependencies } = data.dna;
    dependencies.dependencies = dependencies.dependencies.slice(0, 1);
    await renderAt("/dependencies", data);
    const shown = await screen.findAllByText("1 dependency");
    expect(shown.some((element) => element.getAttribute("aria-live") === "polite")).toBe(true);
  });

  it("says when one finding was suppressed", async () => {
    const data = demo();
    data.dna.analysisMetadata.suppressedFindings = 1;
    await renderAt("/reports", data);
    expect(await screen.findByText("1 finding was suppressed by these rules.")).toBeTruthy();
  });

  it('says "1 line added or removed" for a day with one changed line', async () => {
    const data = demo();
    const [day] = data.dna.git.dailyActivity;
    if (!day) throw new Error("the demo has daily activity");
    day.churn = 1;
    await renderAt("/history", data);
    const chart = await screen.findByRole("img", { name: "Commits per day" });
    const first = chart.querySelector(".mark");
    if (!first) throw new Error("the chart has columns");
    fireEvent.focus(first);
    const tips = screen.getAllByRole("status").map((element) => element.textContent ?? "");
    expect(tips.some((text) => text.includes("1 line added or removed"))).toBe(true);
  });

  it('says "1 function" for a language with one function', async () => {
    const data = demo();
    for (const language of data.dna.codeQuality.complexity.byLanguage) language.functions = 1;
    await renderAt("/quality", data);
    const chart = await screen.findByRole("img", {
      name: "Average cyclomatic complexity by language",
    });
    const first = chart.querySelector(".mark");
    if (!first) throw new Error("the chart has bars");
    fireEvent.focus(first);
    const tips = screen.getAllByRole("status").map((element) => element.textContent ?? "");
    expect(tips.some((text) => text.includes("1 function · highest"))).toBe(true);
  });

  it("describes a one-line README in the singular", async () => {
    const data = demo();
    const { readme } = data.dna.docs;
    if (!readme) throw new Error("the demo has a README");
    readme.words = 1;
    readme.lines = 1;
    await renderAt("/project", data);
    expect(await screen.findByText("1 word · 1 line")).toBeTruthy();
  });

  it("opens the navigation with the Menu button and closes it on a new page", async () => {
    await renderAt("/overview", dataset);
    const menu = () => document.getElementById("sidebar-menu");
    const toggle = screen.getByRole("button", { name: "Menu" });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(menu()?.hasAttribute("data-open")).toBe(false);
    fireEvent.click(toggle);
    expect(screen.getByRole("button", { name: "Close menu" }).getAttribute("aria-expanded")).toBe(
      "true",
    );
    expect(menu()?.hasAttribute("data-open")).toBe(true);
    await act(async () => {
      window.location.hash = "#/history";
      window.dispatchEvent(new HashChangeEvent("hashchange"));
    });
    expect(screen.getByRole("button", { name: "Menu" }).getAttribute("aria-expanded")).toBe(
      "false",
    );
    expect(menu()?.hasAttribute("data-open")).toBe(false);
  });

  it("names the file that could not be opened", async () => {
    await renderAt("/", null);
    const input = screen.getByLabelText("Open an analysis file");
    await act(async () => {
      fireEvent.change(input, {
        target: { files: [new File(["not json"], "notes.json", { type: "application/json" })] },
      });
    });
    expect(
      await screen.findByText(/^Could not open notes\.json: The file is not valid JSON/),
    ).toBeTruthy();
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
      "https://sanskarin.github.io",
      "https://sanskarIN.gumroad.com",
      "https://www.buymeacoffee.com/sanskarIN",
      "https://www.razorpay.me/@sanskarIN",
    ]) {
      expect(document.querySelector(`a[href="${url}"]`), url).toBeTruthy();
    }
  });

  it("links the sidebar credit to the creator's website", async () => {
    await renderAt("/overview", dataset);
    const credit = screen.getByRole("link", { name: "Made by the Sanskar" });
    expect(credit.getAttribute("href")).toBe("https://sanskarin.github.io");
  });

  it("points to more open-source projects from the About page's introduction", async () => {
    await renderAt("/about", null);
    const introduction = screen.getByText(/More open-source projects by Sanskar/);
    expect(introduction.querySelector('a[href="https://sanskarin.github.io"]')).toBeTruthy();
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
