// Scales and ticks with clean, human-friendly numbers.

/** A linear mapping from a domain to a range. */
export interface LinearScale {
  (value: number): number;
  domain: readonly [number, number];
  range: readonly [number, number];
  ticks: number[];
}

/** A "nice" step for about `count` intervals over `span`: 1, 2, or 5 times a power of 10. */
export function niceStep(span: number, count: number): number {
  if (!(span > 0) || count < 1) {
    return 1;
  }
  const raw = span / count;
  const power = 10 ** Math.floor(Math.log10(raw));
  const fraction = raw / power;
  const nice = fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 5 ? 5 : 10;
  return nice * power;
}

/**
 * A linear scale from 0 (or `min`) to a nice maximum covering `max`, with ticks. With
 * `integer` (the default, for counts), ticks never fall between whole numbers.
 */
export function linearScale(
  max: number,
  range: readonly [number, number],
  count = 4,
  min = 0,
  integer = true,
): LinearScale {
  const raw = niceStep(Math.max(max - min, 0) || 1, count);
  const step = integer ? Math.max(1, raw) : raw;
  const top = Math.max(step, Math.ceil(max / step) * step);
  const bottom = Math.floor(min / step) * step;
  const ticks: number[] = [];
  for (let tick = bottom; tick <= top + step / 2; tick += step) {
    ticks.push(Math.round(tick * 1e9) / 1e9);
  }
  const [r0, r1] = range;
  const scale = ((value: number) =>
    top === bottom ? r0 : r0 + ((value - bottom) / (top - bottom)) * (r1 - r0)) as LinearScale;
  scale.domain = [bottom, top];
  scale.range = range;
  scale.ticks = ticks;
  return scale;
}

/** Positions for `count` bands across `length`, each at most `maxBand` thick. */
export function bands(
  count: number,
  length: number,
  maxBand = 24,
  gap = 2,
): { offset: number; size: number }[] {
  if (count <= 0) {
    return [];
  }
  const slot = length / count;
  const size = Math.max(1, Math.min(maxBand, slot - gap));
  return Array.from({ length: count }, (_, index) => ({
    offset: index * slot + (slot - size) / 2,
    size,
  }));
}
