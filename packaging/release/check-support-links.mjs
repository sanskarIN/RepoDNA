#!/usr/bin/env node
// Protect the published support URL across tracked source, docs, and packaging files.
// The check is read-only and uses Node built-ins so it can run in CI without npm install.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const RAZORPAY_URL = "https://razorpay.me/@SanskarCodes";

export const REQUIRED_LINK_FILES = [
  ".github/FUNDING.yml",
  "README.md",
  "SUPPORT.md",
  "docs/faq.md",
  "apps/web/src/lib/links.ts",
  "packaging/docs/build.mjs",
];

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const TEXT_FILE = /\.(?:c|cc|cpp|cs|css|go|h|html|java|js|jsx|json|md|mdx|mjs|py|rs|sh|svg|toml|ts|tsx|txt|xml|ya?ml|ps1)$/i;
// Match provider destinations, including short links and links without a scheme.
// Only the exact canonical HTTPS URL is permitted as a Razorpay destination.
const RAZORPAY_LINK = /\b(?:https?:\/\/)?(?:www\.)?(?:razorpay\.me|rzp\.io)(?:\/[^\s<>"'\x60()[\]{}]+)?/gi;

/** Return actionable errors without reading or modifying the filesystem. */
export function auditSupportLinks(files, { requiredFiles = REQUIRED_LINK_FILES } = {}) {
  const issues = [];
  const contents = new Map(files.map(({ path, text }) => [path, text]));

  for (const { path, text } of files) {
    if (!TEXT_FILE.test(path)) continue;
    for (const match of text.matchAll(RAZORPAY_LINK)) {
      const found = match[0].replace(/[.,;!]+$/, "");
      if (found === RAZORPAY_URL) continue;
      const line = text.slice(0, match.index).split("\n").length;
      issues.push(path + ":" + line + ": replace " + found + " with " + RAZORPAY_URL);
    }
  }

  for (const path of requiredFiles) {
    if (!contents.get(path)?.includes(RAZORPAY_URL)) {
      issues.push(path + ": missing the official Razorpay support URL");
    }
  }
  return issues;
}

export function checkRepository(root = ROOT) {
  const tracked = execFileSync("git", ["ls-files", "-z"], {
    cwd: root,
    encoding: "utf8",
  }).split("\0").filter(Boolean);
  const files = tracked.filter((path) => TEXT_FILE.test(path)).map((path) => ({
    path,
    text: readFileSync(resolve(root, path), "utf8"),
  }));
  return auditSupportLinks(files);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const issues = checkRepository();
  if (issues.length > 0) {
    console.error("RepoDNA support link verification failed:\n" + issues.join("\n"));
    process.exitCode = 1;
  } else {
    console.log("All tracked Razorpay links use the official support URL.");
  }
}
