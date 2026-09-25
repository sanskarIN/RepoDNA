// Layered graph layout for dependency graphs. Modules already carry a layer (their depth
// in the dependency order), so the layout places layers as columns, orders nodes within a
// layer by the barycenter of their neighbors to reduce crossings, and draws edges as
// smooth curves. It is deterministic: the same graph always gets the same picture.

export interface GraphNode {
  id: string;
  /** Layer index (0 = depends on nothing); nodes without a layer go last. */
  layer: number | null;
  /** Relative size, e.g. lines of code. */
  weight: number;
}

export interface GraphEdge {
  from: string;
  to: string;
  weight: number;
}

export interface PlacedNode {
  id: string;
  x: number;
  y: number;
  radius: number;
  layer: number;
}

export interface PlacedEdge {
  from: string;
  to: string;
  weight: number;
  path: string;
  /** `true` when the edge points against the layer order (part of a cycle). */
  backward: boolean;
}

export interface GraphLayout {
  nodes: PlacedNode[];
  edges: PlacedEdge[];
  width: number;
  height: number;
  layers: number;
}

export interface LayoutOptions {
  width: number;
  /** Vertical distance between nodes in a layer. */
  rowGap?: number;
  minRadius?: number;
  maxRadius?: number;
  padding?: number;
  /** Barycenter sweeps. */
  sweeps?: number;
}

function mean(values: number[]): number | undefined {
  return values.length === 0 ? undefined : values.reduce((a, b) => a + b, 0) / values.length;
}

/**
 * Layers for a graph that has none: nodes that depend on nothing are layer 0, and every
 * other node sits one layer past its deepest dependency. Nodes in a cycle, or depending on
 * one, get `null`.
 */
export function assignLayers(
  ids: readonly string[],
  edges: readonly { from: string; to: string }[],
): Map<string, number | null> {
  const known = new Set(ids);
  const dependencies = new Map<string, string[]>(ids.map((id) => [id, []]));
  for (const edge of edges) {
    if (known.has(edge.from) && known.has(edge.to) && edge.from !== edge.to) {
      dependencies.get(edge.from)?.push(edge.to);
    }
  }
  const layers = new Map<string, number | null>();
  const visiting = new Set<string>();
  const visit = (id: string): number | null => {
    if (layers.has(id)) {
      return layers.get(id) ?? null;
    }
    if (visiting.has(id)) {
      return null;
    }
    visiting.add(id);
    let layer: number | null = 0;
    for (const dependency of dependencies.get(id) ?? []) {
      const below = visit(dependency);
      layer = below === null || layer === null ? null : Math.max(layer, below + 1);
    }
    visiting.delete(id);
    layers.set(id, layer);
    return layer;
  };
  for (const id of ids) {
    visit(id);
  }
  return layers;
}

/** Lays out a layered dependency graph from left (layer 0) to right. */
export function layeredLayout(
  nodes: readonly GraphNode[],
  edges: readonly GraphEdge[],
  options: LayoutOptions,
): GraphLayout {
  const rowGap = options.rowGap ?? 44;
  const minRadius = options.minRadius ?? 5;
  const maxRadius = options.maxRadius ?? 14;
  const padding = options.padding ?? 24;
  const sweeps = options.sweeps ?? 6;
  if (nodes.length === 0) {
    return { nodes: [], edges: [], width: options.width, height: padding * 2, layers: 0 };
  }

  const known = new Set(nodes.map((n) => n.id));
  const maxLayer = Math.max(0, ...nodes.map((n) => n.layer ?? -1));
  const layerOf = new Map(nodes.map((n) => [n.id, n.layer ?? maxLayer + 1]));
  const layerCount = Math.max(...layerOf.values()) + 1;
  const columns: string[][] = Array.from({ length: layerCount }, () => []);
  for (const node of [...nodes].sort((a, b) => b.weight - a.weight || a.id.localeCompare(b.id))) {
    columns[layerOf.get(node.id) ?? 0]?.push(node.id);
  }

  const neighbors = new Map<string, string[]>();
  for (const edge of edges) {
    if (!known.has(edge.from) || !known.has(edge.to) || edge.from === edge.to) {
      continue;
    }
    neighbors.set(edge.from, [...(neighbors.get(edge.from) ?? []), edge.to]);
    neighbors.set(edge.to, [...(neighbors.get(edge.to) ?? []), edge.from]);
  }

  const position = new Map<string, number>();
  const refresh = () => {
    for (const column of columns) {
      column.forEach((id, index) => position.set(id, index));
    }
  };
  refresh();
  for (let sweep = 0; sweep < sweeps; sweep += 1) {
    const order = sweep % 2 === 0 ? columns.keys() : [...columns.keys()].reverse();
    for (const layer of order) {
      const column = columns[layer];
      if (!column) {
        continue;
      }
      const keyed = column.map((id, index) => ({
        id,
        key:
          mean(
            (neighbors.get(id) ?? [])
              .filter((other) => layerOf.get(other) !== layer)
              .map((other) => position.get(other) ?? 0),
          ) ?? index,
        index,
      }));
      keyed.sort((a, b) => a.key - b.key || a.index - b.index);
      columns[layer] = keyed.map((entry) => entry.id);
      refresh();
    }
  }

  const tallest = Math.max(...columns.map((c) => c.length));
  const height = padding * 2 + Math.max(0, tallest - 1) * rowGap;
  const innerWidth = Math.max(0, options.width - padding * 2);
  const xStep = layerCount > 1 ? innerWidth / (layerCount - 1) : 0;
  const maxWeight = Math.max(1, ...nodes.map((n) => n.weight));
  const weights = new Map(nodes.map((n) => [n.id, n.weight]));
  const placed = new Map<string, PlacedNode>();
  columns.forEach((column, layer) => {
    const offset = ((tallest - column.length) * rowGap) / 2;
    column.forEach((id, index) => {
      const weight = weights.get(id) ?? 0;
      placed.set(id, {
        id,
        layer,
        x: padding + (layerCount > 1 ? layer * xStep : innerWidth / 2),
        y: padding + offset + index * rowGap,
        radius: minRadius + (maxRadius - minRadius) * Math.sqrt(weight / maxWeight),
      });
    });
  });

  const placedEdges: PlacedEdge[] = [];
  for (const edge of edges) {
    const a = placed.get(edge.from);
    const b = placed.get(edge.to);
    if (!a || !b || a.id === b.id) {
      continue;
    }
    // Edges point from the dependent to its dependency, so they usually run leftwards.
    const backward = a.layer <= b.layer;
    const dx = Math.max(24, Math.abs(b.x - a.x) / 2);
    const path =
      a.layer === b.layer
        ? `M${a.x},${a.y} C${a.x + dx},${a.y} ${b.x + dx},${b.y} ${b.x},${b.y}`
        : `M${a.x},${a.y} C${a.x + (b.x > a.x ? dx : -dx)},${a.y} ${b.x + (b.x > a.x ? -dx : dx)},${b.y} ${b.x},${b.y}`;
    placedEdges.push({ from: edge.from, to: edge.to, weight: edge.weight, path, backward });
  }
  return {
    nodes: [...placed.values()],
    edges: placedEdges,
    width: options.width,
    height,
    layers: layerCount,
  };
}
