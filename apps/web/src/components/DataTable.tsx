import { isValidElement, useMemo, useState, type ReactNode } from "react";
import { useDownload } from "../lib/download";
import { ErrorBox } from "./common";
import { usePanelTitle } from "./panelTitle";

export interface Column<T> {
  key: string;
  header: string;
  /** Cell content. */
  cell: (row: T) => ReactNode;
  /** Sort value; columns without one are not sortable. */
  sort?: (row: T) => number | string;
  /** Value in a downloaded CSV file, when the text of the cell is not the right one. */
  csv?: (row: T) => number | string;
  numeric?: boolean;
}

/** The text a cell shows, without its markup. */
function textOf(node: ReactNode): string {
  if (typeof node === "string" || typeof node === "number" || typeof node === "bigint") {
    return String(node);
  }
  if (Array.isArray(node)) {
    return node.map(textOf).join("");
  }
  if (isValidElement(node)) {
    const props = node.props as { children?: ReactNode; text?: unknown };
    if (props.children !== undefined) {
      return textOf(props.children);
    }
    return typeof props.text === "string" ? props.text : "";
  }
  return "";
}

/**
 * The value of `column` in a CSV file: its own, or the text of the cell, where "–" stands for
 * nothing. Numbers stay numbers that a spreadsheet can add up: a numeric column gives the
 * number it sorts by instead of "1,204" or "12.3 KB".
 */
function csvValue<T>(column: Column<T>, row: T): number | string {
  if (column.csv) {
    return column.csv(row);
  }
  const text = textOf(column.cell(row)).replace(/\s+/g, " ").trim();
  if (text === "–") {
    return "";
  }
  if (column.numeric) {
    const value = column.sort?.(row);
    if (typeof value === "number") {
      return value;
    }
    if (/^-?\d{1,3}(,\d{3})*(\.\d+)?$/.test(text)) {
      return Number(text.replace(/,/g, ""));
    }
  }
  return text;
}

/**
 * One CSV field. Text that a spreadsheet would read as a formula (=, +, -, @) gets a leading
 * apostrophe, so a file name in an analyzed repository cannot run anything when the file is
 * opened.
 */
export function csvField(value: number | string): string {
  if (typeof value === "number") {
    return Number.isFinite(value) ? String(value) : "";
  }
  const text = /^[=+\-@\t\r]/.test(value) ? `'${value}` : value;
  return /[",\n\r]/.test(text) || text !== text.trim() ? `"${text.replace(/"/g, '""')}"` : text;
}

/** The rows as CSV, every row and column of them, with a header line. */
export function toCsv<T>(rows: readonly T[], columns: readonly Column<T>[]): string {
  const lines = [columns.map((column) => csvField(column.header)).join(",")];
  for (const row of rows) {
    lines.push(columns.map((column) => csvField(csvValue(column, row))).join(","));
  }
  return `${lines.join("\r\n")}\r\n`;
}

/** A file name from the title of the panel a table is in: "Largest files" → "largest-files.csv". */
export function csvFileName(title: string | undefined): string {
  const slug = (title ?? "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return `${slug || "table"}.csv`;
}

/** Orders text the way people read it: "file2" before "file10", "1.9" before "1.10". */
const collator = new Intl.Collator(undefined, { numeric: true });

/** A sortable table that shows the first `limit` rows until asked for all of them. */
export function DataTable<T>({
  rows,
  columns,
  rowKey,
  limit = 25,
  caption,
  initialSort,
}: {
  rows: readonly T[];
  columns: Column<T>[];
  rowKey: (row: T, index: number) => string;
  limit?: number;
  caption?: string;
  initialSort?: { key: string; descending: boolean };
}) {
  const [sort, setSort] = useState(initialSort);
  const [expanded, setExpanded] = useState(false);
  const sorted = useMemo(() => {
    const column = columns.find((c) => c.key === sort?.key);
    if (!column?.sort) {
      return rows;
    }
    const value = column.sort;
    const direction = sort?.descending ? -1 : 1;
    return [...rows].sort((a, b) => {
      const x = value(a);
      const y = value(b);
      if (typeof x === "number" && typeof y === "number") {
        return (x - y) * direction;
      }
      return collator.compare(String(x), String(y)) * direction;
    });
  }, [rows, columns, sort]);
  const shown = expanded ? sorted : sorted.slice(0, limit);
  const title = usePanelTitle();
  const [save, failure] = useDownload();
  const download = () => save(csvFileName(title), toCsv(sorted, columns), "text/csv;charset=utf-8");

  return (
    <>
      <div className="table-wrap">
        <table>
          {caption ? <caption className="visually-hidden">{caption}</caption> : null}
          <thead>
            <tr>
              {columns.map((column) => {
                const active = sort?.key === column.key;
                const ariaSort = active
                  ? sort?.descending
                    ? "descending"
                    : "ascending"
                  : undefined;
                const arrow = active ? (sort?.descending ? " ↓" : " ↑") : null;
                return (
                  <th
                    key={column.key}
                    className={column.numeric ? "num" : undefined}
                    aria-sort={ariaSort}
                    scope="col"
                  >
                    {column.sort ? (
                      <button
                        type="button"
                        onClick={() =>
                          setSort({
                            key: column.key,
                            descending: active ? !sort?.descending : Boolean(column.numeric),
                          })
                        }
                      >
                        {column.header}
                        {arrow ?? (
                          <span className="sort-hint" aria-hidden="true">
                            {" ↕"}
                          </span>
                        )}
                      </button>
                    ) : (
                      column.header
                    )}
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {shown.map((row, index) => (
              <tr key={rowKey(row, index)}>
                {columns.map((column) => (
                  <td key={column.key} className={column.numeric ? "num" : undefined}>
                    {column.cell(row)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {/* Outside the part that scrolls sideways, so the buttons stay in view. */}
      {rows.length > 0 ? (
        <p className="table-actions">
          {sorted.length > limit ? (
            <button type="button" className="ghost" onClick={() => setExpanded((value) => !value)}>
              {expanded ? "Show fewer" : `Show all ${sorted.length} rows`}
            </button>
          ) : null}
          <button
            type="button"
            className="ghost"
            aria-label={title ? `Download CSV of ${title}` : undefined}
            onClick={download}
          >
            Download CSV
          </button>
        </p>
      ) : null}
      {failure ? <ErrorBox>{failure}</ErrorBox> : null}
      {rows.length === 0 ? <p className="muted">Nothing to show.</p> : null}
    </>
  );
}
