// Tests for the server of the web interface package. Run them with
// node --test "packaging/npm/test/*.test.mjs"

import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { request } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { createWebServer, fileFor, HEADERS } from "../web/lib/server.js";

let dir;
let server;
let port;

/** Sends a request with `path` exactly as given, unlike fetch, which tidies it up. */
function get(path, method = "GET") {
  return new Promise((resolve, reject) => {
    const req = request({ host: "127.0.0.1", port, path, method }, (response) => {
      let body = "";
      response.setEncoding("utf8");
      response.on("data", (chunk) => (body += chunk));
      response.on("end", () =>
        resolve({ status: response.statusCode, headers: response.headers, body }),
      );
    });
    req.on("error", reject);
    req.end();
  });
}

before(async () => {
  dir = mkdtempSync(join(tmpdir(), "repodna-web-"));
  const root = join(dir, "dist");
  mkdirSync(join(root, "assets"), { recursive: true });
  writeFileSync(join(root, "index.html"), "<!doctype html><title>RepoDNA</title>");
  writeFileSync(join(root, "assets", "index-abc.js"), "export {};");
  writeFileSync(join(root, "manifest.webmanifest"), "{}");
  mkdirSync(join(root, "engine"));
  writeFileSync(join(root, "engine", "repodna.wasm"), "\0asm");
  writeFileSync(join(dir, "secret.txt"), "not served");
  server = createWebServer(root);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  port = server.address().port;
});

after(async () => {
  await new Promise((resolve) => server.close(resolve));
  rmSync(dir, { recursive: true, force: true });
});

test("serves the page with the security headers, checked on every visit", async () => {
  const response = await get("/");
  assert.equal(response.status, 200);
  assert.equal(response.body, "<!doctype html><title>RepoDNA</title>");
  assert.equal(response.headers["content-type"], "text/html; charset=utf-8");
  assert.equal(response.headers["cache-control"], "no-cache");
  for (const [name, value] of Object.entries(HEADERS)) {
    assert.equal(response.headers[name.toLowerCase()], value, name);
  }
});

test("serves built files with their types, and keeps them", async () => {
  const script = await get("/assets/index-abc.js");
  assert.equal(script.headers["content-type"], "text/javascript; charset=utf-8");
  assert.equal(script.headers["cache-control"], "public, max-age=31536000, immutable");
  const manifest = await get("/manifest.webmanifest?v=1");
  assert.equal(manifest.headers["content-type"], "application/manifest+json");
  // Browsers compile WebAssembly as it arrives only when it has this type.
  const engine = await get("/engine/repodna.wasm?v=1.0.0");
  assert.equal(engine.headers["content-type"], "application/wasm");
});

test("sends the security policy of the container image", () => {
  const nginx = readFileSync(new URL("../../web/nginx.conf", import.meta.url), "utf8");
  assert.ok(
    nginx.includes(`Content-Security-Policy "${HEADERS["Content-Security-Policy"]}"`),
    "packaging/web/nginx.conf and packaging/npm/web/lib/server.js send different policies",
  );
  // The web version analyzes folders with WebAssembly, which the policy has to allow.
  assert.match(HEADERS["Content-Security-Policy"], /script-src 'self' 'wasm-unsafe-eval';/);
});

test("serves nothing outside the interface", async () => {
  for (const path of [
    "/../secret.txt",
    "/%2e%2e/secret.txt",
    "/assets/..%2f..%2fsecret.txt",
    "/%00",
    "/%E0%A4%A",
  ]) {
    const response = await get(path);
    assert.equal(response.status, 404, path);
    assert.doesNotMatch(response.body, /not served/, path);
  }
  assert.equal(fileFor(join(dir, "dist"), "/../secret.txt"), null);
  assert.equal((await get("/missing.js")).status, 404);
});

test("answers HEAD and refuses other methods", async () => {
  const head = await get("/", "HEAD");
  assert.equal(head.status, 200);
  assert.equal(head.body, "");
  const post = await get("/", "POST");
  assert.equal(post.status, 405);
  assert.equal(post.headers.allow, "GET, HEAD");
});
