import { describe, expect, it } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { DataTable } from "./DataTable";
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
