import { describe, expect, it, vi } from "vitest";
import actualChangelog from "../../../../CHANGELOG.md?raw";
import workspacePackage from "../../../../package.json";
import { markWhatsNewSeen, needsWhatsNew, releaseNotes } from "./whats-new";

const changelog = `# Changelog

## [Unreleased]

- A future feature users should not see yet.

## [1.4.3] - 2026-10-23

### Added
- **Smaller desktop windows** that remain resizable.
- [Explore the new filters](https://example.com) without uploads.

### Fixed
- Better scan recovery.

## [1.4.2] - 2026-10-16

- An earlier fix.
`;

describe("What's New release content", () => {
  it("reads exactly the installed release, not Unreleased or an older release", () => {
    expect(releaseNotes(changelog, "1.4.3")).toEqual({
      version: "1.4.3",
      date: "2026-10-23",
      highlights: [
        "Smaller desktop windows that remain resizable.",
        "Explore the new filters without uploads.",
        "Better scan recovery.",
      ],
    });
  });

  it("supports old releases, absent sections and a maximum number of highlights", () => {
    expect(releaseNotes(changelog, "1.4.2").highlights).toEqual(["An earlier fix."]);
    expect(releaseNotes(changelog, "2.0.0").highlights).toEqual([]);
    expect(releaseNotes(changelog, "1.4.3", 1).highlights).toHaveLength(1);
    expect(releaseNotes(changelog, "1.4.3", 0).highlights).toEqual([]);
  });
});

describe("Bundled release changelog", () => {
  it("contains dated offline highlights matching the packaged version", () => {
    const notes = releaseNotes(actualChangelog, workspacePackage.version);
    expect(notes.version).toBe(workspacePackage.version);
    expect(notes.date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(notes.highlights.length).toBeGreaterThanOrEqual(3);
    expect(notes.highlights.length).toBeLessThanOrEqual(6);
  });
});

describe("What's New version gate", () => {
  it("opens on a first install and on an update, but not repeatedly for the same version", () => {
    const data = new Map<string, string>();
    const storage = {
      getItem: vi.fn((key: string) => data.get(key) ?? null),
      setItem: vi.fn((key: string, value: string) => {
        data.set(key, value);
      }),
    };
    expect(needsWhatsNew("1.4.2", storage)).toBe(true);
    markWhatsNewSeen("1.4.2", storage);
    expect(needsWhatsNew("1.4.2", storage)).toBe(false);
    expect(needsWhatsNew("1.4.3", storage)).toBe(true);
    markWhatsNewSeen("1.4.3", storage);
    expect(needsWhatsNew("1.4.3", storage)).toBe(false);
    expect(storage.setItem).toHaveBeenCalledTimes(2);
  });

  it("continues to work for a session when persistent storage is denied", () => {
    const denied = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    };
    expect(needsWhatsNew("8.9.0", denied)).toBe(true);
    expect(() => markWhatsNewSeen("8.9.0", denied)).not.toThrow();
    expect(needsWhatsNew("8.9.0", denied)).toBe(false);
    expect(needsWhatsNew("8.9.1", denied)).toBe(true);
  });
});
