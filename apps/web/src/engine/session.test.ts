import { describe, expect, it } from "vitest";
import { analysisFileName } from "./session";

describe("browser analysis download names", () => {
  it("removes archive extensions without leaving unsafe path separators", () => {
    expect(analysisFileName("project.tar.gz")).toBe("project.repodna");
    expect(analysisFileName("../other\\\\repo.zip")).toBe("..-other--repo.repodna");
  });

  it("handles reserved Windows names, forbidden characters and empty input", () => {
    expect(analysisFileName("CON.zip")).toBe("repodna-CON.repodna");
    expect(analysisFileName("bad:name?.tar")).toBe("bad-name-.repodna");
    expect(analysisFileName("...")).toBe("repository.repodna");
    expect(analysisFileName("z".repeat(500))).toBe("z".repeat(100) + ".repodna");
  });
});
