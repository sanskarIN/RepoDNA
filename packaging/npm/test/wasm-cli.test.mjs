import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
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
