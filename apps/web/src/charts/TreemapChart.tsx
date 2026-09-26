import { sequentialIndex, thousands, treemap } from "@repodna/visualization";
import { useTooltip } from "../components/Tooltip";
import { shorten, textWidth, useWidth } from "./size";

export interface TreemapDatum {
  label: string;
  /** Area. */
  size: number;
  /** Color intensity (for example recent changes); 0 is the lightest step. */
  heat: number;
  details?: string;
}

/**
 * Areas show size; a single-hue ramp shows `heat`. Labels appear only where they fit, and
 * every value stays available in the tooltip and the table twin.
 */
export function TreemapChart({
  data,
  label,
  sizeUnit,
  heatLabel,
  height = 360,
}: {
  data: TreemapDatum[];
  label: string;
  sizeUnit: string;
  heatLabel: string;
  height?: number;
}) {
  const [ref, width] = useWidth<HTMLDivElement>();
  const tooltip = useTooltip();
  const cells = treemap(
    data.map((d) => ({ value: d.size, data: d })),
    width,
    height,
  );
  const maxHeat = Math.max(0, ...data.map((d) => d.heat));
  return (
    <div ref={ref}>
      <svg className="chart" width={width} height={height} role="img" aria-label={label}>
        {cells.map((cell) => {
          const level = sequentialIndex(cell.data.heat, maxHeat);
          const name = shorten(cell.data.label, Math.floor((cell.width - 8) / 6.2));
          const fits = cell.width > 48 && cell.height > 20 && textWidth(name) < cell.width - 8;
          const darkFill = level >= 4;
          return (
            <g
              key={cell.data.label}
              className="mark"
              {...tooltip.bind({
                value: `${thousands(cell.value)} ${sizeUnit}`,
                label: cell.data.label,
                details: `${heatLabel}: ${thousands(cell.data.heat)}${cell.data.details ? ` · ${cell.data.details}` : ""}`,
              })}
            >
              <rect
                x={cell.x + 1}
                y={cell.y + 1}
                width={Math.max(0, cell.width - 2)}
                height={Math.max(0, cell.height - 2)}
                rx={3}
                fill={`var(--seq${level})`}
              />
              {fits ? (
                <text
                  x={cell.x + 6}
                  y={cell.y + 15}
                  style={{ fill: darkFill ? "#ffffff" : "var(--ink)" }}
                >
                  {name}
                </text>
              ) : null}
            </g>
          );
        })}
      </svg>
      <div className="legend" aria-hidden="true">
        <span>Area: {sizeUnit}</span>
        <span>
          Color: {heatLabel} (lighter = fewer)
          {[0, 2, 4, 6].map((l) => (
            <i key={l} style={{ background: `var(--seq${l})` }} />
          ))}
        </span>
      </div>
      {tooltip.element}
    </div>
  );
}
