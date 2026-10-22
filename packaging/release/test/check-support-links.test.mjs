import assert from "node:assert/strict";
import { test } from "node:test";
import {
  RAZORPAY_URL,
  REQUIRED_LINK_FILES,
  auditSupportLinks,
  checkRepository,
} from "../check-support-links.mjs";

test("the official link is accepted in source and documentation", () => {
  const files = [
    { path: "README.md", text: "Support: " + RAZORPAY_URL },
    { path: ".github/FUNDING.yml", text: "custom:\n  - " + RAZORPAY_URL },
  ];
  assert.deepEqual(auditSupportLinks(files, { requiredFiles: [] }), []);
});

test("incorrect destinations, short links, case changes and HTTP fail", () => {
  const alternatives = [
    "https://razorpay" + ".me/@OtherAccount",
    "http://razorpay" + ".me/@SanskarCodes",
    "https://www.razorpay" + ".me/@SanskarCodes",
    "https://RazorPay" + ".me/@SanskarCodes",
    "https://rzp" + ".io/l/old-link",
    "razorpay" + ".me/@SanskarCodes",
    RAZORPAY_URL + "?campaign=alternate",
  ];
  for (const alternative of alternatives) {
    const issues = auditSupportLinks(
      [{ path: "README.md", text: "Donate at " + alternative + "\n" }],
      { requiredFiles: [] },
    );
    assert.equal(issues.length, 1, alternative);
    assert.match(issues[0], /^README\.md:1: replace /);
  }
});

test("errors include file and line numbers for multiple mistakes", () => {
  const incorrect = "https://rzp" + ".io/l/previous";
  const problems = auditSupportLinks(
    [{ path: "docs/faq.md", text: "First line\nSecond line " + incorrect }],
    { requiredFiles: [] },
  );
  assert.equal(problems.length, 1);
  assert.match(problems[0], /^docs\/faq\.md:2:/);
});

test("required link locations cannot silently remove the official URL", () => {
  const problems = auditSupportLinks([{ path: "README.md", text: "No link" }], {
    requiredFiles: ["README.md", "SUPPORT.md"],
  });
  assert.deepEqual(problems, [
    "README.md: missing the official Razorpay support URL",
    "SUPPORT.md: missing the official Razorpay support URL",
  ]);
  assert.ok(REQUIRED_LINK_FILES.includes(".github/FUNDING.yml"));
});

test("the tracked repository uses only the official Razorpay destination", () => {
  assert.deepEqual(checkRepository(), []);
});
