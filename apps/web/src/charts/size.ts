import { useEffect, useRef, useState } from "react";

/** Tracks an element's width so charts render at real pixels (crisp text, no scaling). */
export function useWidth<T extends HTMLElement>(fallback = 640) {
  const ref = useRef<T | null>(null);
  const [width, setWidth] = useState(fallback);
  useEffect(() => {
    const element = ref.current;
    if (!element) {
      return;
    }
    const update = () => {
      const next = Math.floor(element.getBoundingClientRect().width);
      if (next > 0) {
        setWidth(next);
      }
    };
    update();
    if (typeof ResizeObserver === "undefined") {
      return;
    }
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  return [ref, width] as const;
}

/** Approximate rendered width of text at 11–12px, for layout decisions. */
export function textWidth(text: string, size = 11): number {
  return text.length * size * 0.56;
}

/** Shortens text to about `max` characters, keeping the end of paths. */
export function shorten(text: string, max: number): string {
  if (text.length <= max) {
    return text;
  }
  return `…${text.slice(text.length - max + 1)}`;
}

/** The unit to print after `value`: a pair holds the singular and plural forms. */
export function unitFor(unit: string | readonly [string, string], value: number): string {
  return typeof unit === "string" ? unit : value === 1 ? unit[0] : unit[1];
}
