import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// Run from apps/web, as Vitest does when called through the workspace.
const web = join(process.cwd(), "public");
const desktop = join(process.cwd(), "../desktop/src-tauri/icons");

function pngSize(path: string): [number, number] {
  const bytes = readFileSync(path);
  expect(bytes.subarray(0, 8).equals(Buffer.from("89504e470d0a1a0a", "hex"))).toBe(true);
  return [bytes.readUInt32BE(16), bytes.readUInt32BE(20)];
}

describe("v1.4.0 consistent application branding", () => {
  it("keeps the vector source as an accessible, hand-authored SVG", () => {
    const source = readFileSync(join(web, "favicon.svg"), "utf8");
    expect(source).toContain('viewBox="0 0 128 128"');
    expect(source).toContain('<title id="title">RepoDNA</title>');
    expect(source).toContain("<path");
    expect(source).toContain("<circle");
    expect(source).not.toContain("data:image/");
    expect(source).not.toContain("<image ");
  });

  it("keeps app icons at the sizes declared in the manifest", () => {
    const manifest = JSON.parse(readFileSync(join(web, "manifest.webmanifest"), "utf8")) as {
      icons: Array<{ src: string; sizes: string; type: string }>;
    };
    for (const icon of manifest.icons) {
      if (icon.type !== "image/png") {
        continue;
      }
      const size = Number(icon.sizes.split("x")[0]);
      expect(pngSize(join(web, icon.src))).toEqual([size, size]);
    }
    expect(pngSize(join(web, "icons/apple-touch-icon.png"))).toEqual([180, 180]);
  });

  it("keeps the required Tauri icons generated from the same mark", () => {
    expect(pngSize(join(desktop, "32x32.png"))).toEqual([32, 32]);
    expect(pngSize(join(desktop, "128x128.png"))).toEqual([128, 128]);
    expect(pngSize(join(desktop, "128x128@2x.png"))).toEqual([256, 256]);
    expect(readFileSync(join(desktop, "icon.ico")).subarray(0, 4)).toEqual(
      Buffer.from([0, 0, 1, 0]),
    );
    expect(readFileSync(join(desktop, "icon.icns")).subarray(0, 4).toString()).toBe("icns");
  });
});
