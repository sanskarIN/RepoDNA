import { useMemo, useState } from "react";
import { layeredLayout, thousands } from "@repodna/visualization";
import { shorten, useWidth } from "./size";

/** Horizontal room per dependency layer, enough for a module name under each node. */
const MIN_COLUMN_WIDTH = 150;

export interface GraphModule {
  id: string;
  name: string;
  layer: number | null;
  codeLines: number;
  fanIn: number;
  fanOut: number;
}

export interface GraphDependency {
  from: string;
  to: string;
  weight: number;
}

/**
 * Modules as nodes in dependency layers (layer 0 on the left depends on no other
 * module). Selecting a node highlights what it depends on and what depends on it; edges
 * that run against the layer order are drawn in the caution color.
 */
export function ArchitectureGraph({
  modules,
  dependencies,
  selected,
  onSelect,
  label,
}: {
  modules: GraphModule[];
  dependencies: GraphDependency[];
  selected: string | null;
  onSelect: (id: string | null) => void;
  label: string;
}) {
  const [ref, width] = useWidth<HTMLDivElement>();
  const [hovered, setHovered] = useState<string | null>(null);
  const layout = useMemo(() => {
    // Leave room for a label between columns; wider graphs scroll sideways.
    const layered = modules.filter((m) => m.layer !== null).map((m) => m.layer ?? 0);
    const columns =
      (layered.length > 0 ? Math.max(...layered) + 1 : 0) +
      (modules.some((m) => m.layer === null) ? 1 : 0);
    return layeredLayout(
      modules.map((m) => ({ id: m.id, layer: m.layer, weight: m.codeLines })),
      dependencies,
      { width: Math.max(320, width, columns * MIN_COLUMN_WIDTH), rowGap: 46, padding: 80 },
    );
  }, [modules, dependencies, width]);
  const focus = hovered ?? selected;
  const related = useMemo(() => {
    const set = new Set<string>();
    if (focus) {
      set.add(focus);
      for (const edge of dependencies) {
        if (edge.from === focus) {
          set.add(edge.to);
        }
        if (edge.to === focus) {
          set.add(edge.from);
        }
      }
    }
    return set;
  }, [focus, dependencies]);
  const byId = new Map(modules.map((m) => [m.id, m]));
  const maxWeight = Math.max(1, ...dependencies.map((d) => d.weight));

  return (
    <div ref={ref}>
      <div className="graph-scroll">
        <svg
          className="chart"
          width={layout.width}
          height={layout.height}
          role="group"
          aria-label={label}
          onClick={(event) => {
            if (event.target === event.currentTarget) {
              onSelect(null);
            }
          }}
        >
          <defs>
            <marker
              id="arrow"
              viewBox="0 0 8 8"
              refX="7"
              refY="4"
              markerWidth="6"
              markerHeight="6"
              orient="auto-start-reverse"
            >
              <path d="M0,0 L8,4 L0,8 z" fill="var(--chart-axis)" />
            </marker>
          </defs>
          {layout.edges.map((edge) => {
            const active = focus !== null && (edge.from === focus || edge.to === focus);
            const dimmed = focus !== null && !active;
            return (
              <path
                key={`${edge.from}->${edge.to}`}
                d={edge.path}
                fill="none"
                stroke={
                  edge.backward ? "var(--attention)" : active ? "var(--s1)" : "var(--chart-axis)"
                }
                strokeWidth={1 + (2 * edge.weight) / maxWeight}
                opacity={dimmed ? 0.12 : active ? 0.95 : 0.55}
                markerEnd="url(#arrow)"
              >
                <title>
                  {`${byId.get(edge.from)?.name ?? edge.from} depends on ${byId.get(edge.to)?.name ?? edge.to} (${thousands(edge.weight)} imports)${edge.backward ? "; against the layer order" : ""}`}
                </title>
              </path>
            );
          })}
          {layout.nodes.map((node) => {
            const module = byId.get(node.id);
            const isSelected = node.id === selected;
            const dimmed = focus !== null && !related.has(node.id);
            const name = module?.name ?? node.id;
            return (
              <g
                key={node.id}
                className="mark"
                role="button"
                tabIndex={0}
                aria-pressed={isSelected}
                aria-label={`${name}: ${thousands(module?.codeLines ?? 0)} code lines, used by ${module?.fanIn ?? 0} modules, uses ${module?.fanOut ?? 0}`}
                opacity={dimmed ? 0.3 : 1}
                onPointerEnter={() => setHovered(node.id)}
                onPointerLeave={() => setHovered(null)}
                onFocus={() => setHovered(node.id)}
                onBlur={() => setHovered(null)}
                onClick={() => onSelect(isSelected ? null : node.id)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    onSelect(isSelected ? null : node.id);
                  }
                }}
              >
                <circle cx={node.x} cy={node.y} r={node.radius + 12} fill="transparent" data-hit />
                <circle
                  cx={node.x}
                  cy={node.y}
                  r={node.radius}
                  fill={isSelected ? "var(--s1)" : "var(--seq3)"}
                  stroke={isSelected ? "var(--ink)" : "var(--surface)"}
                  strokeWidth={2}
                />
                <text
                  className="label"
                  x={node.x}
                  y={node.y + node.radius + 13}
                  textAnchor="middle"
                >
                  {shorten(name, 22)}
                </text>
              </g>
            );
          })}
        </svg>
      </div>
      <div className="legend">
        <span>
          <i style={{ background: "var(--seq3)", borderRadius: "50%" }} aria-hidden="true" />
          Module (area = code lines)
        </span>
        <span>
          <i style={{ background: "var(--chart-axis)", height: 2, width: 14 }} aria-hidden="true" />
          Depends on (arrow points to the dependency)
        </span>
        <span>
          <i style={{ background: "var(--attention)", height: 2, width: 14 }} aria-hidden="true" />
          Against the layer order (cycle or same-layer dependency)
        </span>
      </div>
    </div>
  );
}
