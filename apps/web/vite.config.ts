import { readFileSync } from "node:fs";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

const { version } = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8")) as {
  version: string;
};

// Relative asset paths let the same build run from `repodna serve`, the desktop app, and
// any static host.
export default defineConfig({
  plugins: [react()],
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
