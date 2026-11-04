import assert from "node:assert/strict";
import { test } from "node:test";
import {
  BMC_FUNDING_HANDLE,
  BMC_URL,
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

test("the official BMC destination and Sponsors handle are accepted", () => {
  const files = [
    { path: "README.md", text: BMC_URL + "\n" + RAZORPAY_URL },
    {
      path: ".github/FUNDING.yml",
      text: "buy_me_a_coffee: " + BMC_FUNDING_HANDLE + "\ncustom:\n  - " + RAZORPAY_URL,
    },
  ];
  assert.deepEqual(
    auditSupportLinks(files, { requiredFiles: ["README.md", ".github/FUNDING.yml"] }),
    [],
  );
});

test("old BMC accounts, insecure destinations, URL variations and tracking fail", () => {
  const domain = "buymeacoffee" + ".com";
  const alternatives = [
    "https://" + domain + "/SanskarCodes",
    "http://" + domain + "/sanskarIN",
    "https://www." + domain + "/sanskarIN",
    "https://" + domain + "/sanskarIN?ref=other",
    "https://bmc" + ".link/sanskarIN",
    domain + "/sanskarIN",
  ];
  for (const alternative of alternatives) {
    const issues = auditSupportLinks(
      [{ path: "README.md", text: "Support: " + alternative }],
      { requiredFiles: [] },
    );
    assert.equal(issues.length, 1, alternative);
    assert.match(issues[0], /^README\.md:1: replace /);
    assert.ok(issues[0].endsWith(BMC_URL));
  }
});

test("GitHub Sponsors must use the canonical BMC account", () => {
  const issues = auditSupportLinks(
    [{
      path: ".github/FUNDING.yml",
      text: "buy_me_a_coffee: SanskarCodes\ncustom:\n  - " + RAZORPAY_URL,
    }],
    { requiredFiles: [".github/FUNDING.yml"] },
  );
  assert.deepEqual(issues, [
    ".github/FUNDING.yml: expected buy_me_a_coffee: sanskarIN",
  ]);
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
    "README.md: missing the official Buy Me a Coffee support URL",
    "SUPPORT.md: missing the official Razorpay support URL",
    "SUPPORT.md: missing the official Buy Me a Coffee support URL",
  ]);
  assert.ok(REQUIRED_LINK_FILES.includes(".github/FUNDING.yml"));
});

test("tracked support links use the official Razorpay and BMC destinations", () => {
  assert.deepEqual(checkRepository(), []);
});
