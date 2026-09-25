// Hash-based routing: works from `repodna serve`, the desktop app, and static hosting
// without server-side route configuration.

import { useEffect, useState } from "react";

export interface Route {
  path: string;
  params: URLSearchParams;
}

export function parseHash(hash: string): Route {
  const raw = hash.replace(/^#/, "") || "/";
  const [path = "/", query = ""] = raw.split("?", 2);
  return { path: path.startsWith("/") ? path : `/${path}`, params: new URLSearchParams(query) };
}

export function useRoute(): Route {
  const [route, setRoute] = useState(() => parseHash(window.location.hash));
  useEffect(() => {
    const update = () => setRoute(parseHash(window.location.hash));
    window.addEventListener("hashchange", update);
    return () => window.removeEventListener("hashchange", update);
  }, []);
  return route;
}

/** A link target for `path` with optional query parameters. */
export function href(path: string, params?: Record<string, string>): string {
  const query = params ? new URLSearchParams(params).toString() : "";
  return `#${path}${query ? `?${query}` : ""}`;
}

export function navigate(path: string, params?: Record<string, string>): void {
  window.location.hash = href(path, params).slice(1);
}
