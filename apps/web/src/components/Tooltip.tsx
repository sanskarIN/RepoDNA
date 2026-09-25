import { useCallback, useState, type FocusEvent, type PointerEvent, type ReactNode } from "react";

export interface TooltipContent {
  /** The value, shown first and strongest. */
  value: ReactNode;
  /** What the value belongs to. */
  label: ReactNode;
  /** Optional extra lines. */
  details?: ReactNode;
}

interface Position {
  x: number;
  y: number;
}

/**
 * One tooltip per chart. Marks call `bind(content)` to get pointer and focus handlers, so
 * keyboard users get the same details as pointer users. Content is rendered as React
 * text, never as HTML.
 */
export function useTooltip() {
  const [state, setState] = useState<{ content: TooltipContent; at: Position } | null>(null);

  const hide = useCallback(() => setState(null), []);

  const bind = useCallback(
    (content: TooltipContent) => ({
      onPointerMove: (event: PointerEvent<Element>) =>
        setState({ content, at: { x: event.clientX, y: event.clientY } }),
      onPointerLeave: hide,
      onFocus: (event: FocusEvent<Element>) => {
        const box = event.currentTarget.getBoundingClientRect();
        setState({ content, at: { x: box.left + box.width / 2, y: box.top } });
      },
      onBlur: hide,
    }),
    [hide],
  );

  const element = state ? (
    <div
      className="chart-tooltip"
      role="status"
      style={{
        left: Math.min(state.at.x + 14, window.innerWidth - 340),
        top: Math.max(8, state.at.y - 12),
      }}
    >
      <strong>{state.content.value}</strong>
      <span>{state.content.label}</span>
      {state.content.details ? <div className="muted">{state.content.details}</div> : null}
    </div>
  ) : null;

  return { bind, hide, element };
}
