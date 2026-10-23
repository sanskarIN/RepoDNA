/** Release highlights displayed offline using the changelog bundled with the app. */
export interface ReleaseNotes {
  version: string;
  date: string | null;
  highlights: string[];
}

const KEY = "repodna-whats-new-seen";
let sessionSeen: string | null = null;

/** Return only the chosen released version, never upcoming Unreleased changes. */
export function releaseNotes(changelog: string, version: string, maximum = 6): ReleaseNotes {
  const lines = changelog.split(/\r?\n/);
  const heading = `## [${version}]`;
  const start = lines.findIndex((line) => line === heading || line.startsWith(heading + " - "));
  if (start === -1) return { version, date: null, highlights: [] };

  const title = lines[start] ?? "";
  const date = title.startsWith(heading + " - ") ? title.slice(heading.length + 3).trim() : null;
  const highlights: string[] = [];
  for (const line of lines.slice(start + 1)) {
    if (line.startsWith("## [")) break;
    if (!line.startsWith("- ")) continue;
    // Render as text, not HTML: changelog Markdown is never an injection surface.
    const text = line
      .slice(2)
      .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
      .replace(/[\u0060*]/g, "")
      .trim();
    if (text) highlights.push(text);
    if (highlights.length === Math.max(0, maximum)) break;
  }
  return { version, date, highlights };
}

/** Per-browser version gate. Falls back to memory when browser storage is disabled. */
export function needsWhatsNew(version: string, storage?: Pick<Storage, "getItem">): boolean {
  if (!version.trim()) return false;
  try {
    return (storage ?? window.localStorage).getItem(KEY) !== version;
  } catch {
    return sessionSeen !== version;
  }
}

/** A dismissed announcement stays dismissed until the next version is installed. */
export function markWhatsNewSeen(version: string, storage?: Pick<Storage, "setItem">): void {
  sessionSeen = version;
  try {
    (storage ?? window.localStorage).setItem(KEY, version);
  } catch {
    // Disabled storage does not prevent dismissing the announcement in this session.
  }
}
