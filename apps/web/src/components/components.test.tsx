import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { Heatmap } from "../charts/Heatmap";
import { DataTable, csvField, csvFileName, toCsv, type Column } from "./DataTable";
import { useTooltip } from "./Tooltip";
import { Omittable, Panel, Tile } from "./common";

// The desktop app's commands, for a table saved in its window.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

interface Row {
  name: string;
  size: number;
}

const rows: Row[] = [
  { name: "beta", size: 2 },
  { name: "alpha", size: 30 },
  { name: "gamma", size: 7 },
];

function names(): string[] {
  const table = screen.getByRole("table");
  return within(table)
    .getAllByRole("row")
    .slice(1)
    .map((row) => within(row).getAllByRole("cell")[0]?.textContent ?? "");
}

describe("DataTable", () => {
  it("sorts by a column and toggles the direction", () => {
    render(
      <DataTable
        rows={rows}
        rowKey={(row) => row.name}
        columns={[
          { key: "name", header: "Name", cell: (row) => row.name, sort: (row) => row.name },
          {
            key: "size",
            header: "Size",
            cell: (row) => row.size,
            sort: (row) => row.size,
            numeric: true,
          },
        ]}
      />,
    );
    expect(names()).toEqual(["beta", "alpha", "gamma"]);
    // Sortable headers show that they sort without changing their names.
    expect(screen.getByRole("button", { name: "Size" }).textContent).toBe("Size ↕");
    fireEvent.click(screen.getByRole("button", { name: "Size" }));
    expect(names()).toEqual(["alpha", "gamma", "beta"]);
    expect(screen.getByRole("columnheader", { name: /Size/ }).getAttribute("aria-sort")).toBe(
      "descending",
    );
    fireEvent.click(screen.getByRole("button", { name: /Size/ }));
    expect(names()).toEqual(["beta", "gamma", "alpha"]);
  });

  it("sorts numbers inside text by their value", () => {
    const files = ["file10.rs", "file2.rs", "file1.rs"].map((name) => ({ name, size: 1 }));
    render(
      <DataTable
        rows={files}
        rowKey={(row) => row.name}
        columns={[
          { key: "name", header: "Name", cell: (row) => row.name, sort: (row) => row.name },
        ]}
        initialSort={{ key: "name", descending: false }}
      />,
    );
    expect(names()).toEqual(["file1.rs", "file2.rs", "file10.rs"]);
  });

  it("shows the first rows until asked for all", () => {
    render(
      <DataTable
        rows={rows}
        rowKey={(row) => row.name}
        limit={2}
        columns={[{ key: "name", header: "Name", cell: (row) => row.name }]}
      />,
    );
    expect(names()).toHaveLength(2);
    fireEvent.click(screen.getByRole("button", { name: "Show all 3 rows" }));
    expect(names()).toHaveLength(3);
  });
});

describe("CSV", () => {
  it("quotes fields that need it", () => {
    expect(csvField("plain")).toBe("plain");
    expect(csvField(1204)).toBe("1204");
    expect(csvField(Number.NaN)).toBe("");
    expect(csvField("a, b")).toBe('"a, b"');
    expect(csvField('say "hi"')).toBe('"say ""hi"""');
    expect(csvField("two\nlines")).toBe('"two\nlines"');
    expect(csvField(" padded")).toBe('" padded"');
  });

  it("keeps text a spreadsheet would run as a formula from running", () => {
    expect(csvField("=HYPERLINK(1)")).toBe("'=HYPERLINK(1)");
    expect(csvField("+1")).toBe("'+1");
    expect(csvField("-2")).toBe("'-2");
    expect(csvField("@SUM(A1)")).toBe("'@SUM(A1)");
    expect(csvField("=1,2")).toBe('"\'=1,2"');
    expect(csvField(-2)).toBe("-2");
  });

  it("writes every row with the text of its cells, numbers as numbers, and – as nothing", () => {
    const columns: Column<Row>[] = [
      { key: "name", header: "Name", cell: (row) => <span className="path">{row.name}</span> },
      { key: "note", header: "Note", cell: (row) => <Omittable text={`${row.name}!`} /> },
      {
        key: "size",
        header: "Size",
        cell: (row) => `${row.size} KB`,
        sort: (row) => row.size,
        numeric: true,
      },
      { key: "big", header: "Big", cell: () => "–", csv: (row) => (row.size > 5 ? "yes" : "no") },
      {
        key: "count",
        header: "Count",
        cell: (row) => (row.size > 5 ? `${row.size},000` : "–"),
        numeric: true,
      },
    ];
    expect(toCsv(rows, columns)).toBe(
      "Name,Note,Size,Big,Count\r\n" +
        "beta,beta!,2,no,\r\n" +
        "alpha,alpha!,30,yes,30000\r\n" +
        "gamma,gamma!,7,yes,7000\r\n",
    );
  });

  it("names the file after the panel", () => {
    expect(csvFileName("Largest files")).toBe("largest-files.csv");
    expect(csvFileName("Tests, build & docs")).toBe("tests-build-docs.csv");
    expect(csvFileName("")).toBe("table.csv");
    expect(csvFileName(undefined)).toBe("table.csv");
  });

  it("downloads all rows of a table in the order shown", async () => {
    const blobs = new Map<string, Blob>();
    const saved: { name: string; blob: Blob | undefined }[] = [];
    vi.spyOn(URL, "createObjectURL").mockImplementation((blob) => {
      const url = `blob:test/${blobs.size}`;
      blobs.set(url, blob as Blob);
      return url;
    });
    vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => undefined);
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (
      this: HTMLAnchorElement,
    ) {
      saved.push({ name: this.download, blob: blobs.get(this.href) });
    });
    render(
      <Panel title="Sizes">
        <DataTable
          rows={rows}
          rowKey={(row) => row.name}
          limit={2}
          columns={[
            { key: "name", header: "Name", cell: (row) => row.name, sort: (row) => row.name },
          ]}
          initialSort={{ key: "name", descending: false }}
        />
      </Panel>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Download CSV of Sizes" }));
    await waitFor(() => expect(saved).toHaveLength(1));
    expect(saved[0]?.name).toBe("sizes.csv");
    expect(saved[0]?.blob?.type).toBe("text/csv;charset=utf-8");
    expect(await saved[0]?.blob?.text()).toBe("Name\r\nalpha\r\nbeta\r\ngamma\r\n");
    vi.restoreAllMocks();
  });

  it("saves a table through the desktop app's save dialog, and says why it could not", async () => {
    const desktop = window as { __TAURI_INTERNALS__?: unknown };
    desktop.__TAURI_INTERNALS__ = {};
    vi.mocked(invoke).mockRejectedValueOnce("The disk is full.");
    try {
      render(
        <Panel title="Sizes">
          <DataTable
            rows={rows}
            rowKey={(row) => row.name}
            columns={[{ key: "name", header: "Name", cell: (row) => row.name }]}
          />
        </Panel>,
      );
      fireEvent.click(screen.getByRole("button", { name: "Download CSV of Sizes" }));
      expect((await screen.findByRole("alert")).textContent).toBe(
        "sizes.csv could not be saved: The disk is full.",
      );
      expect(invoke).toHaveBeenCalledWith("save_file", {
        name: "sizes.csv",
        text: "Name\r\nbeta\r\nalpha\r\ngamma\r\n",
      });
    } finally {
      delete desktop.__TAURI_INTERNALS__;
    }
  });

  it("lets the keyboard reach a table that scrolls sideways", () => {
    const scrollWidth = vi.spyOn(HTMLElement.prototype, "scrollWidth", "get");
    const clientWidth = vi.spyOn(HTMLElement.prototype, "clientWidth", "get");
    scrollWidth.mockReturnValue(800);
    clientWidth.mockReturnValue(300);
    try {
      render(
        <Panel title="Sizes">
          <DataTable
            rows={rows}
            rowKey={(row) => row.name}
            columns={[{ key: "name", header: "Name", cell: (row) => row.name }]}
          />
        </Panel>,
      );
      const region = screen.getByRole("region", { name: "Table: Sizes" });
      expect(region.className).toBe("table-wrap");
      expect(region.tabIndex).toBe(0);
    } finally {
      vi.restoreAllMocks();
    }
  });

  it("leaves a table that fits out of the keyboard's way", () => {
    render(
      <Panel title="Sizes">
        <DataTable
          rows={rows}
          rowKey={(row) => row.name}
          columns={[{ key: "name", header: "Name", cell: (row) => row.name }]}
        />
      </Panel>,
    );
    expect(screen.queryByRole("region", { name: "Table: Sizes" })).toBeNull();
    expect(document.querySelector(".table-wrap")?.hasAttribute("tabindex")).toBe(false);
  });

  it("offers no download for an empty table", () => {
    render(
      <DataTable
        rows={[] as Row[]}
        rowKey={(row) => row.name}
        columns={[{ key: "name", header: "Name", cell: (row) => row.name }]}
      />,
    );
    expect(screen.queryByRole("button", { name: /Download CSV/ })).toBeNull();
    expect(screen.getByText("Nothing to show.")).toBeTruthy();
  });
});

describe("Tile", () => {
  it("shows the label, value, and hint", () => {
    render(<Tile label="Files" value="1,234" note="small repository" />);
    expect(screen.getByText("Files")).toBeTruthy();
    expect(screen.getByTitle("1,234").textContent).toBe("1,234");
    expect(screen.getByText("small repository").className).toBe("hint");
  });
});

describe("Heatmap", () => {
  it("counts one commit in the singular", () => {
    const grid = Array.from({ length: 7 }, (_, row) =>
      Array.from({ length: 24 }, (_, hour) => (row === 0 && hour === 0 ? 1 : 0)),
    );
    const { container } = render(<Heatmap grid={grid} label="Commits by weekday and hour" />);
    const first = container.querySelector("rect.mark");
    if (!first) throw new Error("the heatmap has cells");
    fireEvent.focus(first);
    expect(screen.getByRole("status").textContent).toContain("1 commit");
    expect(screen.getByRole("status").textContent).not.toContain("1 commits");
  });
});

function TooltipProbe() {
  const { bind, element } = useTooltip();
  return (
    <div>
      <button type="button" {...bind({ value: "12 commits", label: "src/main.rs" })}>
        mark
      </button>
      {element}
    </div>
  );
}

/** Focuses the probe's mark as if it sat at `left`, `top` in a window of `width` × `height`. */
function focusMark(width: number, height: number, left: number, top: number): HTMLElement {
  Object.defineProperty(window, "innerWidth", { configurable: true, value: width });
  Object.defineProperty(window, "innerHeight", { configurable: true, value: height });
  render(<TooltipProbe />);
  const mark = screen.getByRole("button", { name: "mark" });
  mark.getBoundingClientRect = () => new DOMRect(left, top, 10, 10);
  fireEvent.focus(mark);
  return screen.getByRole("status");
}

describe("useTooltip", () => {
  afterEach(() => {
    Object.defineProperty(window, "innerWidth", { configurable: true, value: 1024 });
    Object.defineProperty(window, "innerHeight", { configurable: true, value: 768 });
  });

  it("keeps the tooltip inside a narrow window", () => {
    const tooltip = focusMark(320, 640, 300, 100);
    expect(tooltip.textContent).toContain("12 commits");
    expect(tooltip.style.left).toBe("8px");
  });

  it("opens below a mark in the top half of the window and above one in the bottom half", () => {
    const high = focusMark(1024, 768, 100, 100);
    expect([high.style.top, high.style.bottom]).toEqual(["88px", ""]);
    cleanup();
    const low = focusMark(1024, 768, 100, 700);
    expect([low.style.top, low.style.bottom]).toEqual(["", "56px"]);
  });
});
