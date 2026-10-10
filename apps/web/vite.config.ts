import { readFileSync, statSync } from "node:fs";
import { resolve } from "node:path";
import react from "@vitejs/plugin-react";
import type { Plugin } from "vite";
import { defineConfig } from "vitest/config";

const { version } = JSON.parse(
  readFileSync(new URL("./package.json", import.meta.url), "utf8"),
) as {
  version: string;
};

// RepoDNA's license and the notices of the third-party software it includes, published next
// to the web interface, which the command line, the desktop app, and the web version all
// contain. The Licenses page shows them.
const LEGAL_FILES: Record<string, string> = {
  "LICENSE.txt": "../../LICENSE",
  "NOTICE.txt": "../../NOTICE",
  "THIRD-PARTY-NOTICES.txt": "../../THIRD-PARTY-NOTICES.txt",
};

function legalFiles(): Plugin {
  const read = (source: string) => readFileSync(new URL(source, import.meta.url));
  return {
    name: "repodna-legal-files",
    generateBundle() {
      for (const [fileName, source] of Object.entries(LEGAL_FILES)) {
        this.emitFile({ type: "asset", fileName, source: read(source) });
      }
    },
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        const [path = ""] = (request.url ?? "").split("?");
        const source = LEGAL_FILES[path.replace(/^\//, "")];
        if (!source) {
          next();
          return;
        }
        response.setHeader("Content-Type", "text/plain; charset=utf-8");
        response.end(read(source));
      });
    },
  };
}

// RepoDNA's analysis as a WebAssembly program (crates/repodna-wasm), which lets the web
// version analyze a folder or an archive in the browser. It is included when REPODNA_ENGINE
// names the built program; `repodna serve` and the desktop app analyze on the machine
// instead, so their builds go without it.
const ENGINE = process.env.REPODNA_ENGINE ? resolve(process.env.REPODNA_ENGINE) : null;

function engineFiles(): Plugin {
  const files = (): Record<string, () => Buffer | string> =>
    ENGINE
      ? {
          "engine/repodna.wasm": () => readFileSync(ENGINE),
          "engine/engine.json": () =>
            `${JSON.stringify({ version, bytes: statSync(ENGINE).size })}\n`,
        }
      : {};
  return {
    name: "repodna-engine",
    buildStart() {
      if (ENGINE) {
        // Fails the build early when the program has not been built.
        statSync(ENGINE);
      }
    },
    generateBundle() {
      for (const [fileName, read] of Object.entries(files())) {
        this.emitFile({ type: "asset", fileName, source: read() });
      }
    },
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        const [path = ""] = (request.url ?? "").split("?");
        const read = files()[path.replace(/^\//, "")];
        if (!read) {
          next();
          return;
        }
        response.setHeader(
          "Content-Type",
          path.endsWith(".wasm") ? "application/wasm" : "application/json; charset=utf-8",
        );
        response.end(read());
      });
    },
  };
}

// Relative asset paths let the same build run from `repodna serve`, the desktop app, and
// any static host.
export default defineConfig({
  plugins: [react(), legalFiles(), engineFiles()],
  base: "./",
  define: {
    __REPODNA_VERSION__: JSON.stringify(version),
  },
  build: {
    outDir: "dist",
    target: "es2022",
    sourcemap: false,
    assetsInlineLimit: 0,
  },
  server: {
    // During development, API requests go to a running `repodna serve`.
    proxy: {
      "/api": { target: "http://127.0.0.1:7878", changeOrigin: true },
    },
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["src/test/setup.ts"],
  },
});
