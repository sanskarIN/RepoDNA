import { WEEKDAYS, heatCells, thousands } from "@repodna/visualization";
import { useTooltip } from "../components/Tooltip";
import { useWidth } from "./size";

/** Commits by weekday and hour on a single-hue ramp (more commits, darker). */
export function Heatmap({ grid, label }: { grid: number[][]; label: string }) {
  const [ref, width] = useWidth<HTMLDivElement>();
  const tooltip = useTooltip();
  const left = 36;
  const top = 16;
  const cell = Math.max(8, Math.min(26, Math.floor((width - left) / 24)) - 2);
  const step = cell + 2;
  const cells = heatCells(grid);
  const height = top + WEEKDAYS.length * step + 4;
  return (
    <div ref={ref}>
      <svg className="chart" width={width} height={height} role="img" aria-label={label}>
        {[0, 6, 12, 18].map((hour) => (
          <text key={hour} x={left + hour * step} y={11}>
            {`${String(hour).padStart(2, "0")}:00`}
          </text>
        ))}
        {WEEKDAYS.map((day, row) => (
          <text key={day} x={0} y={top + row * step + cell / 2 + 4}>
            {day}
          </text>
        ))}
        {cells.map((c) => {
          const text = `${thousands(c.value)} commits`;
          const when = `${WEEKDAYS[c.row] ?? ""} ${String(c.column).padStart(2, "0")}:00–${String(c.column).padStart(2, "0")}:59`;
          return (
            <rect
              key={`${c.row}-${c.column}`}
              className="mark"
              tabIndex={-1}
              x={left + c.column * step}
              y={top + c.row * step}
              width={cell}
              height={cell}
              rx={3}
              fill={`var(--seq${c.level})`}
              {...tooltip.bind({ value: text, label: when })}
            />
          );
        })}
      </svg>
      <div className="legend" aria-hidden="true">
        <span>Fewer</span>
        {[0, 1, 2, 3, 4, 5, 6].map((level) => (
          <i key={level} style={{ background: `var(--seq${level})` }} />
        ))}
        <span>More</span>
      </div>
      {tooltip.element}
    </div>
  );
}
