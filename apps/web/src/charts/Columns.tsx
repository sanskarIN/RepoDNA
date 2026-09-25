import { bands, linearScale, thousands } from "@repodna/visualization";
import { useTooltip } from "../components/Tooltip";
import { unitFor, useWidth } from "./size";

export interface ColumnPoint {
  label: string;
  value: number;
  details?: string;
}

/**
 * Columns along a time axis: 24px maximum, a 2px gap, 4px rounded tops, clean y ticks,
 * and a few x labels so they never collide.
 */
export function Columns({
  points,
  label,
  unit = "",
  height = 180,
  color = "var(--s1)",
}: {
  points: ColumnPoint[];
  label: string;
  /** Unit after each value; a pair picks the singular form for 1. */
  unit?: string | readonly [string, string];
  height?: number;
  color?: string;
}) {
  const [ref, width] = useWidth<HTMLDivElement>();
  const tooltip = useTooltip();
  const left = 44;
  const bottom = 22;
  const top = 8;
  const plotWidth = Math.max(40, width - left - 8);
  const plotHeight = height - bottom - top;
  const max = Math.max(0, ...points.map((p) => p.value));
  const y = linearScale(max, [plotHeight, 0]);
  const slots = bands(points.length, plotWidth);
  const every = Math.max(1, Math.ceil((points.length * 58) / plotWidth));
  return (
    <div ref={ref}>
      <svg className="chart" width={width} height={height} role="img" aria-label={label}>
        <g transform={`translate(${left},${top})`}>
          {y.ticks.map((tick) => (
            <g key={tick}>
              <line className="grid-line" x1={0} x2={plotWidth} y1={y(tick)} y2={y(tick)} />
              <text x={-8} y={y(tick) + 4} textAnchor="end">
                {thousands(tick)}
              </text>
            </g>
          ))}
          {points.map((point, index) => {
            const slot = slots[index];
            if (!slot) {
              return null;
            }
            const barTop = y(point.value);
            const barHeight = Math.max(point.value > 0 ? 1 : 0, plotHeight - barTop);
            const r = Math.min(4, barHeight, slot.size / 2);
            const x = slot.offset;
            const path =
              barHeight > 0
                ? `M${x},${plotHeight} V${plotHeight - barHeight + r} Q${x},${plotHeight - barHeight} ${x + r},${plotHeight - barHeight} H${x + slot.size - r} Q${x + slot.size},${plotHeight - barHeight} ${x + slot.size},${plotHeight - barHeight + r} V${plotHeight} Z`
                : "";
            const text = `${thousands(point.value)}${unitFor(unit, point.value)}`;
            return (
              <g
                key={`${point.label}-${index}`}
                className="mark"
                tabIndex={0}
                aria-label={`${point.label}: ${text}`}
                {...tooltip.bind({ value: text, label: point.label, details: point.details })}
              >
                <rect
                  x={x - 1}
                  y={0}
                  width={slot.size + 2}
                  height={plotHeight}
                  fill="transparent"
                  data-hit
                />
                <path d={path} fill={color} />
                {index % every === 0 ? (
                  <text x={x + slot.size / 2} y={plotHeight + 15} textAnchor="middle">
                    {point.label}
                  </text>
                ) : null}
              </g>
            );
          })}
          <line className="axis-line" x1={0} x2={plotWidth} y1={plotHeight} y2={plotHeight} />
        </g>
      </svg>
      {tooltip.element}
    </div>
  );
}
