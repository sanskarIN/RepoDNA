import { describe, expect, it } from "vitest";
import type { Finding } from "@repodna/schema";
import { findingRenderKeys } from "./finding-render-keys";

function finding(id: string): Finding {
  return { id } as Finding;
}

describe("stable findings render identities", () => {
  it("keeps duplicate IDs distinct across filtering and reordering", () => {
    const first = finding("same-id");
    const second = finding("same-id");
    const third = finding("unique-id");
    const keys = findingRenderKeys([first, second, third], "analysis-one");
    expect(keys.get(first)).toBe("analysis-one:0");
    expect(keys.get(second)).toBe("analysis-one:1");
    expect(keys.get(third)).toBe("analysis-one:2");
    const filtered = [second, first];
    expect(filtered.map((item) => keys.get(item))).toEqual([
      "analysis-one:1",
      "analysis-one:0",
    ]);
  });

  it("changes keys across analyses without mutating frozen findings", () => {
    const record = Object.freeze(finding(""));
    const previous = findingRenderKeys([record], "analysis-one");
    const current = findingRenderKeys([record], "analysis-two");
    expect(previous.get(record)).not.toBe(current.get(record));
    expect(Object.keys(record)).toEqual(["id"]);
  });
});
