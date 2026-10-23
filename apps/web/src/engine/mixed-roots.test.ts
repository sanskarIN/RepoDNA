import { describe, expect, it } from "vitest";
import { folderFromList } from "./input";

function selected(path: string): File {
  const file = new File(["const x = 1;"], path.split("/").at(-1) ?? "file.ts");
  Object.defineProperty(file, "webkitRelativePath", { value: path });
  return file;
}

describe("browser directory selection roots", () => {
  it("imports files that share one selected directory", () => {
    const result = folderFromList([
      selected("project/src/main.ts"),
      selected("project/src/util.ts"),
    ]);
    expect(result?.kind).toBe("folder");
    if (result?.kind !== "folder") throw new Error("Expected folder");
    expect(result.name).toBe("project");
    expect(result.files).toHaveLength(2);
  });

  it("refuses to merge unrelated roots into the same analysis", () => {
    expect(folderFromList([
      selected("first/src/main.ts"),
      selected("other/src/main.ts"),
    ])).toBeNull();
  });
});
