// Where analyses come from: the local server (`repodna serve`), the desktop app, or
// nowhere (static hosting: files opened in the browser and the bundled demo only).

import { checkArtifact, type RepositoryDna } from "@repodna/schema";

export interface RepositorySummary {
  id: string;
  name: string;
  location: string;
  kind: string;
  scans: number;
  firstScannedAt: string;
  lastScannedAt: string;
}

export interface ScanSummary {
  id: string;
  generatedAt: string;
  profile: string;
  revision?: string | null;
  dnaHash: string;
  toolVersion: string;
  files: number;
  codeLines: number;
  commits: number;
  contributors: number;
  findings: {
    critical: number;
    warning: number;
    attention: number;
    info: number;
    suppressed: number;
  };
}

export interface JobState {
  id: string;
  input: string;
  status: "running" | "completed" | "failed" | "cancelled";
  stage?: string | null;
  stages: [string, string][];
  filesDone: number;
  filesTotal: number;
  repositoryId?: string | null;
  scanId?: string | null;
  error?: string | null;
  warnings: string[];
  startedAt: string;
  finishedAt?: string | null;
}

export interface Session {
  version: string;
  allowScans: boolean;
}

export type ReportFormat = "html" | "markdown" | "json";
/** What the desktop app can save: a report, the full report folder, or the DNA card. */
export type SaveFormat = ReportFormat | "bundle" | "card";
export type ReportTheme = "professional" | "minimal" | "technical" | "dark";
export type PrivacyPreset = "local" | "share" | "public";

export interface ReportOptions {
  scan?: string;
  theme?: ReportTheme;
  privacy?: PrivacyPreset;
}

/** Operations every backend offers. */
export interface Backend {
  readonly kind: "server" | "desktop";
  session(): Promise<Session>;
  repositories(): Promise<RepositorySummary[]>;
  repository(id: string): Promise<{ repository: RepositorySummary; scans: ScanSummary[] }>;
  artifact(id: string, scan?: string): Promise<RepositoryDna>;
  startScan(input: string, profile?: string): Promise<JobState>;
  job(id: string): Promise<JobState>;
  cancelJob(id: string): Promise<void>;
  /** A link to a generated report (server only). */
  reportUrl(id: string, format: ReportFormat, options?: ReportOptions): string | null;
  /** The Project DNA card as SVG markup. */
  card(id: string, dark: boolean, scan?: string): Promise<string>;
  /** Lets the user pick a directory to analyze (desktop only). */
  pickDirectory?(): Promise<string | null>;
  /** Writes a report through a save dialog (desktop only); returns the saved path. */
  saveReport?(id: string, format: SaveFormat, options?: ReportOptions): Promise<string | null>;
  /** Opens a web link in the system browser (desktop only). */
  openExternal?(url: string): Promise<void>;
}

export class BackendError extends Error {
  override name = "BackendError";
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
  }
}

const TOKEN_KEY = "repodna-token";

function readStorage(key: string): string | null {
  try {
    return window.sessionStorage.getItem(key);
  } catch {
    return null;
  }
}

function writeStorage(key: string, value: string): void {
  try {
    window.sessionStorage.setItem(key, value);
  } catch {
    // Storage may be unavailable (private mode); the cookie still works.
  }
}

/**
 * Takes a `?token=` from the address (then removes it from the address bar), or the one
 * remembered for this tab. `repodna serve` normally exchanges the token for a cookie
 * before the app loads; this path covers development servers and proxies.
 */
export function takeToken(): string | null {
  const url = new URL(window.location.href);
  const token = url.searchParams.get("token");
  if (token) {
    writeStorage(TOKEN_KEY, token);
    url.searchParams.delete("token");
    window.history.replaceState(null, "", url.toString());
    return token;
  }
  return readStorage(TOKEN_KEY);
}

class ServerBackend implements Backend {
  readonly kind = "server" as const;

  constructor(private readonly token: string | null) {}

  private async request<T>(path: string, init?: RequestInit): Promise<T> {
    const headers = new Headers(init?.headers);
    if (this.token) {
      headers.set("X-RepoDNA-Token", this.token);
    }
    const response = await fetch(`/api/${path}`, {
      ...init,
      headers,
      credentials: "same-origin",
      cache: "no-store",
    });
    const text = await response.text();
    let body: unknown = undefined;
    try {
      body = text ? JSON.parse(text) : undefined;
    } catch {
      body = undefined;
    }
    if (!response.ok) {
      const message =
        body && typeof body === "object" && "error" in body
          ? String((body as { error: unknown }).error)
          : `The server answered ${response.status}.`;
      throw new BackendError(message, response.status);
    }
    return body as T;
  }

  session(): Promise<Session> {
    return this.request<Session>("session");
  }

  repositories(): Promise<RepositorySummary[]> {
    return this.request<RepositorySummary[]>("repositories");
  }

  repository(id: string): Promise<{ repository: RepositorySummary; scans: ScanSummary[] }> {
    return this.request(`repositories/${encodeURIComponent(id)}`);
  }

  async artifact(id: string, scan?: string): Promise<RepositoryDna> {
    const query = scan ? `?scan=${encodeURIComponent(scan)}` : "";
    const value = await this.request<unknown>(
      `repositories/${encodeURIComponent(id)}/artifact${query}`,
    );
    return checkArtifact(value).artifact;
  }

  startScan(input: string, profile?: string): Promise<JobState> {
    return this.request<JobState>("scans", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(profile ? { input, profile } : { input }),
    });
  }

  job(id: string): Promise<JobState> {
    return this.request<JobState>(`scans/${encodeURIComponent(id)}`);
  }

  async cancelJob(id: string): Promise<void> {
    await this.request(`scans/${encodeURIComponent(id)}`, { method: "DELETE" });
  }

  reportUrl(id: string, format: ReportFormat, options: ReportOptions = {}): string {
    const params = new URLSearchParams({ format });
    if (options.scan) {
      params.set("scan", options.scan);
    }
    if (options.theme) {
      params.set("theme", options.theme);
    }
    if (options.privacy) {
      params.set("privacy", options.privacy);
    }
    return `/api/repositories/${encodeURIComponent(id)}/report?${params.toString()}`;
  }

  async card(id: string, dark: boolean, scan?: string): Promise<string> {
    const params = new URLSearchParams();
    if (dark) {
      params.set("dark", "1");
    }
    if (scan) {
      params.set("scan", scan);
    }
    const headers = new Headers();
    if (this.token) {
      headers.set("X-RepoDNA-Token", this.token);
    }
    const response = await fetch(
      `/api/repositories/${encodeURIComponent(id)}/card.svg?${params.toString()}`,
      { headers, credentials: "same-origin", cache: "no-store" },
    );
    if (!response.ok) {
      throw new BackendError(`The server answered ${response.status}.`, response.status);
    }
    return response.text();
  }
}

/** `true` inside the desktop app. */
export function isDesktop(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export interface Detected {
  backend: Backend | null;
  session: Session | null;
  /** A server is running but this browser has not signed in. */
  signInNeeded: boolean;
}

/**
 * `true` for the addresses `repodna serve` answers on. It listens on 127.0.0.1 only and
 * refuses other host names, so a page from anywhere else (such as the web version on
 * GitHub Pages) has no server to ask.
 */
export function isLoopbackHost(hostname: string): boolean {
  return ["127.0.0.1", "localhost", "[::1]", "::1"].includes(hostname.toLowerCase());
}

/** Finds the backend this page can use. */
export async function detectBackend(): Promise<Detected> {
  if (isDesktop()) {
    const { DesktopBackend } = await import("./desktop");
    const backend = new DesktopBackend();
    return { backend, session: await backend.session(), signInNeeded: false };
  }
  if (!isLoopbackHost(window.location.hostname)) {
    return { backend: null, session: null, signInNeeded: false };
  }
  const token = takeToken();
  try {
    const health = await fetch("/api/health", { cache: "no-store" });
    const body: unknown = health.ok ? await health.json() : null;
    if (!body || typeof body !== "object" || (body as { status?: unknown }).status !== "ok") {
      return { backend: null, session: null, signInNeeded: false };
    }
  } catch {
    return { backend: null, session: null, signInNeeded: false };
  }
  const backend = new ServerBackend(token);
  try {
    return { backend, session: await backend.session(), signInNeeded: false };
  } catch (error) {
    if (error instanceof BackendError && error.status === 401) {
      return { backend: null, session: null, signInNeeded: true };
    }
    throw error;
  }
}
