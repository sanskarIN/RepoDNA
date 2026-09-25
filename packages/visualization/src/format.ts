// Number and date formatting shared by charts and tables.

const grouping = new Intl.NumberFormat("en-US");

/** 12345 → "12,345". */
export function thousands(value: number): string {
  return grouping.format(Math.round(value));
}

/** 1284 → "1,284"; 12900 → "12.9K"; 4200000 → "4.2M". */
export function compact(value: number): string {
  const abs = Math.abs(value);
  if (abs < 10_000) {
    return thousands(value);
  }
  const units: [number, string][] = [
    [1e9, "B"],
    [1e6, "M"],
    [1e3, "K"],
  ];
  for (const [size, unit] of units) {
    if (abs >= size) {
      const scaled = value / size;
      return `${scaled >= 100 ? Math.round(scaled) : Math.round(scaled * 10) / 10}${unit}`;
    }
  }
  return thousands(value);
}

/** 0.125 → "12.5%". */
export function percent(ratio: number, digits = 1): string {
  if (!Number.isFinite(ratio)) {
    return "–";
  }
  return `${(ratio * 100).toFixed(digits)}%`;
}

/** Bytes with a binary unit: 1536 → "1.5 KiB". */
export function bytes(value: number): string {
  if (value < 1024) {
    return `${value} B`;
  }
  const units = ["KiB", "MiB", "GiB", "TiB"];
  let amount = value;
  let unit = units[0] ?? "KiB";
  for (const candidate of units) {
    amount /= 1024;
    unit = candidate;
    if (amount < 1024) {
      break;
    }
  }
  return `${amount.toFixed(1)} ${unit}`;
}

/** An RFC 3339 timestamp as a date: "2026-09-24". */
export function date(timestamp: string | null | undefined): string {
  return timestamp ? timestamp.slice(0, 10) : "–";
}

/** Whole days as "3 days", "5 months", or "2.1 years". */
export function span(days: number): string {
  if (days < 1) {
    return "less than a day";
  }
  if (days < 60) {
    return `${Math.round(days)} day${Math.round(days) === 1 ? "" : "s"}`;
  }
  if (days < 730) {
    return `${Math.round(days / 30.44)} months`;
  }
  return `${(days / 365.25).toFixed(1)} years`;
}
