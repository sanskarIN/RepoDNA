import { describe, expect, it } from "vitest";
import { githubSourceHint, repositoryInputError } from "./github-source";

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

  it("previews hidden folders and encoded spaces after a GitHub revision", () => {
    for (const [url, reference] of [
      ["https://github.com/org/repo/tree/main/.github", "main/.github"],
      ["https://github.com/org/repo/tree/main/docs/design%20notes", "main/docs/design notes"],
      [
        "https://github.com/org/repo/tree/feat/new-ui/.github/workflows",
        "feat/new-ui/.github/workflows",
      ],
      ["https://github.com/org/repo/tree/main/docs/said%20%22hello%22", 'main/docs/said "hello"'],
      ["https://github.com/org/repo/tree/main/config.lock", "main/config.lock"],
    ] as const) {
      expect(githubSourceHint(url)).toEqual({
        repository: "org/repo",
        reference,
        kind: "tree",
      });
    }
  });

  it("rejects traversal and unencrypted links before a misleading preview", () => {
    for (const url of [
      "https://github.com/org/repo/tree/main/../secrets",
      "https://github.com/org/repo/tree/main/%2E%2E/secrets",
      "https://github.com/org/repo/tree/main/.%2Fsecret",
      "https://github.com/org/repo/tree/main/%00",
      "https://github.com/org/repo/tree/main/%5Cescaped",
      "https://github.com/org/repo/tree/main/%2A",
      "https://github.com/org/repo/tree/main/design notes",
      "http://github.com/org/repo/tree/main",
    ]) {
      expect(githubSourceHint(url)).toBeNull();
    }
  });

  it("previews exact commit URLs without confusing them with a clone URL", () => {
    const sha = "0123456789abcdef0123456789abcdef01234567";
    expect(githubSourceHint(`https://github.com/org/repo/commit/${sha}?diff=split`)).toEqual({
      repository: "org/repo",
      reference: sha,
      kind: "commit",
    });
  });

  it("rejects encoded GitHub hostnames before URL normalization", () => {
    for (const host of ["%67ithub.com", "github%2ecom", "%77ww.github.com"]) {
      expect(githubSourceHint(`https://${host}/org/repo/tree/main`)).toBeNull();
      expect(githubSourceHint(`https://${host}/org/repo/commit/${"a".repeat(40)}`)).toBeNull();
    }
    // Userinfo encoding is not a hostname and remains display-only metadata.
    expect(githubSourceHint("https://user%40domain@github.com/org/repo/tree/main")).toEqual({
      repository: "org/repo",
      reference: "main",
      kind: "tree",
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

describe("Repository input validation", () => {
  const branch = "https://github.com/sanskarIN/latebrake/tree/feat/experimental-six-car-runtime";

  it("blocks the exact concatenated LateBrake URL and preserves the valid branch", () => {
    for (const input of [branch + branch, branch + " " + branch]) {
      expect(repositoryInputError(input)).toContain("Multiple repository URLs");
      expect(githubSourceHint(input)).toBeNull();
    }
    expect(repositoryInputError(branch)).toBeNull();
    expect(githubSourceHint(branch)).toEqual({
      repository: "sanskarIN/latebrake",
      reference: "feat/experimental-six-car-runtime",
      kind: "tree",
    });
  });

  it("does not echo credentials from a second pasted URL", () => {
    const invalid = repositoryInputError(branch + "https://user:secret@github.com/org/repo");
    expect(invalid).toContain("Multiple repository URLs");
    expect(invalid).not.toContain("secret");
  });

  it("leaves local paths, normal remotes and URL query parameters alone", () => {
    for (const input of [
      "C:\\Users\\developer\\My project",
      "/home/user/my project",
      "git@github.com:org/repo.git",
      "https://github.com/org/repo.git",
      "https://github.com/org/repo?next=https://example.org/",
      "https://github.com/org/repo#https://example.org/",
    ]) {
      expect(repositoryInputError(input)).toBeNull();
    }
  });
});
