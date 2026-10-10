import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
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
// A relative path is taken from where npm was started, which `npm run build -w` from the
// repository's root leaves for apps/web.
const ENGINE = process.env.REPODNA_ENGINE
  ? resolve(process.env.INIT_CWD ?? process.cwd(), process.env.REPODNA_ENGINE)
  : null;

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

// The service worker that keeps the web version working offline (sw.js). The build gives it
// the files to keep, which are all of them but the analysis program, which it keeps the
// first time it is used, and a version that changes with any of them.
function serviceWorker(): Plugin {
  // A path, not a URL's pathname, which is "/D:/..." on Windows and escapes spaces.
  const publicDir = fileURLToPath(new URL("./public/", import.meta.url));
  const publicFiles = (dir: string): string[] =>
    readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
      entry.isDirectory()
        ? publicFiles(join(dir, entry.name))
        : [relative(publicDir, join(dir, entry.name)).split("\\").join("/")],
    );
  return {
    name: "repodna-service-worker",
    apply: "build",
    // After the page itself is added to the build.
    enforce: "post",
    generateBundle(_options, bundle) {
      const hash = createHash("sha256");
      const files = ["./"];
      for (const [name, output] of Object.entries(bundle).sort(([a], [b]) => a.localeCompare(b))) {
        hash.update(name);
        hash.update(output.type === "chunk" ? output.code : output.source);
        if (!name.startsWith("engine/")) {
          files.push(name);
        }
      }
      for (const name of publicFiles(publicDir).sort()) {
        hash.update(name);
        hash.update(readFileSync(join(publicDir, name)));
        files.push(name);
      }
      const template = readFileSync(new URL("./sw.js", import.meta.url), "utf8");
      this.emitFile({
        type: "asset",
        fileName: "sw.js",
        source: template
          .replace("__VERSION__", `${version}-${hash.digest("hex").slice(0, 12)}`)
          .replace("__FILES__", JSON.stringify(files)),
      });
    },
  };
}

// Relative asset paths let the same build run from `repodna serve`, the desktop app, and
// any static host.
export default defineConfig({
  plugins: [react(), legalFiles(), engineFiles(), serviceWorker()],
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
