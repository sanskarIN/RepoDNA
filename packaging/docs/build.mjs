#!/usr/bin/env node
// Builds the documentation site that the web version publishes at
// https://sanskarin.github.io/RepoDNA/docs/: every guide in docs/ and the project's own pages
// (changelog, roadmap, policies, and the examples, benchmarks, and fixtures pages), with a
// sidebar that follows docs/index.md, search, and links that work as they do on GitHub.
//
//   node packaging/docs/build.mjs --out apps/web/dist/docs [--check]
//
// Links between the pages lead to the pages; links to other files of the repository lead to
// them on GitHub. --check fails when a link leads to a file the repository does not have, or
// to a heading a page does not have.

import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, posix, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Marked, Renderer } from "marked";

const HERE = dirname(fileURLToPath(import.meta.url));
export const REPOSITORY = "https://github.com/sanskarIN/RepoDNA";
const RAW = "https://raw.githubusercontent.com/sanskarIN/RepoDNA";

/** The project's own pages, beside the guides in docs/, with the address each gets. */
const PROJECT_PAGES = [
  ["CHANGELOG.md", "changelog"],
  ["ROADMAP.md", "roadmap"],
  ["CONTRIBUTING.md", "contributing-guide"],
  ["SECURITY.md", "security-policy"],
  ["SUPPORT.md", "support"],
  ["CODE_OF_CONDUCT.md", "code-of-conduct"],
  ["PRIVACY.md", "privacy-policy"],
  ["TERMS.md", "terms"],
  ["GOVERNANCE.md", "governance"],
  ["MAINTAINERS.md", "maintainers"],
  ["examples/README.md", "examples"],
  ["benchmarks/README.md", "benchmarks"],
  ["fixtures/README.md", "fixtures"],
];

/** The pages of the site: `{ source, slug }`, with `source` relative to the repository. */
export function pages(root) {
  const guides = readDirMarkdown(join(root, "docs")).map((name) => ({
    source: `docs/${name}`,
    slug: name === "index.md" ? "" : name.replace(/\.md$/, ""),
  }));
  const project = PROJECT_PAGES.filter(([source]) => existsSync(join(root, source))).map(
    ([source, slug]) => ({ source, slug }),
  );
  return [...guides, ...project];
}

function readDirMarkdown(dir) {
  // Sorted, so that the site is the same wherever it is built.
  return readdirSync(dir)
    .filter((name) => name.endsWith(".md"))
    .sort();
}

/**
 * GitHub's anchor for a heading: lowercase, without punctuation or symbols, with spaces as
 * hyphens, and -1, -2, ... after a repeated one. `seen` is per page.
 */
export function slugify(text, seen = new Map()) {
  const base = text
    .toLowerCase()
    .replace(/[^\p{L}\p{M}\p{N}\p{Pc} -]/gu, "")
    .replace(/ /g, "-");
  let slug = base;
  if (seen.has(base)) {
    let count = seen.get(base);
    do {
      count += 1;
      slug = `${base}-${count}`;
    } while (seen.has(slug));
    seen.set(base, count);
  }
  if (!seen.has(slug)) {
    seen.set(slug, 0);
  }
  return slug;
}

const ENTITIES = { amp: "&", lt: "<", gt: ">", quot: '"', "#39": "'", apos: "'", nbsp: " " };

function decode(text) {
  return text.replace(/&(amp|lt|gt|quot|#39|apos|nbsp);/g, (_, name) => ENTITIES[name]);
}

export function escapeHtml(text) {
  return String(text)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/**
 * HTML without its tags: everything from a "<" to the next ">" is left out, and so is the rest
 * after a "<" that is never closed. What is left has no "<" at all, so no tag can remain.
 */
export function withoutTags(html) {
  let text = "";
  let at = 0;
  while (at < html.length) {
    const open = html.indexOf("<", at);
    if (open === -1) {
      return text + html.slice(at);
    }
    text += html.slice(at, open);
    const close = html.indexOf(">", open);
    if (close === -1) {
      return text;
    }
    at = close + 1;
  }
  return text;
}

/** The text of inline tokens, without markup: what a heading's anchor is made from. */
function plainText(tokens = []) {
  return tokens
    .map((token) => {
      if (token.type === "html") {
        return withoutTags(token.text);
      }
      if (token.tokens && token.type !== "codespan") {
        return plainText(token.tokens);
      }
      return decode(token.text ?? "");
    })
    .join("");
}

/**
 * Resolves a link of the page at `source` (relative to the repository): a page of the site,
 * a file on GitHub, or the address as it is. Returns `{ href, page?, anchor?, missing? }`.
 */
export function resolveLink(href, source, site, root, { image = false } = {}) {
  if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith("//")) {
    return { href };
  }
  if (href.startsWith("#")) {
    return { href, page: site.bySource.get(source), anchor: decodeURIComponent(href.slice(1)) };
  }
  const [pathPart, anchorPart] = splitAnchor(href);
  const target = posix.normalize(posix.join(posix.dirname(source), decodeURIComponent(pathPart)));
  const anchor = anchorPart === undefined ? undefined : decodeURIComponent(anchorPart);
  const page = site.bySource.get(target.replace(/\/$/, ""));
  if (page && !image) {
    return { href: null, page, anchor };
  }
  const missing = target.startsWith("../") || !existsSync(join(root, target));
  const isDirectory = !missing && statSync(join(root, target)).isDirectory();
  const base = image ? RAW + "/main" : `${REPOSITORY}/${isDirectory ? "tree" : "blob"}/main`;
  return {
    href: `${base}/${encodePath(target)}${anchor === undefined ? "" : `#${anchorPart}`}`,
    missing: missing ? target : undefined,
    file: target,
    anchor,
  };
}

function splitAnchor(href) {
  const index = href.indexOf("#");
  return index === -1 ? [href, undefined] : [href.slice(0, index), href.slice(index + 1)];
}

function encodePath(path) {
  return path.split("/").map(encodeURIComponent).join("/");
}

/** The address of `page` from the page `from`: both are site pages. */
function relativeUrl(from, page, anchor) {
  const up = from.slug === "" ? "" : "../";
  const path = page.slug === "" ? up || "./" : `${up}${page.slug}/`;
  return anchor === undefined ? path : `${path}#${encodeURIComponent(anchor).replace(/%2D/g, "-")}`;
}

/** The text of a block, without markup: paragraphs, lists, tables, and code alike. */
function blockText(token) {
  if (token.type === "code") {
    return token.text;
  }
  if (token.type === "table") {
    const cells = [...token.header, ...token.rows.flat()];
    return cells.map((cell) => plainText(cell.tokens)).join(" ");
  }
  if (token.type === "list") {
    return token.items.map((item) => item.tokens.map(blockText).join(" ")).join(" ");
  }
  if (token.tokens) {
    return plainText(token.tokens);
  }
  return decode(token.text ?? "");
}

/**
 * The sections of a page for search: the text under each heading down to the third level,
 * with the heading's anchor. The anchors follow the same order as the rendered page's.
 */
function sectionsOf(markdown, title) {
  const seen = new Map();
  const sections = [{ id: "", text: title, body: [] }];
  for (const token of new Marked({ gfm: true }).lexer(markdown)) {
    if (token.type === "heading") {
      const text = plainText(token.tokens);
      const id = slugify(text, seen);
      if (token.depth === 1) {
        sections[0].text = text;
      } else if (token.depth <= 3) {
        sections.push({ id, text, body: [] });
      }
      continue;
    }
    if (token.type !== "space" && token.type !== "hr") {
      sections.at(-1).body.push(blockText(token));
    }
  }
  return sections
    .map((section) => ({ ...section, body: section.body.join(" ").replace(/\s+/g, " ").trim() }))
    .filter((section) => section.id === "" || section.body || section.text);
}

/** The headings of a Markdown file, as GitHub's anchors, for links to it from other pages. */
function headingsOf(markdown) {
  const seen = new Map();
  const ids = new Set();
  for (const token of new Marked({ gfm: true }).lexer(markdown)) {
    if (token.type === "heading") {
      ids.add(slugify(plainText(token.tokens), seen));
    }
  }
  return ids;
}

/** Renders one page's Markdown to HTML, and records its title, headings, and sections. */
function render(page, site, root, problems) {
  const markdown = readFileSync(join(root, page.source), "utf8");
  const seen = new Map();
  const headings = [];
  let title = null;
  let description = null;
  const link = (href, opts) => {
    const resolved = resolveLink(href, page.source, site, root, opts);
    if (resolved.missing) {
      problems.push(
        `${page.source}: links to ${resolved.missing}, which the repository does not have`,
      );
    }
    if (resolved.page) {
      site.anchorChecks.push({ from: page, to: resolved.page, anchor: resolved.anchor });
      return relativeUrl(page, resolved.page, resolved.anchor);
    }
    if (
      resolved.file &&
      resolved.anchor !== undefined &&
      resolved.file.endsWith(".md") &&
      !resolved.missing
    ) {
      site.fileAnchorChecks.push({ from: page, file: resolved.file, anchor: resolved.anchor });
    }
    return resolved.href;
  };
  const renderer = {
    heading({ tokens, depth }) {
      const text = plainText(tokens);
      const id = slugify(text, seen);
      const inner = this.parser.parseInline(tokens);
      if (depth === 1 && title === null) {
        title = text;
        return `<h1 id="${id}">${inner}</h1>\n`;
      }
      headings.push({ id, depth, text });
      return `<h${depth} id="${id}">${inner}<a class="anchor" href="#${id}" aria-label="Link to ${escapeHtml(text)}">#</a></h${depth}>\n`;
    },
    paragraph({ tokens }) {
      const text = plainText(tokens);
      if (description === null && text.trim()) {
        description = text.replace(/\s+/g, " ").trim();
      }
      return `<p>${this.parser.parseInline(tokens)}</p>\n`;
    },
    link({ href, title: linkTitle, tokens }) {
      const text = this.parser.parseInline(tokens);
      const titleAttr = linkTitle ? ` title="${escapeHtml(linkTitle)}"` : "";
      const target = link(href);
      const external = /^https?:/.test(target) ? ' rel="noopener"' : "";
      return `<a href="${escapeHtml(target)}"${titleAttr}${external}>${text}</a>`;
    },
    image({ href, title: imageTitle, text }) {
      const titleAttr = imageTitle ? ` title="${escapeHtml(imageTitle)}"` : "";
      return `<img src="${escapeHtml(link(href, { image: true }))}" alt="${escapeHtml(text)}"${titleAttr} loading="lazy">`;
    },
    code({ text, lang }) {
      const language = (lang || "").split(/\s/)[0];
      const code = `<pre><code${language ? ` class="language-${escapeHtml(language)}"` : ""}>${escapeHtml(text)}</code></pre>`;
      if (language === "mermaid") {
        const view = `${REPOSITORY}/blob/main/${encodePath(page.source)}`;
        return `<figure class="diagram">${code}<figcaption>A diagram in Mermaid; <a href="${view}" rel="noopener">GitHub draws it</a>.</figcaption></figure>\n`;
      }
      return `<div class="code">${code}</div>\n`;
    },
    table(token) {
      return `<div class="table" tabindex="0">${Renderer.prototype.table.call(this, token)}</div>\n`;
    },
    html({ text }) {
      // Markdown permits single- and double-quoted HTML attributes. Always emit double
      // quotes with HTML escaping, so an apostrophe in a path stays data, not markup.
      return text.replace(
        /\\b(href|src)\\s*=\\s*(["'])(.*?)\\2/gi,
        (_, name, _quote, value) =>
          `${name}="${escapeHtml(link(decode(value), { image: name.toLowerCase() === "src" }))}"`,
      );
    },
  };
  const marked = new Marked({ gfm: true });
  marked.use({ renderer });
  const html = marked.parse(markdown);
  return {
    html,
    title: title ?? page.slug,
    description: description ?? "",
    headings,
    sections: sectionsOf(markdown, title ?? page.slug),
  };
}

/** The sidebar: the sections of docs/index.md and the pages each links to, in order. */
function navigation(site, root) {
  const index = site.bySource.get("docs/index.md");
  const markdown = readFileSync(join(root, index.source), "utf8");
  const groups = [{ title: "Documentation", items: [{ page: index, label: "Overview" }] }];
  const listed = new Set([index]);
  for (const token of new Marked({ gfm: true }).lexer(markdown)) {
    if (token.type === "heading" && token.depth === 2) {
      groups.push({ title: plainText(token.tokens), items: [] });
    }
    if (token.type !== "list") {
      continue;
    }
    for (const item of token.items) {
      for (const linkToken of linksIn(item.tokens)) {
        const resolved = resolveLink(linkToken.href, index.source, site, root);
        if (!resolved.page || listed.has(resolved.page)) {
          continue;
        }
        listed.add(resolved.page);
        const label =
          resolved.anchor === undefined ? plainText(linkToken.tokens) : resolved.page.title;
        groups.at(-1).items.push({ page: resolved.page, label: capitalize(label) });
      }
    }
  }
  const rest = site.pages.filter((page) => !listed.has(page));
  if (rest.length > 0) {
    groups.push({ title: "More", items: rest.map((page) => ({ page, label: page.title })) });
  }
  return groups.filter((group) => group.items.length > 0);
}

function linksIn(tokens = []) {
  return tokens.flatMap((token) =>
    token.type === "link" ? [token] : token.tokens ? linksIn(token.tokens) : [],
  );
}

function capitalize(text) {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

function layout(page, site, groups, order) {
  const root = page.slug === "" ? "" : "../";
  const position = order.indexOf(page);
  const previous = order[position - 1];
  const next = order[position + 1];
  const nav = groups
    .map(
      (group) =>
        `<section><h2>${escapeHtml(group.title)}</h2><ul>${group.items
          .map(
            (item) =>
              `<li><a href="${relativeUrl(page, item.page)}"${item.page === page ? ' aria-current="page"' : ""}>${escapeHtml(item.label)}</a></li>`,
          )
          .join("")}</ul></section>`,
    )
    .join("");
  const onThisPage = page.headings.filter((heading) => heading.depth === 2);
  const toc =
    onThisPage.length >= 3
      ? `<nav class="toc" aria-label="On this page"><h2>On this page</h2><ul>${onThisPage
          .map((heading) => `<li><a href="#${heading.id}">${escapeHtml(heading.text)}</a></li>`)
          .join("")}</ul></nav>`
      : "";
  const pager = [
    previous
      ? `<a class="previous" href="${relativeUrl(page, previous)}"><span>Previous</span>${escapeHtml(previous.label)}</a>`
      : "<span></span>",
    next
      ? `<a class="next" href="${relativeUrl(page, next)}"><span>Next</span>${escapeHtml(next.label)}</a>`
      : "<span></span>",
  ].join("");
  const title =
    page.slug === "" ? "RepoDNA documentation" : `${page.title} · RepoDNA documentation`;
  return `<!doctype html>
<html lang="en" data-root="${root || "./"}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src 'self' https://raw.githubusercontent.com data:; style-src 'self'; script-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'">
<meta name="referrer" content="no-referrer">
<title>${escapeHtml(title)}</title>
<meta name="description" content="${escapeHtml(page.description.slice(0, 200))}">
<meta name="theme-color" content="#f9f9f7" media="(prefers-color-scheme: light)">
<meta name="theme-color" content="#0d0d0d" media="(prefers-color-scheme: dark)">
<link rel="icon" href="${root}assets/icon.svg" type="image/svg+xml">
<link rel="stylesheet" href="${root}assets/docs.css">
<script src="${root}assets/theme.js"></script>
<script src="${root}assets/docs.js" defer></script>
</head>
<body>
<a class="skip" href="#content">Skip to content</a>
<header class="top">
<button class="menu" type="button" aria-expanded="false" aria-controls="sidebar">Menu</button>
<a class="brand" href="${root || "./"}"><img src="${root}assets/icon.svg" alt="" width="26" height="26"><span>RepoDNA</span> <span class="section">Documentation</span></a>
<div class="search" role="search">
<input type="search" id="search" placeholder="Search the documentation" aria-label="Search the documentation" autocomplete="off" aria-controls="results" aria-expanded="false">
<ul id="results" hidden></ul>
</div>
<nav class="links" aria-label="RepoDNA">
<a href="${root}../">Web version</a>
<a href="${REPOSITORY}" rel="noopener">GitHub</a>
<button class="theme" type="button" aria-label="Theme: system">System</button>
</nav>
</header>
<div class="layout">
<nav class="sidebar" id="sidebar" aria-label="Documentation">${nav}</nav>
<main id="content">
<article>
${page.html}
</article>
<nav class="pager" aria-label="Previous and next page">${pager}</nav>
<p class="edit"><a href="${REPOSITORY}/edit/main/${encodePath(page.source)}" rel="noopener">Edit this page on GitHub</a></p>
<footer>
<p>RepoDNA is made by <a href="https://sanskarin.github.io" rel="noopener">Sanskar</a> and licensed under the <a href="${REPOSITORY}/blob/main/LICENSE" rel="noopener">Apache License 2.0</a>. Support its development on <a href="https://buymeacoffee.com/SanskarCodes" rel="noopener">Buy Me a Coffee</a> or <a href="https://razorpay.me/@SanskarCodes" rel="noopener">Razorpay</a>.</p>
</footer>
</main>
${toc}
</div>
</body>
</html>
`;
}

/** Builds the site into `out`, and returns the problems found in its links. */
export function build({ root, out }) {
  const site = { pages: pages(root), bySource: new Map(), anchorChecks: [], fileAnchorChecks: [] };
  for (const page of site.pages) {
    site.bySource.set(page.source, page);
  }
  const problems = [];
  for (const page of site.pages) {
    Object.assign(page, render(page, site, root, problems));
    page.label = page.title;
  }
  const groups = navigation(site, root);
  const order = [];
  for (const group of groups) {
    for (const item of group.items) {
      item.page.label = item.label;
      order.push(item.page);
    }
  }
  for (const { from, to, anchor } of site.anchorChecks) {
    if (
      anchor !== undefined &&
      anchor !== "" &&
      !to.headings.some((heading) => heading.id === anchor) &&
      anchor !== slugify(to.title)
    ) {
      problems.push(
        `${from.source}: links to #${anchor} in ${to.source}, which has no such heading`,
      );
    }
  }
  const fileHeadings = new Map();
  for (const { from, file, anchor } of site.fileAnchorChecks) {
    if (!fileHeadings.has(file)) {
      fileHeadings.set(file, headingsOf(readFileSync(join(root, file), "utf8")));
    }
    if (anchor && !fileHeadings.get(file).has(anchor)) {
      problems.push(`${from.source}: links to #${anchor} in ${file}, which has no such heading`);
    }
  }

  rmSync(out, { recursive: true, force: true });
  mkdirSync(join(out, "assets"), { recursive: true });
  for (const page of site.pages) {
    const dir = page.slug === "" ? out : join(out, page.slug);
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "index.html"), layout(page, site, groups, order));
  }
  for (const name of ["docs.css", "docs.js", "theme.js"]) {
    copyFileSync(join(HERE, "assets", name), join(out, "assets", name));
  }
  copyFileSync(join(root, "apps/web/public/favicon.svg"), join(out, "assets/icon.svg"));
  const index = order.map((page) => ({
    t: page.label,
    u: page.slug === "" ? "" : `${page.slug}/`,
    d: page.description.slice(0, 160),
    s: page.sections.map((section) => ({
      i: section.id,
      t: section.text,
      b: section.body.slice(0, 2000),
    })),
  }));
  writeFileSync(join(out, "search-index.json"), JSON.stringify(index));
  return { pages: site.pages.length, problems };
}

function parseArgs(args) {
  const options = { root: resolve(HERE, "../.."), out: null, check: false };
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--out") {
      options.out = resolve(args[++index] ?? "");
    } else if (arg === "--root") {
      options.root = resolve(args[++index] ?? "");
    } else if (arg === "--check") {
      options.check = true;
    } else {
      throw new Error(`Unknown option ${arg}. Usage: build.mjs --out DIR [--root DIR] [--check]`);
    }
  }
  if (!options.out) {
    throw new Error("Give the folder to build into with --out.");
  }
  return options;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const options = parseArgs(process.argv.slice(2));
    const { pages: count, problems } = build(options);
    for (const problem of problems) {
      process.stderr.write(`${problem}\n`);
    }
    process.stdout.write(
      `Built ${count} pages into ${options.out}${problems.length ? `, with ${problems.length} broken links` : ""}.\n`,
    );
    if (options.check && problems.length > 0) {
      process.exit(1);
    }
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exit(2);
  }
}
