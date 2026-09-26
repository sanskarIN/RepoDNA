import { percent } from "@repodna/visualization";
import { useTooltip } from "../components/Tooltip";
import { useWidth } from "./size";

export interface Share {
  label: string;
  value: number;
  color: string;
}

/**
 * Part-to-whole as one stacked bar with 2px surface gaps and a legend (identity never
 * depends on color alone). Shares are relative to the total of `shares`.
 */
export function ShareBar({ shares, label }: { shares: Share[]; label: string }) {
  const [ref, width] = useWidth<HTMLDivElement>();
  const tooltip = useTooltip();
  const total = shares.reduce((sum, share) => sum + share.value, 0);
  const height = 22;
  const gap = 2;
  let x = 0;
  const usable = width - gap * Math.max(0, shares.length - 1);
  return (
    <div ref={ref}>
      <svg className="chart" width={width} height={height} role="img" aria-label={label}>
        {shares.map((share, index) => {
          const size = total > 0 ? (share.value / total) * usable : 0;
          const start = x;
          x += size + gap;
          const text = percent(total > 0 ? share.value / total : 0);
          const first = index === 0;
          const last = index === shares.length - 1;
          return (
            <g
              key={share.label}
              className="mark"
              tabIndex={0}
              aria-label={`${share.label}: ${text}`}
              {...tooltip.bind({ value: text, label: share.label })}
            >
              <rect
                x={start}
                y={0}
                width={Math.max(0, size)}
                height={height}
                rx={first || last ? 4 : 0}
                fill={share.color}
              />
            </g>
          );
        })}
      </svg>
      <div className="legend">
        {shares.map((share) => (
          <span key={share.label}>
            <i style={{ background: share.color }} aria-hidden="true" />
            {share.label} {percent(total > 0 ? share.value / total : 0)}
          </span>
        ))}
      </div>
      {tooltip.element}
    </div>
  );
}
