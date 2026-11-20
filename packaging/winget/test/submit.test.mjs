import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, dirname, join, resolve } from "node:path";
import test from "node:test";

const submit = resolve("packaging/winget/submit.sh");

// All GitHub calls go to a local mock. Tests never create real forks or PRs.
function fixture(t, options = {}) {
  const dir = mkdtempSync(join(tmpdir(), "repodna-winget-submit-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const root = join(dir, "manifests");
  const bin = join(dir, "bin");
  const log = join(dir, "gh-calls.jsonl");
  const payloads = join(dir, "gh-payloads.jsonl");
  mkdirSync(bin, { recursive: true });
  writeFileSync(log, "");
  writeFileSync(payloads, "");

  for (const id of ["Sanskar.RepoDNA", "Sanskar.RepoDNA.CLI"]) {
    const folder = join(root, "s", ...id.split("."), "1.5.2");
    mkdirSync(folder, { recursive: true });
    for (const suffix of [".installer.yaml", ".locale.en-US.yaml", ".yaml"]) {
      writeFileSync(join(folder, id + suffix), "PackageIdentifier: " + id + "\nPackageVersion: 1.5.2\n");
    }
  }

  const fake = [
    "#!/usr/bin/env node",
    'const fs = require("node:fs");',
    "const argv = process.argv.slice(2);",
    'fs.appendFileSync(process.env.TEST_GH_LOG, JSON.stringify(argv) + "\\n");',
    'const endpoint = argv.find(a => a.startsWith("repos/")) || "";',
    'const qi = argv.indexOf("--jq");',
    'const jq = qi < 0 ? "" : argv[qi + 1];',
    'const ii = argv.indexOf("--input");',
    'if (ii >= 0) fs.appendFileSync(process.env.TEST_GH_PAYLOADS, JSON.stringify({ endpoint, body: JSON.parse(fs.readFileSync(argv[ii + 1], "utf8")) }) + "\\n");',
    'if (endpoint.includes("merge-upstream")) { console.error("unexpected upstream sync"); process.exit(89); }',
    'if (argv[1] === "user") { console.log(process.env.TEST_GH_LOGIN || "sanskarIN"); process.exit(0); }',
    'if (endpoint === "repos/sanskarIN/winget-pkgs") { console.log(jq.includes("parent.full_name") ? "microsoft/winget-pkgs" : "{}"); process.exit(0); }',
    'if (endpoint.includes("/git/ref/heads/master")) { console.log(jq === ".object.sha" ? "existing-fork-master" : "{}"); process.exit(0); }',
    'if (endpoint.includes("/git/ref/heads/repodna-")) { if (process.env.TEST_EXISTING_BRANCH === "1") { console.log("{}"); process.exit(0); } process.exit(1); }',
    'if (endpoint.includes("/git/commits/")) { console.log("existing-fork-tree"); process.exit(0); }',
    'if (endpoint.includes("/git/trees")) { console.log("new-tree"); process.exit(0); }',
    'if (endpoint.includes("/git/commits")) { console.log("new-commit"); process.exit(0); }',
    'if (endpoint.includes("/git/refs")) process.exit(0);',
    'if (endpoint.includes("/pulls?")) { console.log(process.env.TEST_EXISTING_PR === "1" ? "https://github.com/microsoft/winget-pkgs/pull/existing" : ""); process.exit(0); }',
    'if (endpoint === "repos/microsoft/winget-pkgs/pulls") { console.log("https://github.com/microsoft/winget-pkgs/pull/mock"); process.exit(0); }',
    'console.error("Unrecognized mock GitHub call:", argv.join(" ")); process.exit(1);',
  ].join("\n");
  const gh = join(bin, "gh");
  writeFileSync(gh, fake + "\n");
  chmodSync(gh, 0o755);

  const env = {
    ...process.env,
    PATH: bin + delimiter + process.env.PATH,
    GH_TOKEN: "fake-public-repo-token",
    TEST_GH_LOG: log,
    TEST_GH_PAYLOADS: payloads,
    TEST_EXISTING_BRANCH: options.existing ? "1" : "",
    TEST_EXISTING_PR: options.existing ? "1" : "",
    TEST_GH_LOGIN: options.login || "sanskarIN",
  };
  return {
    run: () => spawnSync("bash", [submit, root, "v1.5.2"], { env, encoding: "utf8" }),
    calls: () => readFileSync(log, "utf8").trim().split("\n").filter(Boolean).map(JSON.parse),
    payloads: () => readFileSync(payloads, "utf8").trim().split("\n").filter(Boolean).map(JSON.parse),
  };
}

test("submits two independent manifest-only PRs without synchronizing workflow files", (t) => {
  const f = fixture(t);
  const result = f.run();
  assert.equal(result.status, 0, result.stderr + "\n" + result.stdout);
  assert.match(result.stdout, /Sanskar\.RepoDNA PR created/);
  assert.match(result.stdout, /Sanskar\.RepoDNA\.CLI PR created/);
  const calls = f.calls();
  assert.ok(calls.every(args => !args.join(" ").includes("merge-upstream")));
  const submissions = f.payloads();
  const trees = submissions.filter(p => p.endpoint.endsWith("/git/trees"));
  const prs = submissions.filter(p => p.endpoint === "repos/microsoft/winget-pkgs/pulls");
  assert.equal(trees.length, 2);
  assert.equal(prs.length, 2);
  for (const record of trees) {
    assert.equal(record.body.base_tree, "existing-fork-tree");
    assert.equal(record.body.tree.length, 3, "one package means exactly three manifest files");
    for (const entry of record.body.tree) {
      assert.match(entry.path, /^manifests\/s\/Sanskar\/RepoDNA\//);
      assert.match(entry.path, /\/1\.5\.2\/Sanskar\.RepoDNA/);
      assert.ok(entry.path.endsWith(".yaml"));
      assert.equal(entry.mode, "100644");
      assert.equal(entry.type, "blob");
      assert.ok(!entry.path.includes(".github/workflows"));
    }
  }
  const prHeads = prs.map(p => p.body.head).sort();
  assert.deepEqual(prHeads, [
    "sanskarIN:repodna-v1.5.2-cli",
    "sanskarIN:repodna-v1.5.2-desktop",
  ]);
  assert.ok(prs.every(p => p.body.base === "master"));
});

test("existing upstream PRs are reused without creating duplicate branches or commits", (t) => {
  const f = fixture(t, { existing: true });
  const result = f.run();
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Upstream pull request already exists/);
  assert.equal(f.payloads().length, 0);
  assert.equal(f.calls().filter(args => args.join(" ").includes("merge-upstream")).length, 0);
});

test("a token belonging to a different account is rejected before any write", (t) => {
  const f = fixture(t, { login: "another-owner" });
  const result = f.run();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Use a GitHub token for sanskarIN/);
  assert.equal(f.payloads().length, 0);
});
