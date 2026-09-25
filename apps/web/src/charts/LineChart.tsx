import { useState } from "react";
import { linearScale, thousands } from "@repodna/visualization";
import { useWidth } from "./size";

export interface LineSeries {
  label: string;
  color: string;
  values: number[];
}

/**
 * Lines over ordered labels (dates or snapshots): 2px strokes, a 10% area wash for a single
 * series, and a crosshair that snaps to the nearest point and lists every series there.
 */
export function LineChart({
  labels,
  series,
  label,
  height = 200,
}: {
  labels: string[];
  series: LineSeries[];
  label: string;
  height?: number;
}) {
  const [ref, width] = useWidth<HTMLDivElement>();
  const [hover, setHover] = useState<number | null>(null);
  const left = 48;
  const right = 12;
  const top = 10;
  const bottom = 24;
  const plotWidth = Math.max(40, width - left - right);
  const plotHeight = height - top - bottom;
  const max = Math.max(0, ...series.flatMap((s) => s.values));
  const y = linearScale(max, [plotHeight, 0]);
  const x = (index: number) =>
    labels.length > 1 ? (index / (labels.length - 1)) * plotWidth : plotWidth / 2;
  const every = Math.max(1, Math.ceil((labels.length * 80) / plotWidth));

  const onMove = (clientX: number, box: DOMRect) => {
    if (labels.length === 0) {
      return;
    }
    const relative = clientX - box.left - left;
    const index = Math.round((relative / plotWidth) * (labels.length - 1));
    setHover(Math.min(labels.length - 1, Math.max(0, index)));
  };

  return (
    <div ref={ref}>
      <svg
        className="chart"
        width={width}
        height={height}
        role="img"
        aria-label={label}
        onPointerMove={(event) =>
          onMove(event.clientX, event.currentTarget.getBoundingClientRect())
        }
        onPointerLeave={() => setHover(null)}
      >
        <g transform={`translate(${left},${top})`}>
          {y.ticks.map((tick) => (
            <g key={tick}>
              <line className="grid-line" x1={0} x2={plotWidth} y1={y(tick)} y2={y(tick)} />
              <text x={-8} y={y(tick) + 4} textAnchor="end">
                {thousands(tick)}
              </text>
            </g>
          ))}
          {labels.map((text, index) =>
            index % every === 0 || index === labels.length - 1 ? (
              <text key={`${text}-${index}`} x={x(index)} y={plotHeight + 16} textAnchor="middle">
                {text}
              </text>
            ) : null,
          )}
          {series.map((s) => {
            const points = s.values.map((v, i) => `${x(i)},${y(v)}`).join(" ");
            return (
              <g key={s.label}>
                {series.length === 1 && s.values.length > 1 ? (
                  <polygon
                    points={`${x(0)},${plotHeight} ${points} ${x(s.values.length - 1)},${plotHeight}`}
                    fill={s.color}
                    opacity={0.1}
                  />
                ) : null}
                <polyline
                  points={points}
                  fill="none"
                  stroke={s.color}
                  strokeWidth={2}
                  strokeLinejoin="round"
                  strokeLinecap="round"
                />
                {s.values.length > 0 ? (
                  <circle
                    cx={x(s.values.length - 1)}
                    cy={y(s.values[s.values.length - 1] ?? 0)}
                    r={4}
                    fill={s.color}
                    stroke="var(--surface)"
                    strokeWidth={2}
                  />
                ) : null}
              </g>
            );
          })}
          <line className="axis-line" x1={0} x2={plotWidth} y1={plotHeight} y2={plotHeight} />
          {hover !== null ? (
            <g>
              <line className="axis-line" x1={x(hover)} x2={x(hover)} y1={0} y2={plotHeight} />
              {series.map((s) => (
                <circle
                  key={s.label}
                  cx={x(hover)}
                  cy={y(s.values[hover] ?? 0)}
                  r={4}
                  fill={s.color}
                  stroke="var(--surface)"
                  strokeWidth={2}
                />
              ))}
            </g>
          ) : null}
        </g>
      </svg>
      {series.length > 1 ? (
        <div className="legend">
          {series.map((s) => (
            <span key={s.label}>
              <i style={{ background: s.color, height: 2, width: 14 }} aria-hidden="true" />
              {s.label}
            </span>
          ))}
        </div>
      ) : null}
      {hover !== null ? (
        <div className="muted" aria-live="polite">
          <strong>{labels[hover]}</strong>:{" "}
          {series.map((s) => `${s.label} ${thousands(s.values[hover] ?? 0)}`).join(" · ")}
        </div>
      ) : (
        <div className="muted" aria-hidden="true">
          &nbsp;
        </div>
      )}
    </div>
  );
}
