import { linearScale, thousands } from "@repodna/visualization";
import { useTooltip } from "../components/Tooltip";
import { shorten, textWidth, unitFor, useWidth } from "./size";

export interface Bar {
  label: string;
  value: number;
  /** Mark color; defaults to the first series color. */
  color?: string;
  /** Extra line for the tooltip. */
  details?: string;
}

/**
 * Horizontal bars with the value at each bar's tip. Bars are at most 24px thick with
 * rounded data ends and a 2px gap; labels use text colors, never the mark color.
 */
export function BarList({
  bars,
  unit = "",
  label,
  format = thousands,
  max: fixedMax,
}: {
  bars: Bar[];
  /** Unit after each value; a pair picks the singular form for 1. */
  unit?: string | readonly [string, string];
  label: string;
  format?: (value: number) => string;
  /** A fixed scale maximum, so several charts can be compared (small multiples). */
  max?: number;
}) {
  const [ref, width] = useWidth<HTMLDivElement>();
  const tooltip = useTooltip();
  const row = 26;
  const labelWidth = Math.min(
    Math.max(80, ...bars.map((bar) => textWidth(shorten(bar.label, 42)) + 12)),
    width * 0.42,
  );
  const valueWidth =
    Math.max(...bars.map((bar) => textWidth(`${format(bar.value)}${unitFor(unit, bar.value)}`))) +
    10;
  const plot = Math.max(40, width - labelWidth - valueWidth);
  const max = fixedMax ?? Math.max(0, ...bars.map((bar) => bar.value));
  const scale = linearScale(max, [0, plot], 4, 0, false);
  const height = bars.length * row + 4;
  return (
    <div ref={ref}>
      <svg className="chart" width={width} height={height} role="img" aria-label={label}>
        {bars.map((bar, index) => {
          const y = index * row + 4;
          const length = Math.max(bar.value > 0 ? 2 : 0, scale(bar.value));
          const thickness = Math.min(24, row - 8);
          const text = `${format(bar.value)}${unitFor(unit, bar.value)}`;
          return (
            <g
              key={`${bar.label}-${index}`}
              tabIndex={0}
              className="mark"
              aria-label={`${bar.label}: ${text}`}
              {...tooltip.bind({ value: text, label: bar.label, details: bar.details })}
            >
              <rect x={0} y={y - 2} width={width} height={row} fill="transparent" data-hit />
              <text
                className="label"
                x={labelWidth - 10}
                y={y + thickness / 2 + 4}
                textAnchor="end"
              >
                {shorten(bar.label, 42)}
              </text>
              <path
                d={roundedBar(labelWidth, y, length, thickness)}
                fill={bar.color ?? "var(--s1)"}
              />
              <text className="value" x={labelWidth + length + 6} y={y + thickness / 2 + 4}>
                {text}
              </text>
            </g>
          );
        })}
      </svg>
      {tooltip.element}
    </div>
  );
}

/** A horizontal bar with a 4px rounded end and a square base. */
export function roundedBar(x: number, y: number, length: number, thickness: number): string {
  if (length <= 0) {
    return "";
  }
  const r = Math.min(4, length, thickness / 2);
  return `M${x},${y} H${x + length - r} Q${x + length},${y} ${x + length},${y + r} V${y + thickness - r} Q${x + length},${y + thickness} ${x + length - r},${y + thickness} H${x} Z`;
}
