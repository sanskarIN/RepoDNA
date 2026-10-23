import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const tauri = JSON.parse(readFileSync("apps/desktop/src-tauri/tauri.conf.json", "utf8"));
const main = tauri.app.windows.find((window) => window.label === "main");

test("desktop opens a centered resizable window instead of covering the screen", () => {
  assert.ok(main, "missing desktop main window");
  assert.equal(main.center, true);
  assert.equal(main.resizable, true);
  assert.ok(main.width <= 1200, "desktop default is too wide for a normal monitor");
  assert.ok(main.height <= 750, "desktop default should leave room for the taskbar");
  assert.ok(main.minWidth < main.width && main.minHeight < main.height);
});

test("desktop still permits compact windows", () => {
  assert.ok(main.minWidth <= 800);
  assert.ok(main.minHeight <= 560);
});
