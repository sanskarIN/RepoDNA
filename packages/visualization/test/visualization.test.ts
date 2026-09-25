import { describe, expect, it } from "vitest";
import {
  DARK,
  LIGHT,
  bands,
  categorize,
  compact,
  heatCells,
  layeredLayout,
  linearScale,
  niceStep,
  percent,
  sequentialIndex,
  span,
  thousands,
  treemap,
} from "../src/index";

describe("palette", () => {
  it("keeps eight fixed categorical slots and folds the tail", () => {
    expect(LIGHT.series).toHaveLength(8);
    expect(DARK.series).toHaveLength(8);
    const items = Array.from({ length: 10 }, (_, i) => ({ name: `l${i}`, size: 10 - i }));
    const categories = categorize(
      items,
      (i) => i.size,
      (i) => i.name,
      "light",
    );
    expect(categories).toHaveLength(8);
    expect(categories[0]?.color).toBe(LIGHT.series[0]);
    expect(categories[7]?.label).toBe("Other (3)");
    expect(categories[7]?.value).toBe(3 + 2 + 1);
    expect(categories[7]?.color).toBe(LIGHT.other);
    // Colors follow the entity, not its rank, when a stable order is given.
    const stable = categorize(
      items.slice(1, 3),
      (i) => i.size,
      (i) => i.name,
      "dark",
      ["l0", "l1", "l2"],
    );
    expect(stable[0]?.color).toBe(DARK.series[1]);
    // Items outside the stable order fold into "Other" rather than borrow a slot color.
    const folded = categorize(
      items.slice(0, 3),
      (i) => i.size,
      (i) => i.name,
      "light",
      ["l1", "l2"],
    );
    expect(folded.map((c) => c.label)).toEqual(["l1", "l2", "Other (1)"]);
    expect(folded[0]?.color).toBe(LIGHT.series[0]);
    expect(folded[2]?.color).toBe(LIGHT.other);
    expect(sequentialIndex(0, 10)).toBe(0);
    expect(sequentialIndex(10, 10)).toBe(6);
    expect(sequentialIndex(1, 10)).toBe(1);
  });
});

describe("scales", () => {
  it("uses clean ticks", () => {
    expect(niceStep(100, 4)).toBe(50);
    expect(niceStep(7, 4)).toBe(2);
    const scale = linearScale(1840, [0, 200]);
    expect(scale.ticks).toEqual([0, 500, 1000, 1500, 2000]);
    expect(scale(1000)).toBe(100);
    expect(linearScale(0, [0, 10]).ticks).toEqual([0, 1]);
    expect(linearScale(1, [0, 10], 4, 0, false).ticks).toEqual([0, 0.5, 1]);
    const placed = bands(3, 90);
    expect(placed).toHaveLength(3);
    expect(placed[0]?.size).toBe(24);
    expect(placed[1]?.offset).toBeCloseTo(33);
  });
});

describe("treemap", () => {
  it("fills the area in proportion to the values", () => {
    const cells = treemap(
      [
        { value: 6, data: "a" },
        { value: 3, data: "b" },
        { value: 1, data: "c" },
        { value: 0, data: "none" },
      ],
      100,
      100,
    );
    expect(cells.map((c) => c.data)).toEqual(["a", "b", "c"]);
    const area = cells.reduce((sum, c) => sum + c.width * c.height, 0);
    expect(area).toBeCloseTo(10_000);
    expect((cells[0]?.width ?? 0) * (cells[0]?.height ?? 0)).toBeCloseTo(6000);
    for (const cell of cells) {
      expect(cell.x + cell.width).toBeLessThanOrEqual(100.0001);
      expect(cell.y + cell.height).toBeLessThanOrEqual(100.0001);
    }
    expect(treemap([], 10, 10)).toEqual([]);
  });
});

describe("layered layout", () => {
  it("places layers left to right and flags edges against the order", () => {
    const layout = layeredLayout(
      [
        { id: "core", layer: 0, weight: 100 },
        { id: "parser", layer: 1, weight: 50 },
        { id: "cli", layer: 2, weight: 10 },
        { id: "loose", layer: null, weight: 1 },
      ],
      [
        { from: "parser", to: "core", weight: 3 },
        { from: "cli", to: "parser", weight: 1 },
        { from: "core", to: "cli", weight: 1 },
        { from: "cli", to: "missing", weight: 1 },
      ],
      { width: 400 },
    );
    const x = (id: string) => layout.nodes.find((n) => n.id === id)?.x ?? -1;
    expect(x("core")).toBeLessThan(x("parser"));
    expect(x("parser")).toBeLessThan(x("cli"));
    expect(x("cli")).toBeLessThan(x("loose"));
    expect(layout.layers).toBe(4);
    expect(layout.edges).toHaveLength(3);
    expect(layout.edges.find((e) => e.from === "core")?.backward).toBe(true);
    expect(layout.edges.find((e) => e.from === "parser")?.backward).toBe(false);
    const core = layout.nodes.find((n) => n.id === "core");
    const loose = layout.nodes.find((n) => n.id === "loose");
    expect(core?.radius).toBeGreaterThan(loose?.radius ?? Infinity);
    expect(layeredLayout([], [], { width: 100 }).nodes).toEqual([]);
  });
});

describe("heatmap and formatting", () => {
  it("ranks cells and formats numbers", () => {
    const cells = heatCells([
      [0, 5],
      [10, 1],
    ]);
    expect(cells.map((c) => c.level)).toEqual([0, 3, 6, 1]);
    expect(thousands(1234567)).toBe("1,234,567");
    expect(compact(1284)).toBe("1,284");
    expect(compact(12_900)).toBe("12.9K");
    expect(compact(4_200_000)).toBe("4.2M");
    expect(percent(0.125)).toBe("12.5%");
    expect(percent(Number.NaN)).toBe("–");
    expect(span(3)).toBe("3 days");
    expect(span(400)).toBe("13 months");
    expect(span(800)).toBe("2.2 years");
  });
});
