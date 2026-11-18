import { describe, expect, it } from "vitest";
import { githubSourceHint } from "./github-source";

describe("GitHub browser URL preview", () => {
  it("recognizes the exact Sonumio branch without truncating slash names", () => {
    expect(
      githubSourceHint("https://github.com/sanskarIN/sonumio/tree/feat/initial-audio-platform"),
    ).toEqual({
      repository: "sanskarIN/sonumio",
      reference: "feat/initial-audio-platform",
      kind: "tree",
    });
  });

  it("decodes encoded slashes and ignores URL navigation suffixes", () => {
    expect(
      githubSourceHint("https://github.com/org/repo/tree/feature%2Flogin/?tab=readme"),
    ).toEqual({
      repository: "org/repo",
      reference: "feature/login",
      kind: "tree",
    });
  });

  it("previews exact commit URLs without confusing them with a clone URL", () => {
    const sha = "0123456789abcdef0123456789abcdef01234567";
    expect(githubSourceHint(`https://github.com/org/repo/commit/${sha}?diff=split`)).toEqual({
      repository: "org/repo",
      reference: sha,
      kind: "commit",
    });
  });

  it("does not guess from other providers or incomplete paths", () => {
    expect(githubSourceHint("https://gitlab.com/org/repo/tree/main")).toBeNull();
    expect(githubSourceHint("https://github.com/org/repo/tree/")).toBeNull();
    expect(githubSourceHint("https://github.com/org/repo/tree/bad%GG")).toBeNull();
    expect(githubSourceHint("not a URL")).toBeNull();
  });

  it("rejects malformed Git refs rather than previewing an invalid checkout", () => {
    for (const ref of [
      "%2D%2Dupload-pack%3Dcmd",
      "a%2E%2Eb",
      "%40%7Bbad",
      "a%2F",
      "main//",
      ".hidden",
      "a.lock",
      "main%0Aevil",
      "bad%GG",
    ]) {
      expect(githubSourceHint("https://github.com/org/repo/tree/" + ref)).toBeNull();
    }
  });

  it("accepts exact full SHA-1 and SHA-256 commit IDs only", () => {
    for (const size of [40, 64]) {
      const sha = "a".repeat(size);
      expect(githubSourceHint("https://github.com/org/repo/commit/" + sha)).toEqual({
        repository: "org/repo",
        reference: sha,
        kind: "commit",
      });
    }
    for (const sha of ["main", "a".repeat(39), "z".repeat(40), "a".repeat(65)]) {
      expect(githubSourceHint("https://github.com/org/repo/commit/" + sha)).toBeNull();
    }
    expect(
      githubSourceHint("https://github.com/org/repo/commit/" + "a".repeat(40) + "/extra"),
    ).toBeNull();
  });
});
