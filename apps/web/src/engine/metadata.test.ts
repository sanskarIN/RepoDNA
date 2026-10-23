import { describe, expect, it } from "vitest";
import { isInfo } from "./client";

describe("browser engine manifest validation", () => {
  it("accepts a real engine version and positive byte length", () => {
    expect(isInfo({ version: "1.4.2", bytes: 2048 })).toBe(true);
    expect(isInfo({ version: "1.5.0-rc.1", bytes: 1 })).toBe(true);
  });

  it("rejects malformed and nonfinite engine metadata", () => {
    for (const value of [
      null, [], 2, {}, { version: "", bytes: 100 },
      { version: "latest", bytes: 100 },
      { version: "1.4.2", bytes: 0 },
      { version: "1.4.2", bytes: -5 },
      { version: "1.4.2", bytes: NaN },
      { version: "1.4.2", bytes: Infinity },
      { version: "1.4.2", bytes: 2.4 },
    ]) expect(isInfo(value)).toBe(false);
  });
}
