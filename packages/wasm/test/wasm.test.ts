import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { Directory, File as WasiFile } from "@bjorn3/browser_wasi_shim";
import { afterAll, describe, expect, it, vi } from "vitest";
import { bytesFile, fileFileSystem, folderFileSystem } from "../src/files";
import { keep } from "../src/paths";

/** The entry at `path` in a file system root, or undefined. */
function at(root: Map<string, unknown>, path: string): unknown {
  let current: unknown = root;
  for (const part of path.split("/")) {
    const contents: Map<string, unknown> | undefined =
      current instanceof Map ? current : (current as Directory | undefined)?.contents;
    current = contents?.get(part);
  }
  return current;
}

describe("file system", () => {
  it("skips version control", () => {
    expect(keep("src/main.rs")).toBe(true);
    expect(keep(".git/HEAD")).toBe(false);
    expect(keep("vendor/lib/.svn/entries")).toBe(false);
    expect(keep(".github/workflows/ci.yml")).toBe(true);
  });

  it("puts a folder under /work and reads each file once, when opened", () => {
    const read = vi.fn(() => new Uint8Array(13));
    const file = (path: string) => ({ path, size: 13, read });
    const root = folderFileSystem("widget", [
      file("src/lib.rs"),
      file("./README.md"),
      file("../escape.txt"),
      file("src/../../escape.txt"),
      file(".git/HEAD"),
    ]);
    expect(at(root, "tmp")).toBeInstanceOf(Directory);
    const lib = at(root, "work/widget/src/lib.rs") as WasiFile;
    expect(lib).toBeInstanceOf(WasiFile);
    expect(at(root, "work/widget/README.md")).toBeInstanceOf(WasiFile);
    expect(at(root, "work/widget/.git")).toBeUndefined();
    expect(at(root, "work/escape.txt")).toBeUndefined();
    expect(at(root, "work/widget/escape.txt")).toBeUndefined();

    // Listing asks for sizes, which do not need the contents.
    expect(lib.size).toBe(13n);
    expect(lib.stat().size).toBe(13n);
    expect(read).not.toHaveBeenCalled();
    expect(lib.data.byteLength).toBe(13);
    expect(lib.data.byteLength).toBe(13);
    expect(read).toHaveBeenCalledTimes(1);
    expect(lib.readonly).toBe(true);
  });

  it("puts a single file under /work", () => {
    const root = fileFileSystem("analysis.repodna", bytesFile(new Uint8Array(2)));
    expect((at(root, "work/analysis.repodna") as WasiFile).size).toBe(2n);
  });
});

/** A TAR archive of `files`, in the ustar format. */
function tar(files: Record<string, string>): Uint8Array {
  const encoder = new TextEncoder();
  const blocks: Uint8Array[] = [];
  for (const [name, text] of Object.entries(files)) {
    const content = encoder.encode(text);
    const header = new Uint8Array(512);
    const put = (offset: number, value: string) => header.set(encoder.encode(value), offset);
    const octal = (value: number, width: number) =>
      `${value.toString(8).padStart(width - 1, "0")}\0`;
    put(0, name);
    put(100, octal(0o644, 8));
    put(108, octal(0, 8));
    put(116, octal(0, 8));
    put(124, octal(content.byteLength, 12));
    put(136, octal(1_700_000_000, 12));
    put(148, "        ");
    put(156, "0");
    put(257, "ustar\0");
    put(263, "00");
    const sum = header.reduce((total, byte) => total + byte, 0);
    put(148, `${sum.toString(8).padStart(6, "0")}\0 `);
    blocks.push(header, content, new Uint8Array((512 - (content.byteLength % 512)) % 512));
  }
  blocks.push(new Uint8Array(1024));
  const out = new Uint8Array(blocks.reduce((total, block) => total + block.byteLength, 0));
  let offset = 0;
  for (const block of blocks) {
    out.set(block, offset);
    offset += block.byteLength;
  }
  return out;
}

// The tests below run the program when it has been built:
// cargo build -p repodna-wasm --target wasm32-wasip1 --profile web-engine
const built = fileURLToPath(
  new URL("../../../target/wasm32-wasip1/web-engine/repodna-wasm.wasm", import.meta.url),
);
const program = process.env.REPODNA_WASM ?? (existsSync(built) ? built : null);
if (program) {
  process.env.REPODNA_WASM = program;
}

describe.skipIf(!program)("program", async () => {
  const node = await import("../src/node");
  const dir = mkdtempSync(join(tmpdir(), "repodna-wasm-"));
  // A name in Devanagari, which takes more bytes in UTF-8 than characters.
  const repo = join(dir, "विजेट");
  const write = (path: string, text: string) => {
    mkdirSync(join(repo, path, ".."), { recursive: true });
    writeFileSync(join(repo, path), text);
  };
  write("Cargo.toml", '[package]\nname = "widget"\nversion = "0.1.0"\nedition = "2021"\n');
  write("src/lib.rs", "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n");
  write("README.md", "# Widget\n");
  write(".git/HEAD", "ref: refs/heads/main\n");
  afterAll(() => rmSync(dir, { recursive: true, force: true }));

  it("reports its version", async () => {
    const repodna = await node.load();
    expect(await repodna.version()).toMatch(/^\d+\.\d+\.\d+/);
  });

  it("analyzes a folder, with progress, and makes reports and cards of it", async () => {
    const log = vi.spyOn(console, "log");
    const stages: string[] = [];
    const text = await node.analyze(repo, {
      profile: "quick",
      onProgress: (event) => {
        if (event.event === "stage" && event.status !== "started") {
          stages.push(event.stage);
        }
      },
    });
    const dna = JSON.parse(text);
    expect(dna.identity.name).toBe("विजेट");
    expect(dna.languages.primary).toEqual(["rust"]);
    expect(dna.structure.totalFiles).toBe(3);
    expect(dna.analysisMetadata.platform.os).toBe("wasi");
    expect(stages).toContain("discovery");
    expect(log).not.toHaveBeenCalled();
    log.mockRestore();

    expect(await node.report(text)).toContain("<html");
    expect(await node.report(dna, { format: "markdown", privacy: "public" })).toMatch(/^# /);
    expect(await node.card(text, { dark: true })).toContain("<svg");
    const png = await node.cardPng(text);
    expect(Array.from(png.subarray(1, 4))).toEqual([0x50, 0x4e, 0x47]);
  });

  it("analyzes an archive", async () => {
    const archive = join(dir, "gadget.tar");
    writeFileSync(
      archive,
      tar({ "gadget/package.json": '{"name":"gadget"}\n', "gadget/index.js": "export {};\n" }),
    );
    const dna = JSON.parse(await node.analyze(archive, { profile: "quick" }));
    expect(dna.structure.totalFiles).toBe(2);
    expect(dna.analysisMetadata.input.kind).toBe("archive");
  });

  it("explains failures in the program's own words", async () => {
    const broken = join(dir, "broken.zip");
    writeFileSync(broken, "not a zip");
    await expect(node.analyze(broken)).rejects.toThrow(/^broken\.zip is not a valid archive/);
    await expect(node.analyze(join(dir, "missing"))).rejects.toThrow(/ENOENT/);
    await expect(node.report("{")).rejects.toThrow();
  });
});
