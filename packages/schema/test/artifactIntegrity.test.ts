import { describe, expect, it } from "vitest";
import {
  artifactShape,
  inspectArtifact,
  inspectArtifactFile,
  integrityReportJson,
  MAX_INTEGRITY_FILE_BYTES,
} from "../src/artifactIntegrity";

async function rejected(promise: Promise<unknown>): Promise<Error> {
  try { await promise; } catch (reason) { if (reason instanceof Error) return reason; throw reason; }
  throw new Error("Expected rejection");
}

describe("checked-in RepoDNA artifact schema integration", () => {
  it("compiles the full Rust-generated schema and reuses its compiled references", () => {
    const shape = artifactShape();
    expect(artifactShape()).toBe(shape);
    expect(shape.references.size > 50).toBe(true);
  });

  it("reports absent required sections using the checked-in schema", async () => {
    const result = await inspectArtifact({ schemaVersion: "1.0" });
    expect(result.compatibility).toBe("same-version");
    expect(result.checkedAgainst).toBe("1.0");
    expect(result.status).toBe("invalid");
    expect(result.issues.map((issue) => issue.path)).toContain("/analysisMetadata");
    expect(result.issues.map((issue) => issue.path)).toContain("/structure");
  });

  it("qualifies newer and older version claims independently of shape diagnostics", async () => {
    expect((await inspectArtifact({ schemaVersion: "1.5" })).compatibility).toBe("different-minor");
    expect((await inspectArtifact({ schemaVersion: "2.0" })).compatibility).toBe("different-major");
    expect((await inspectArtifact({ schemaVersion: "0.9" })).compatibility).toBe("different-major");
    expect((await inspectArtifact({ schemaVersion: "invalid" })).compatibility).toBe("unknown");
    expect((await inspectArtifact(null)).artifactVersion).toBeNull();
  });

  it("rejects non-JSON documents without echoing document contents", async () => {
    const failure = await rejected(inspectArtifactFile(new Blob(["sensitive invalid JSON"])));
    expect(failure.message).toBe("The selected document is not valid JSON.");
  });

  it("checks file limits before asking the browser to read the file", async () => {
    let reads = 0;
    const file = { size: MAX_INTEGRITY_FILE_BYTES + 1, text: async () => { reads++; return "{}"; } } as Blob;
    expect((await rejected(inspectArtifactFile(file))).message).toContain("64 MiB");
    expect(reads).toBe(0);
  });

  it("supports cancellation while a local file read is pending", async () => {
    const controller = new AbortController();
    let deliver: (text: string) => void = () => undefined;
    const file = { size: 10, text: () => new Promise<string>((resolve) => { deliver = resolve; }) } as Blob;
    const task = inspectArtifactFile(file, { signal: controller.signal });
    controller.abort();
    deliver("{}");
    expect((await rejected(task)).name).toBe("AbortError");
  });

  it("exports diagnostic-only JSON with a stable report format", async () => {
    const result = await inspectArtifactFile(new Blob([JSON.stringify({ schemaVersion: "1.0", identity: { name: "sensitive-project" } })]));
    const json = integrityReportJson(result);
    expect(JSON.parse(json).reportFormat).toBe("repodna-artifact-integrity-1");
    expect(JSON.parse(json).checkedAgainst).toBe("1.0");
    expect(json.includes("sensitive-project")).toBe(false);
  });
});
