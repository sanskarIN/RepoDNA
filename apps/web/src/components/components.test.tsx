import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { Heatmap } from "../charts/Heatmap";
import { DataTable } from "./DataTable";
import { useTooltip } from "./Tooltip";
import { Tile } from "./common";

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
    fireEvent.click(screen.getByRole("button", { name: "Size" }));
    expect(names()).toEqual(["alpha", "gamma", "beta"]);
    expect(screen.getByRole("columnheader", { name: /Size/ }).getAttribute("aria-sort")).toBe(
      "descending",
    );
    fireEvent.click(screen.getByRole("button", { name: /Size/ }));
    expect(names()).toEqual(["beta", "gamma", "alpha"]);
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
