// Colors shared with the HTML reports (crates/repodna-report/src/palette.rs).
//
// Categorical slots are assigned in a fixed order and never cycled; past seven named
// entities the tail folds into "Other". The order and steps were validated for color
// vision deficiencies on both surfaces. Three light-mode slots sit below 3:1 contrast, so
// every chart also has labels and a table view. Status colors are reserved for severity
// and always appear with a label.

export type Theme = "light" | "dark";

export interface Palette {
  surface: string;
  page: string;
  ink: string;
  inkSecondary: string;
  muted: string;
  grid: string;
  axis: string;
  series: readonly string[];
  other: string;
  sequential: readonly string[];
}

export const LIGHT: Palette = {
  surface: "#fcfcfb",
  page: "#f9f9f7",
  ink: "#0b0b0b",
  inkSecondary: "#52514e",
  muted: "#898781",
  grid: "#e1e0d9",
  axis: "#c3c2b7",
  series: ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#4a3aa7", "#e34948"],
  other: "#b4b2a9",
  sequential: ["#f0efec", "#cde2fb", "#9ec5f4", "#6da7ec", "#3987e5", "#256abf", "#104281"],
};

export const DARK: Palette = {
  surface: "#1a1a19",
  page: "#0d0d0d",
  ink: "#ffffff",
  inkSecondary: "#c3c2b7",
  muted: "#898781",
  grid: "#2c2c2a",
  axis: "#383835",
  series: ["#3987e5", "#d95926", "#199e70", "#c98500", "#d55181", "#008300", "#9085e9", "#e66767"],
  other: "#5f5e5a",
  sequential: ["#262625", "#0d366b", "#184f95", "#256abf", "#3987e5", "#6da7ec", "#b7d3f6"],
};

/** Severity colors, the same on both surfaces. */
export const STATUS = {
  critical: "#d03b3b",
  warning: "#ec835a",
  attention: "#fab219",
  good: "#0ca30c",
  info: "#898781",
} as const;

export function palette(theme: Theme): Palette {
  return theme === "dark" ? DARK : LIGHT;
}

/** Most named categories before the rest fold into "Other". */
export const MAX_CATEGORIES = 7;

export interface Category<T> {
  label: string;
  value: number;
  color: string;
  items: T[];
}

/**
 * Assigns fixed-order colors to the largest categories and folds the tail into "Other".
 * To keep an entity's color the same across views (for example across snapshots), pass
 * a stable ordering of labels as `order`: its first seven labels own the named slots, and
 * every other item folds into "Other" instead of borrowing a color.
 */
export function categorize<T>(
  items: readonly T[],
  value: (item: T) => number,
  label: (item: T) => string,
  theme: Theme,
  order?: readonly string[],
): Category<T>[] {
  const colors = palette(theme);
  const sorted = [...items].sort((a, b) => value(b) - value(a));
  const slots = order?.slice(0, MAX_CATEGORIES);
  const named = slots
    ? sorted.filter((item) => slots.includes(label(item)))
    : sorted.slice(0, MAX_CATEGORIES);
  const rest = slots
    ? sorted.filter((item) => !slots.includes(label(item)))
    : sorted.slice(MAX_CATEGORIES);
  const categories: Category<T>[] = named.map((item, index) => {
    const slot = slots ? slots.indexOf(label(item)) : index;
    const color = colors.series[slot] ?? colors.other;
    return { label: label(item), value: value(item), color, items: [item] };
  });
  if (rest.length > 0) {
    categories.push({
      label: `Other (${rest.length})`,
      value: rest.reduce((sum, item) => sum + value(item), 0),
      color: colors.other,
      items: rest,
    });
  }
  return categories;
}

/** A sequential ramp step (0 = none, 6 = most) for a value between 0 and `max`. */
export function sequentialIndex(value: number, max: number): number {
  if (!(value > 0) || !(max > 0)) {
    return 0;
  }
  return Math.min(6, 1 + Math.floor((value / max) * 5.999));
}
