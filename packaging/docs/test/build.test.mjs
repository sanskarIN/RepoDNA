import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { build, slugify, withoutTags } from "../build.mjs";

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");

function repository(files) {
  const root = mkdtempSync(join(tmpdir(), "repodna-docs-"));
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    writeFileSync(join(root, path), text);
  }
  mkdirSync(join(root, "apps/web/public"), { recursive: true });
  writeFileSync(join(root, "apps/web/public/favicon.svg"), "<svg/>");
  return root;
}

test("anchors are GitHub's", () => {
  assert.equal(slugify("Risky patterns (security.<rule>)"), "risky-patterns-securityrule");
  assert.equal(slugify("Tests, build & docs"), "tests-build--docs");
  assert.equal(slugify("`repodna ci`"), "repodna-ci");
  assert.equal(slugify("Windows on Arm, x86_64"), "windows-on-arm-x86_64");
  const seen = new Map();
  assert.deepEqual(
    ["Example", "Example", "Example"].map((text) => slugify(text, seen)),
    ["example", "example-1", "example-2"],
  );
});

test("HTML in a heading is left out of its anchor, as on GitHub", () => {
  assert.equal(withoutTags('<a name="old">'), "");
  assert.equal(withoutTags("before<br>after"), "beforeafter");
  // Pieces of tags never make one: no "<" is left at all.
  assert.equal(withoutTags("<scr<script>ipt>alert(1)"), "ipt>alert(1)");
  assert.equal(withoutTags("text <script"), "text ");
  const root = repository({
    "docs/index.md":
      '# Docs\n\n## Install <sup>new</sup>\n\n## Use <a name="old"></a>it\n\n' +
      "See [install](#install-new) and [use](#use-it).\n",
  });
  const out = join(root, "site");
  try {
    const { problems } = build({ root, out });
    assert.deepEqual(problems, []);
    const index = readFileSync(join(out, "index.html"), "utf8");
    assert.match(index, /<h2 id="install-new">/);
    assert.match(index, /<h2 id="use-it">/);
    const search = readFileSync(join(out, "search-index.json"), "utf8");
    assert.match(search, /"Install new"/);
    assert.match(search, /"Use it"/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("links lead to the pages, to GitHub, or are reported", () => {
  const root = repository({
    "docs/index.md": "# Docs\n\n## Start\n\n- [Guide](guide.md)\n- [Missing](missing.md)\n",
    "docs/guide.md":
      "# Guide\n\n## Steps\n\nSee [the start](index.md#start), [nowhere](index.md#nowhere), " +
      "[the README](../README.md#usage), [code](../src/main.rs), [the web](https://example.com), " +
      "and [steps](#steps).\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n```mermaid\ngraph TD\n```\n",
    "README.md": "# Project\n\n## Usage\n",
    "src/main.rs": "fn main() {}\n",
    "CHANGELOG.md": "# Changelog\n\nSee [the guide](docs/guide.md#steps).\n",
  });
  const out = join(root, "site");
  try {
    const { pages, problems } = build({ root, out });
    assert.equal(pages, 3);
    assert.deepEqual(problems.sort(), [
      "docs/guide.md: links to #nowhere in docs/index.md, which has no such heading",
      "docs/index.md: links to docs/missing.md, which the repository does not have",
    ]);
    const guide = readFileSync(join(out, "guide/index.html"), "utf8");
    assert.match(guide, /href="\.\.\/#start"/);
    assert.match(
      guide,
      /href="https:\/\/github\.com\/sanskarIN\/RepoDNA\/blob\/main\/README\.md#usage"/,
    );
    assert.match(
      guide,
      /href="https:\/\/github\.com\/sanskarIN\/RepoDNA\/blob\/main\/src\/main\.rs"/,
    );
    assert.match(guide, /href="https:\/\/example\.com" rel="noopener"/);
    assert.match(guide, /href="#steps"/);
    assert.match(guide, /<h2 id="steps">/);
    assert.match(guide, /<div class="table" tabindex="0"><table>/);
    assert.match(guide, /<figure class="diagram">/);
    assert.match(guide, /aria-current="page">Guide</);
    const changelog = readFileSync(join(out, "changelog/index.html"), "utf8");
    assert.match(changelog, /href="\.\.\/guide\/#steps"/);
    const index = JSON.parse(readFileSync(join(out, "search-index.json"), "utf8"));
    assert.deepEqual(
      index.map((page) => page.u),
      ["", "guide/", "changelog/"],
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("RepoDNA's own documentation has no broken links", () => {
  const out = mkdtempSync(join(tmpdir(), "repodna-docs-site-"));
  try {
    const { pages, problems } = build({ root: REPO, out });
    assert.deepEqual(problems, []);
    assert.ok(pages >= 30, `only ${pages} pages`);
    const cli = readFileSync(join(out, "cli/index.html"), "utf8");
    assert.match(cli, /<title>Command-line reference · RepoDNA documentation<\/title>/);
    // A guide's link to another guide leads to its page, never to a .md file.
    assert.doesNotMatch(cli, /href="(?!https?:)[^"]*\.md(#[^"]*)?"/);
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
});
