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

/**
 * A GitHub tree URL may contain a valid advertised ref plus a directory.
 * Directory names can begin with a dot or contain spaces (percent-encoded in
 * the URL), but traversal and unsafe controls are never previewed.
 */
function validGitTreeCandidate(reference: string): boolean {
  if (validGitRef(reference)) return true;
  const [first, ...components] = reference.split("/");
  return (
    validGitRef(first) &&
    components.length > 0 &&
    components.every(
      (part) =>
        part.length > 0 &&
        part !== "." &&
        part !== ".." &&
        !/[\x00-\x1f\x7f-\x9f\\~^:?*\[]/.test(part),
    )
  );
}

function validCommitSha(reference: string): boolean {
  return /^(?:[a-fA-F0-9]{40}|[a-fA-F0-9]{64})$/.test(reference);
}

export function githubSourceHint(input: string): GitHubSourceHint | null {
  try {
    const source = input.trim();
    // URL() normalizes /./ and /../ before we can validate the tree path.
    // Reject those raw segments, including their encoded spellings, first.
    const rawPath = source.split(/[?#]/)[0];
    if (
      /\s/.test(source) ||
      /(?:^|\/)(?:\.|%2e){1,2}(?:\/|$)/i.test(rawPath)
    ) return null;
    const url = new URL(source);
    if (
      url.protocol !== "https:" ||
      !["github.com", "www.github.com"].includes(url.hostname.toLowerCase())
    ) {
      return null;
    }
    const [, owner, repository, route, ...refParts] = url.pathname.split("/");
    if (!owner || !repository || (route !== "tree" && route !== "commit")) return null;
    const raw = refParts.join("/").replace(/\/$/, "");
    if (!raw || (route === "commit" && refParts.length !== 1)) return null;
    const reference = decodeURIComponent(raw);
    if (route === "tree" ? !validGitTreeCandidate(reference) : !validCommitSha(reference)) {
      return null;
    }
    return { repository: owner + "/" + repository, reference, kind: route };
  } catch {
    return null;
  }
}
