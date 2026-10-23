import assert from "node:assert/strict";
import test from "node:test";
import { isReleaseVersion } from "../build.mjs";

test("accepts valid npm package versions", () => {
  for (const version of ["1.4.2", "0.0.1", "1.5.0-rc.1", "2.0.0-alpha.2"]) {
    assert.equal(isReleaseVersion(version), true, version);
  }
});

test("rejects malformed semver and leading-zero prerelease identifiers", () => {
  for (const version of ["01.2.3", "1.02.3", "1.2.3-", "1.2.3-..", "1.2.3-rc..1", "1.2.3-rc.01"]) {
    assert.equal(isReleaseVersion(version), false, version);
  }
});
