import { describe, expect, it } from "vitest";
import { continuousDays } from "./activity";

describe("daily activity validity", () => {
  it("rejects duplicated dates rather than silently losing metrics", () => {
    expect(continuousDays([
      { date: "2026-10-21", commits: 3, churn: 15 },
      { date: "2026-10-21", commits: 5, churn: 20 },
    ])).toBeNull();
  });

  it("still fills gaps between unique dates", () => {
    expect(continuousDays([
      { date: "2026-10-20", commits: 1, churn: 2 },
      { date: "2026-10-22", commits: 2, churn: 3 },
    ])?.map((day) => day.date)).toEqual(["2026-10-20", "2026-10-21", "2026-10-22"]);
  });
});
