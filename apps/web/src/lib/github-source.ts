/**
 * Local-only preview of a GitHub browser URL.
 * It does not make a network request or claim that the ref exists. A /tree/
 * route is ambiguous between a branch/tag name and a subdirectory path.
 */
export interface GitHubSourceHint {
  repository: string;
  reference: string;
  kind: "tree" | "commit";
}

export function githubSourceHint(input: string): GitHubSourceHint | null {
  try {
    const url = new URL(input.trim());
    if (
      !["https:", "http:"].includes(url.protocol) ||
      !["github.com", "www.github.com"].includes(url.hostname.toLowerCase())
    ) {
      return null;
    }
    const segments = url.pathname.split("/");
    const [, owner, repository, route, ...refParts] = segments;
    if (!owner || !repository || !route || !["tree", "commit"].includes(route)) return null;
    const raw = refParts.join("/").replace(/\/$/, "");
    if (!raw || (route === "commit" && refParts.length !== 1)) return null;
    const reference = decodeURIComponent(raw);
    if (!reference || /[\x00-\x1f\x7f]/.test(reference)) return null;
    return { repository: `${owner}/${repository}`, reference, kind: route as "tree" | "commit" };
  } catch {
    return null;
  }
}
