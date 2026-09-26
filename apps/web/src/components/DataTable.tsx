import { useMemo, useState, type ReactNode } from "react";

export interface Column<T> {
  key: string;
  header: string;
  /** Cell content. */
  cell: (row: T) => ReactNode;
  /** Sort value; columns without one are not sortable. */
  sort?: (row: T) => number | string;
  numeric?: boolean;
}

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
      return String(x).localeCompare(String(y)) * direction;
    });
  }, [rows, columns, sort]);
  const shown = expanded ? sorted : sorted.slice(0, limit);

  return (
    <div className="table-wrap">
      <table>
        {caption ? <caption className="visually-hidden">{caption}</caption> : null}
        <thead>
          <tr>
            {columns.map((column) => {
              const active = sort?.key === column.key;
              const ariaSort = active ? (sort?.descending ? "descending" : "ascending") : undefined;
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
                      {active ? (sort?.descending ? " ↓" : " ↑") : ""}
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
      {sorted.length > limit ? (
        <p>
          <button type="button" className="ghost" onClick={() => setExpanded((value) => !value)}>
            {expanded ? "Show fewer" : `Show all ${sorted.length} rows`}
          </button>
        </p>
      ) : null}
      {rows.length === 0 ? <p className="muted">Nothing to show.</p> : null}
    </div>
  );
}
