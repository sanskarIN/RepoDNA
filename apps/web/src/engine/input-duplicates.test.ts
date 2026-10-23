import { describe, expect, it } from "vitest";
import { folderFromList } from "./input";

function selected(path: string): File {
  const file = new File(["const ok = true;"], path.split("/").at(-1) ?? "test.ts");
  Object.defineProperty(file, "webkitRelativePath", { value: path });
  return file;
}

describe("browser file import uniqueness", () => {
  it("rejects duplicate relative file paths in a selected folder", () => {
    expect(folderFromList([
      selected("project/src/main.ts"),
      selected("project/src/main.ts"),
    ])).toBeNull();
  });

  it("keeps unique files from a single folder", () => {
    const result = folderFromList([
      selected("project/src/main.ts"),
      selected("project/src/other.ts"),
    ]);
    expect(result?.kind).toBe("folder");
    if (result?.kind !== "folder") throw new Error("Expected a folder");
    expect(result.files.map(({ path }) => path)).toEqual(["src/main.ts", "src/other.ts"]);
  });
});
