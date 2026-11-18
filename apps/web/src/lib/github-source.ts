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

/**
 * Mirror the native Git ref-name rules for a local preview, without running Git.
 * This is only syntax validation; the native backend verifies existence/checkout.
 */
function validGitRef(reference: string): boolean {
  return (
    reference.length > 0 &&
    reference !== "@" &&
    !reference.startsWith("-") &&
    !reference.endsWith("/") &&
    !reference.endsWith(".") &&
    !reference.includes("..") &&
    !reference.includes("@{") &&
    reference
      .split("/")
      .every((part) => part.length > 0 && !part.startsWith(".") && !part.endsWith(".lock")) &&
    !/[\x00-\x1f\x7f-\x9f\s]/.test(reference) &&
    !/[\\~^:?*\[]/.test(reference)
  );
}

function validCommitSha(reference: string): boolean {
  return /^(?:[a-fA-F0-9]{40}|[a-fA-F0-9]{64})$/.test(reference);
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
    const [, owner, repository, route, ...refParts] = url.pathname.split("/");
    if (!owner || !repository || (route !== "tree" && route !== "commit")) return null;
    const raw = refParts.join("/").replace(/\/$/, "");
    if (!raw || (route === "commit" && refParts.length !== 1)) return null;
    const reference = decodeURIComponent(raw);
    if (route === "tree" ? !validGitRef(reference) : !validCommitSha(reference)) return null;
    return { repository: owner + "/" + repository, reference, kind: route };
  } catch {
    return null;
  }
}
