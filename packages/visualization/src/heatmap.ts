// Weekday × hour activity grids.

/** Weekday names, Monday first (the order Git activity grids use). */
export const WEEKDAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] as const;

export interface HeatCell {
  row: number;
  column: number;
  value: number;
  /** Sequential ramp step, 0 (none) to 6 (most). */
  level: number;
}

/** Flattens a grid of counts into cells with ramp steps relative to the busiest cell. */
export function heatCells(grid: readonly (readonly number[])[]): HeatCell[] {
  const max = Math.max(0, ...grid.flat());
  const cells: HeatCell[] = [];
  grid.forEach((row, rowIndex) => {
    row.forEach((value, column) => {
      const level = value > 0 && max > 0 ? Math.min(6, 1 + Math.floor((value / max) * 5.999)) : 0;
      cells.push({ row: rowIndex, column, value, level });
    });
  });
  return cells;
}
