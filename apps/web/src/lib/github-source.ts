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
export function validGitRef(reference: string): boolean {
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
  const segments = reference.split("/");
  const first = segments[0] ?? "";
  const components = segments.slice(1);
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

/** Match native validation without echoing possibly credential-bearing pasted URLs. */
export function repositoryInputError(input: string): string | null {
  const path = input.trim().split(/[?#]/)[0] ?? "";
  const firstScheme = path.indexOf("://");
  if (firstScheme >= 0 && path.indexOf("://", firstScheme + 3) >= 0) {
    return "Multiple repository URLs detected. Clear the field and paste one repository or branch URL.";
  }
  return null;
}

export function githubSourceHint(input: string): GitHubSourceHint | null {
  try {
    const source = input.trim();
    if (repositoryInputError(source)) return null;
    // URL() normalizes /./ and /../ before we can validate the tree path.
    // Reject those raw segments, including their encoded spellings, first.
    const rawPath = source.split(/[?#]/)[0] ?? "";
    if (/\s/.test(source) || /(?:^|\/)(?:\.|%2e){1,2}(?:\/|$)/i.test(rawPath)) {
      return null;
    }
    // URL() decodes percent-encoded hostnames before hostname is exposed.
    // Match the native Git URL policy instead of previewing an input it rejects.
    const authority = /^https:\/\/([^/?#]+)/i.exec(source)?.[1];
    if (authority?.slice(authority.lastIndexOf("@") + 1).includes("%")) return null;
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

/** Conservative UI hint; native Git remains authoritative for URL policy. */
export function looksLikeRemoteSource(input: string): boolean {
  const value = input.trim();
  return (
    /^(?:https?|ssh|git\+ssh|ssh\+git|git):\/\//i.test(value) ||
    /^[^\s/@:]+@[^\s/:]+\.[^\s/:]+:[^\s]+$/.test(value)
  );
}
