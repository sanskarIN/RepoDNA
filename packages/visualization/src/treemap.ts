// Squarified treemap layout (Bruls, Huizing, and van Wijk).

export interface TreemapItem<T> {
  value: number;
  data: T;
}

export interface TreemapCell<T> {
  x: number;
  y: number;
  width: number;
  height: number;
  data: T;
  value: number;
}

interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

function worst(row: number[], side: number, scale: number): number {
  const sum = row.reduce((a, b) => a + b, 0) * scale;
  if (sum === 0) {
    return Infinity;
  }
  let max = 0;
  let min = Infinity;
  for (const value of row) {
    max = Math.max(max, value * scale);
    min = Math.min(min, value * scale);
  }
  const s2 = side * side;
  const sum2 = sum * sum;
  return Math.max((s2 * max) / sum2, sum2 / (s2 * min));
}

/** Lays out items with positive values inside `width` × `height`, largest first. */
export function treemap<T>(
  items: readonly TreemapItem<T>[],
  width: number,
  height: number,
): TreemapCell<T>[] {
  const positive = items.filter((item) => item.value > 0).sort((a, b) => b.value - a.value);
  const total = positive.reduce((sum, item) => sum + item.value, 0);
  if (total === 0 || width <= 0 || height <= 0) {
    return [];
  }
  const scale = (width * height) / total;
  const cells: TreemapCell<T>[] = [];
  let rect: Rect = { x: 0, y: 0, width, height };
  let row: TreemapItem<T>[] = [];

  const place = (items: TreemapItem<T>[], area: Rect): Rect => {
    const sum = items.reduce((s, item) => s + item.value, 0) * scale;
    if (area.width >= area.height) {
      const w = sum / area.height;
      let y = area.y;
      for (const item of items) {
        const h = (item.value * scale) / w;
        cells.push({ x: area.x, y, width: w, height: h, data: item.data, value: item.value });
        y += h;
      }
      return { x: area.x + w, y: area.y, width: area.width - w, height: area.height };
    }
    const h = sum / area.width;
    let x = area.x;
    for (const item of items) {
      const w = (item.value * scale) / h;
      cells.push({ x, y: area.y, width: w, height: h, data: item.data, value: item.value });
      x += w;
    }
    return { x: area.x, y: area.y + h, width: area.width, height: area.height - h };
  };

  for (const item of positive) {
    const side = Math.min(rect.width, rect.height);
    const values = row.map((r) => r.value);
    if (
      row.length === 0 ||
      worst([...values, item.value], side, scale) <= worst(values, side, scale)
    ) {
      row.push(item);
    } else {
      rect = place(row, rect);
      row = [item];
    }
  }
  if (row.length > 0) {
    place(row, rect);
  }
  return cells;
}
