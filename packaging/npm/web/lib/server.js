// A small static file server for the web interface, with the headers `repodna serve` and
// the container image send. It serves files only: analyses are opened in the browser and
// never sent to it.

import { createReadStream, statSync } from "node:fs";
import { createServer } from "node:http";
import { extname, join, resolve, sep } from "node:path";

/** The headers every response carries. */
export const HEADERS = {
  "Content-Security-Policy":
    "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'",
  "X-Content-Type-Options": "nosniff",
  "Referrer-Policy": "no-referrer",
  "X-Frame-Options": "DENY",
  "Cross-Origin-Opener-Policy": "same-origin",
};

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".webmanifest": "application/manifest+json",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".ico": "image/x-icon",
  ".woff2": "font/woff2",
  ".wasm": "application/wasm",
  ".txt": "text/plain; charset=utf-8",
};

/** The file under `root` that a request path names, or null when there is none. */
export function fileFor(root, pathname) {
  let decoded;
  try {
    decoded = decodeURIComponent(pathname);
  } catch {
    return null;
  }
  if (decoded.includes("\0")) {
    return null;
  }
  const base = resolve(root);
  const path = resolve(join(base, decoded));
  if (path !== base && !path.startsWith(base + sep)) {
    return null;
  }
  try {
    const stat = statSync(path);
    if (stat.isDirectory()) {
      const index = join(path, "index.html");
      return statSync(index).isFile() ? index : null;
    }
    return stat.isFile() ? path : null;
  } catch {
    return null;
  }
}

/** A server for the built interface in `root`. */
export function createWebServer(root) {
  return createServer((request, response) => {
    for (const [name, value] of Object.entries(HEADERS)) {
      response.setHeader(name, value);
    }
    if (request.method !== "GET" && request.method !== "HEAD") {
      response.writeHead(405, { Allow: "GET, HEAD", "Content-Type": "text/plain; charset=utf-8" });
      response.end("Only GET and HEAD are served.\n");
      return;
    }
    const { pathname } = new URL(request.url ?? "/", "http://localhost");
    const file = fileFor(root, pathname);
    if (!file) {
      response.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
      response.end("Not found.\n");
      return;
    }
    // Built files have their content in their names, so they can be kept; the page is
    // checked on every visit, so a new version takes effect at once.
    const cache = file.includes(`${sep}assets${sep}`)
      ? "public, max-age=31536000, immutable"
      : "no-cache";
    response.writeHead(200, {
      "Content-Type": TYPES[extname(file).toLowerCase()] ?? "application/octet-stream",
      "Cache-Control": cache,
    });
    if (request.method === "HEAD") {
      response.end();
      return;
    }
    createReadStream(file).pipe(response);
  });
}
