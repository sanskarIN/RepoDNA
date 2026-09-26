import type { DailyActivity } from "@repodna/schema";

/** Histories spanning fewer calendar months than this are charted per day. */
export const MIN_MONTHS_FOR_MONTHLY_CHART = 3;

/** Longest span, in days, that is charted per day. */
const MAX_DAYS = 120;

const DAY_MS = 86_400_000;

function dayNumber(date: string): number | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date);
  if (!match) {
    return null;
  }
  return Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3])) / DAY_MS;
}

/**
 * Every day from the first to the last day with commits, including days without any,
 * or null when the span is empty, longer than 120 days, or has unreadable dates.
 */
export function continuousDays(days: readonly DailyActivity[]): DailyActivity[] | null {
  const byDay = new Map<number, DailyActivity>();
  for (const day of days) {
    const number = dayNumber(day.date);
    if (number === null) {
      return null;
    }
    byDay.set(number, day);
  }
  if (byDay.size === 0) {
    return null;
  }
  const first = Math.min(...byDay.keys());
  const last = Math.max(...byDay.keys());
  if (last - first + 1 > MAX_DAYS) {
    return null;
  }
  const series: DailyActivity[] = [];
  for (let number = first; number <= last; number += 1) {
    series.push(
      byDay.get(number) ?? {
        date: new Date(number * DAY_MS).toISOString().slice(0, 10),
        commits: 0,
        churn: 0,
      },
    );
  }
  return series;
}
