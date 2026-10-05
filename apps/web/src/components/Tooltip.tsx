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
        // Beside the pointer, but never past either edge of a narrow window.
        left: Math.max(8, Math.min(state.at.x + 14, window.innerWidth - 340)),
        // Below the pointer in the top half of the window and above it in the bottom
        // half, so the bottom edge does not cut off a tooltip near it.
        ...(state.at.y < window.innerHeight / 2
          ? { top: Math.max(8, state.at.y - 12) }
          : { bottom: Math.max(8, window.innerHeight - state.at.y - 12) }),
      }}
    >
      <strong>{state.content.value}</strong>
      <span>{state.content.label}</span>
      {state.content.details ? <div className="muted">{state.content.details}</div> : null}
    </div>
  ) : null;

  return { bind, hide, element };
}
