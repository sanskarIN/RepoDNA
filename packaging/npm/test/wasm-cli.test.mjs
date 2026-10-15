import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { linkSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

// Invalid invocations must fail before loading a WebAssembly program. The tests do not
// require a built engine, so they also run in the packaging job.
const CLI = resolve(dirname(fileURLToPath(import.meta.url)), "../wasm/bin/repodna-wasm.js");

function run(...arguments_) {
  return spawnSync(process.execPath, [CLI, ...arguments_], {
    encoding: "utf8",
    timeout: 10_000,
  });
}

test("a valued option must not consume the next option", () => {
  for (const args of [
    ["analyze", ".", "--profile", "--output", "analysis.repodna"],
    ["report", "analysis.repodna", "--format", "--output", "report.html"],
    ["card", "analysis.repodna", "--output", "--png"],
  ]) {
    const result = run(...args);
    assert.equal(result.status, 2, `${args.join(" ")}: ${result.stderr}`);
    assert.match(result.stderr, /needs a value/);
    assert.doesNotMatch(result.stderr, /Cannot find module/);
  }
});

test("the wasm command line accepts a help request without its engine", () => {
  const result = run("--help");
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /repodna-wasm analyze/);
});

test("commands reject flags that they cannot use", () => {
  for (const args of [
    ["analyze", ".", "--dark"],
    ["report", "missing.repodna", "--png"],
    ["card", "missing.repodna", "--privacy", "share"],
  ]) {
    const result = run(...args);
    assert.equal(result.status, 2, `${args.join(" ")}: ${result.stderr}`);
    assert.match(result.stderr, /cannot be used with/);
    assert.doesNotMatch(result.stderr, /Cannot find module/);
  }
});

test("report rejects unsupported values before loading the engine", () => {
  for (const [flag, value] of [
    ["--format", "xml"],
    ["--theme", "unknown"],
    ["--privacy", "everything"],
  ]) {
    const result = run("report", "missing.repodna", flag, value);
    assert.equal(result.status, 2, result.stderr);
    assert.match(result.stderr, new RegExp(`invalid ${flag} value`));
  }
});

test("output cannot replace an input file, even through a hard link", () => {
  const dir = mkdtempSync(join(tmpdir(), "repodna-wasm-cli-"));
  try {
    const artifact = join(dir, "analysis.repodna");
    const alias = join(dir, "same-file.repodna");
    writeFileSync(artifact, '{"important":"original"}');
    linkSync(artifact, alias);
    for (const command of ["report", "card"]) {
      for (const output of [artifact, alias]) {
        const result = run(command, artifact, "--output", output);
        assert.equal(result.status, 2, result.stderr);
        assert.match(result.stderr, /refusing to overwrite/);
        assert.equal(readFileSync(artifact, "utf8"), '{"important":"original"}');
      }
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
