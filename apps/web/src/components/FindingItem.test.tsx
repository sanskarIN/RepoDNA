import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import type { Finding } from "@repodna/schema";
import { FindingItem } from "./FindingItem";

function sample(evidence?: Finding["evidence"]): Finding {
  return {
    id: "duplicate-id",
    rule: "test.legacy",
    title: "Legacy imported finding",
    summary: "Review this record.",
    severity: "warning",
    category: "security",
    confidence: "medium",
    evidence,
    paths: [],
  } as unknown as Finding;
}

describe("FindingItem", () => {
  it("renders imported findings with no evidence array", () => {
    render(<FindingItem finding={sample()} />);
    expect(screen.getByText("Legacy imported finding")).toBeTruthy();
    expect(screen.getByText("Evidence and method")).toBeTruthy();
  });

  it("keeps available evidence visible in the document", () => {
    render(<FindingItem finding={sample([{ kind: "observation", text: "Verified" }])} />);
    expect(screen.getByText("Verified")).toBeTruthy();
  });
});
