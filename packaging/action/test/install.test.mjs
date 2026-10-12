// Tests for the GitHub Action's installer, against a local server that stands in for GitHub's
// releases. Run them with node --test "packaging/action/test/*.test.mjs".

import assert from "node:assert/strict";
import { execFile, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";

const INSTALL = resolve(dirname(fileURLToPath(import.meta.url)), "../install.sh");
const TARGET = "x86_64-unknown-linux-musl";

let dir;
let server;
let base;
let archive;
let sums;
// How many more times a file answers with an error before it answers.
const failures = new Map();

before(async () => {
  dir = mkdtempSync(join(tmpdir(), "repodna-action-"));
  const name = `repodna-9.9.9-${TARGET}`;
  mkdirSync(join(dir, name));
  writeFileSync(join(dir, name, "repodna"), '#!/bin/sh\necho "repodna 9.9.9"\n', { mode: 0o755 });
  execFileSync("tar", ["-czf", `${name}.tar.gz`, name], { cwd: dir });
  archive = readFileSync(join(dir, `${name}.tar.gz`));
  sums = `${createHash("sha256").update(archive).digest("hex")}  ${name}.tar.gz\n`;
  server = createServer((request, response) => {
    const path = new URL(request.url, "http://localhost").pathname;
    const left = failures.get(path) ?? 0;
    if (left > 0) {
      failures.set(path, left - 1);
      response.writeHead(504).end("Gateway Timeout");
      return;
    }
    if (path === "/o/r/releases/latest") {
      response.writeHead(302, { location: "/o/r/releases/tag/v9.9.9" }).end();
    } else if (path === "/o/r/releases/tag/v9.9.9") {
      response.writeHead(200).end("release page");
    } else if (path === `/o/r/releases/download/v9.9.9/${name}.tar.gz`) {
      response.writeHead(200).end(archive);
    } else if (path === "/o/r/releases/download/v9.9.9/SHA256SUMS.txt") {
      response.writeHead(200).end(sums);
    } else if (path.startsWith("/o/r/releases/download/v7.7.7/")) {
      response.writeHead(503).end("Service Unavailable");
    } else {
      response.writeHead(404).end("Not Found");
    }
  });
  await new Promise((done) => server.listen(0, "127.0.0.1", done));
  base = `http://127.0.0.1:${server.address().port}`;
});

after(() => {
  server.close();
  rmSync(dir, { recursive: true, force: true });
});

function install(version) {
  const out = join(dir, `bin-${version}-${Math.random().toString(36).slice(2)}`);
  return new Promise((done) => {
    execFile(
      "bash",
      [INSTALL, version, out],
      {
        env: {
          ...process.env,
          RUNNER_OS: "Linux",
          RUNNER_ARCH: "X64",
          GITHUB_SERVER_URL: base,
          REPODNA_REPOSITORY: "o/r",
          REPODNA_RETRY_DELAY: "1",
        },
      },
      (error, stdout, stderr) => done({ code: error ? error.code : 0, stdout, stderr }),
    );
  });
}

test("installs a release, checked against its checksums", async () => {
  const result = await install("v9.9.9");
  assert.equal(result.code, 0, result.stderr);
  assert.equal(result.stdout, "repodna 9.9.9\n");
});

test("finds the latest release", async () => {
  const result = await install("latest");
  assert.equal(result.code, 0, result.stderr);
  assert.equal(result.stdout, "repodna 9.9.9\n");
});

test("waits out GitHub's passing errors", async () => {
  failures.set(`/o/r/releases/download/v9.9.9/repodna-9.9.9-${TARGET}.tar.gz`, 2);
  failures.set("/o/r/releases/latest", 1);
  const result = await install("latest");
  assert.equal(result.code, 0, result.stderr);
  assert.equal(result.stdout, "repodna 9.9.9\n");
});

test("says when a release has no such file", async () => {
  const result = await install("8.8.8");
  assert.equal(result.code, 1);
  assert.match(
    result.stderr,
    /RepoDNA 8\.8\.8 has no repodna-8\.8\.8-x86_64-unknown-linux-musl\.tar\.gz/,
  );
});

test("says that a download failed, not that the file is missing", async () => {
  const result = await install("7.7.7");
  assert.equal(result.code, 1);
  assert.match(
    result.stderr,
    /Downloading repodna-7\.7\.7-.*\.tar\.gz of RepoDNA 7\.7\.7 failed \(HTTP 503\)/,
  );
  assert.doesNotMatch(result.stderr, /has no/);
});

test("refuses what is not a version", async () => {
  const result = await install("main");
  assert.equal(result.code, 1);
  assert.match(result.stderr, /main is not a RepoDNA version/);
});
