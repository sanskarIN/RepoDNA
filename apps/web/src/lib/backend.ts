// Where analyses come from: the local server (`repodna serve`), the desktop app, or
// nowhere (static hosting: files opened in the browser and the bundled demo only).

import { checkArtifact, type RepositoryDna } from "@repodna/schema";
import { validGitRef } from "./github-source";
import { decodeNativeJson, NATIVE_LIMITS, NativeTransportError, requestNativeText, type NativeTransportOptions } from "./nativeTransport";

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

/** Bounded native history page with an opaque, repository-specific cursor. */
export interface HistoryPage {
  repositoryId: string;
  scans: ScanSummary[];
  nextCursor: string | null;
}

/** Explicit native Git revision selection; omission uses the URL or default branch. */
export interface RemoteRevision {
  kind: "branch" | "tag" | "commit";
  value: string;
}

/** Remote branches and tags are a snapshot, not immutable checkout promises. */
export interface SourceReference {
  kind: "branch" | "tag";
  name: string;
  object: string;
}

export interface SourceRefQuery {
  kind?: "branch" | "tag";
  search?: string;
}

export interface SourceReferences {
  references: SourceReference[];
  truncated: boolean;
}

/** Validate both HTTP and native IPC responses at the shared boundary. */
export function checkSourceReferences(value: unknown): SourceReferences {
  if (
    !isRecord(value) ||
    typeof value.truncated !== "boolean" ||
    !Array.isArray(value.references) ||
    value.references.length > 2_000
  ) {
    throw new BackendError("Invalid remote reference listing.", 200);
  }
  const seen = new Set<string>();
  const references: SourceReference[] = [];
  for (const item of value.references) {
    if (
      !isRecord(item) ||
      (item.kind !== "branch" && item.kind !== "tag") ||
      typeof item.name !== "string" ||
      item.name.length > 16_384 ||
      !validGitRef(item.name) ||
      typeof item.object !== "string" ||
      !/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/i.test(item.object)
    ) {
      throw new BackendError("Invalid remote reference listing.", 200);
    }
    const key = item.kind + ":" + item.name;
    if (seen.has(key)) {
      throw new BackendError("Duplicate remote reference in listing.", 200);
    }
    seen.add(key);
    references.push({ kind: item.kind, name: item.name, object: item.object });
  }
  return { references, truncated: value.truncated };
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
/** What the desktop app can save: a report, or the full report folder. */
export type SaveFormat = ReportFormat | "bundle";
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
  history?(id: string, after?: string): Promise<HistoryPage>;
  artifact(id: string, scan?: string): Promise<RepositoryDna>;
  /** Deletes every saved scan and artifact for one repository, not the source code. */
  deleteRepository(id: string): Promise<void>;
  /** Deletes one saved scan and its artifact. */
  deleteScan(repositoryId: string, scanId: string): Promise<void>;
  startScan(input: string, profile?: string, revision?: RemoteRevision): Promise<JobState>;
  /** User-requested network discovery, optional for native backends. */
  listRemoteReferences?(input: string, query?: SourceRefQuery): Promise<SourceReferences>;
  job(id: string): Promise<JobState>;
  /** Recent native jobs available for reattaching to an analysis. */
  listJobs?(): Promise<JobState[]>;
  cancelJob(id: string): Promise<void>;
  /** A link to a generated report (server only). */
  reportUrl(id: string, format: ReportFormat, options?: ReportOptions): string | null;
  /** The Project DNA card as SVG markup. */
  card(id: string, dark: boolean, scan?: string): Promise<string>;
  /** Lets the user pick a directory to analyze (desktop only). */
  pickDirectory?(): Promise<string | null>;
  /** Writes a report through a save dialog (desktop only); returns the saved path. */
  saveReport?(id: string, format: SaveFormat, options?: ReportOptions): Promise<string | null>;
  /** Writes the Project DNA card as SVG or PNG through a save dialog (desktop only). */
  saveCard?(id: string, dark: boolean, png: boolean, scan?: string): Promise<string | null>;
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function isCount(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function isNullableString(value: unknown): value is string | null | undefined {
  return value === null || value === undefined || typeof value === "string";
}

/** Validate list items as well as the enclosing JSON array. */
function isRepositorySummary(value: unknown): value is RepositorySummary {
  return (
    isRecord(value) &&
    typeof value.id === "string" &&
    value.id.length > 0 &&
    typeof value.name === "string" &&
    typeof value.location === "string" &&
    typeof value.kind === "string" &&
    isCount(value.scans) &&
    typeof value.firstScannedAt === "string" &&
    typeof value.lastScannedAt === "string"
  );
}

function isScanSummary(value: unknown): value is ScanSummary {
  if (!isRecord(value) || !isRecord(value.findings)) return false;
  return (
    typeof value.id === "string" &&
    value.id.length > 0 &&
    typeof value.generatedAt === "string" &&
    typeof value.profile === "string" &&
    isNullableString(value.revision) &&
    typeof value.dnaHash === "string" &&
    typeof value.toolVersion === "string" &&
    isCount(value.files) &&
    isCount(value.codeLines) &&
    isCount(value.commits) &&
    isCount(value.contributors) &&
    ["critical", "warning", "attention", "info", "suppressed"].every((key) =>
      isCount((value.findings as Record<string, unknown>)[key]),
    )
  );
}

/** Reject unbounded, malformed, cross-repository or repeating pages. */
export function checkHistoryPage(value: unknown, id: string, after?: string): HistoryPage {
  if (
    !isRecord(value) ||
    value.repositoryId !== id ||
    !Array.isArray(value.scans) ||
    value.scans.length > 100 ||
    !value.scans.every(isScanSummary) ||
    new Set(value.scans.map((scan) => scan.id)).size !== value.scans.length ||
    (value.nextCursor !== null &&
      (typeof value.nextCursor !== "string" ||
        value.nextCursor.length === 0 ||
        value.nextCursor.length > 200 ||
        value.nextCursor === after ||
        value.scans.length === 0))
  ) {
    throw new BackendError("The backend returned an invalid history page.", 200);
  }
  return {
    repositoryId: id,
    scans: value.scans,
    nextCursor: value.nextCursor,
  };
}

/** Reject malformed nested progress data before it reaches the UI. */
function isJobState(value: unknown, allowed: readonly JobState["status"][]): value is JobState {
  if (!isRecord(value)) return false;
  return (
    typeof value.id === "string" &&
    value.id.length > 0 &&
    typeof value.input === "string" &&
    typeof value.status === "string" &&
    allowed.includes(value.status as JobState["status"]) &&
    isNullableString(value.stage) &&
    Array.isArray(value.stages) &&
    value.stages.every(
      (stage: unknown) =>
        Array.isArray(stage) &&
        stage.length === 2 &&
        stage.every((part: unknown) => typeof part === "string"),
    ) &&
    isCount(value.filesDone) &&
    isCount(value.filesTotal) &&
    isNullableString(value.repositoryId) &&
    isNullableString(value.scanId) &&
    isNullableString(value.error) &&
    Array.isArray(value.warnings) &&
    value.warnings.every((warning: unknown) => typeof warning === "string") &&
    typeof value.startedAt === "string" &&
    isNullableString(value.finishedAt)
  );
}

const JOB_STATUSES = ["running", "completed", "failed", "cancelled"] as const;

/** Validate a native job and, when supplied, the requested identity. */
export function checkScanJob(value: unknown, id?: string): JobState {
  if (!isJobState(value, JOB_STATUSES) || (id !== undefined && value.id !== id)) {
    throw new BackendError("The backend returned an invalid scan job.", 200);
  }
  return value;
}

/** Keep list validation bounded and reject duplicate job identities. */
export function checkScanJobs(value: unknown): JobState[] {
  if (!Array.isArray(value) || value.length > 50) {
    throw new BackendError("The backend returned an invalid scan job list.", 200);
  }
  const jobs = value.map((item: unknown) => checkScanJob(item));
  if (new Set(jobs.map((job) => job.id)).size !== jobs.length) {
    throw new BackendError("The backend returned duplicate scan job IDs.", 200);
  }
  return jobs;
}

/** Both HTTP and IPC must validate the same response contracts. */
export function checkSession(value: unknown): Session {
  if (!isRecord(value) || typeof value.version !== "string" || !value.version || typeof value.allowScans !== "boolean") {
    throw new BackendError("The server returned an invalid session.", 200);
  }
  return { version: value.version, allowScans: value.allowScans };
}

export function checkRepositories(value: unknown): RepositorySummary[] {
  if (!Array.isArray(value) || !value.every(isRepositorySummary) || new Set(value.map((item) => item.id)).size !== value.length) {
    throw new BackendError("The server returned an invalid repository list.", 200);
  }
  return value;
}

export function checkRepository(value: unknown, id: string): { repository: RepositorySummary; scans: ScanSummary[] } {
  if (!isRecord(value) || !isRepositorySummary(value.repository) || value.repository.id !== id ||
    !Array.isArray(value.scans) || value.scans.length > 100 || !value.scans.every(isScanSummary) ||
    new Set(value.scans.map((scan) => scan.id)).size !== value.scans.length) {
    throw new BackendError("The server returned invalid repository details.", 200);
  }
  return { repository: value.repository, scans: value.scans };
}

export function checkNewScanJob(value: unknown): JobState {
  if (!isJobState(value, ["running"])) throw new BackendError("The server returned an invalid new scan job.", 200);
  return value;
}

export function checkNativeArtifact(value: unknown, scan?: string): RepositoryDna {
  const artifact = checkArtifact(value).artifact;
  // Legacy records can have an empty artifact ID, so the exact check is only
  // possible when the stored artifact declares a run identity.
  if (scan && artifact.analysisMetadata.id && artifact.analysisMetadata.id !== scan) {
    throw new BackendError("The backend returned a different stored analysis.", 200);
  }
  return artifact;
}

export function checkSvgCard(value: unknown, status = 200): string {
  if (typeof value !== "string" || !/^\s*(?:<\?xml[^>]*>\s*)?<svg\b/i.test(value)) {
    throw new BackendError("The server did not return an SVG card.", status);
  }
  return value;
}

export function checkOptionalPath(value: unknown): string | null {
  if (value === null || (typeof value === "string" && value.length > 0)) return value;
  throw new BackendError("The desktop app returned an invalid file dialog result.", 200);
}

function asBackendError(reason: unknown): never {
  if (reason instanceof NativeTransportError) throw new BackendError(reason.message, reason.status);
  throw reason;
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
  if (url.searchParams.has("token")) {
    const token = url.searchParams.get("token");
    // Even an empty token should not linger in the address bar. Removing every
    // token parameter also avoids accidentally sharing it when copying the URL.
    url.searchParams.delete("token");
    window.history.replaceState(null, "", url.toString());
    if (token) {
      writeStorage(TOKEN_KEY, token);
      return token;
    }
  }
  return readStorage(TOKEN_KEY);
}

export class ServerBackend implements Backend {
  readonly kind = "server" as const;

  constructor(
    private readonly token: string | null,
    private readonly transport: Pick<NativeTransportOptions, "clock" | "fetcher"> = {},
  ) {}

  private async request<T>(path: string, init?: RequestInit, limits: NativeTransportOptions = NATIVE_LIMITS.small): Promise<T> {
    const headers = new Headers(init?.headers);
    if (this.token) headers.set("X-RepoDNA-Token", this.token);
    try {
      const response = await requestNativeText(`/api/${path}`, { ...init, headers }, { ...limits, ...this.transport });
      return decodeNativeJson(response, init?.method?.toUpperCase() === "DELETE") as T;
    } catch (reason) {
      return asBackendError(reason);
    }
  }

  async session(): Promise<Session> {
    return checkSession(await this.request<unknown>("session"));
  }

  async repositories(): Promise<RepositorySummary[]> {
    return checkRepositories(await this.request<unknown>("repositories", undefined, NATIVE_LIMITS.list));
  }

  async repository(id: string): Promise<{ repository: RepositorySummary; scans: ScanSummary[] }> {
    return checkRepository(await this.request<unknown>(`repositories/${encodeURIComponent(id)}`, undefined, NATIVE_LIMITS.list), id);
  }

  async history(id: string, after?: string): Promise<HistoryPage> {
    const params = new URLSearchParams({ limit: "50" });
    if (after) params.set("after", after);
    const path = `repositories/${encodeURIComponent(id)}/history?${params.toString()}`;
    return checkHistoryPage(await this.request<unknown>(path, undefined, NATIVE_LIMITS.list), id, after);
  }

  async artifact(id: string, scan?: string): Promise<RepositoryDna> {
    const query = scan ? `?scan=${encodeURIComponent(scan)}` : "";
    const value = await this.request<unknown>(
      `repositories/${encodeURIComponent(id)}/artifact${query}`,
      undefined, NATIVE_LIMITS.artifact,
    );
    return checkNativeArtifact(value, scan);
  }

  async deleteRepository(id: string): Promise<void> {
    await this.request(`repositories/${encodeURIComponent(id)}`, { method: "DELETE" });
  }

  async deleteScan(repositoryId: string, scanId: string): Promise<void> {
    await this.request(
      `repositories/${encodeURIComponent(repositoryId)}/scans/${encodeURIComponent(scanId)}`,
      { method: "DELETE" },
    );
  }

  async listRemoteReferences(input: string, query?: SourceRefQuery): Promise<SourceReferences> {
    const value = await this.request<unknown>("remote-references", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ input, ...query }),
    }, NATIVE_LIMITS.discovery);
    return checkSourceReferences(value);
  }

  async startScan(input: string, profile?: string, revision?: RemoteRevision): Promise<JobState> {
    const value = await this.request<unknown>("scans", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        input,
        ...(profile ? { profile } : {}),
        ...(revision ? { [revision.kind]: revision.value } : {}),
      }),
    });
    return checkNewScanJob(value);
  }

  async job(id: string): Promise<JobState> {
    const value = await this.request<unknown>(`scans/${encodeURIComponent(id)}`);
    return checkScanJob(value, id);
  }

  async listJobs(): Promise<JobState[]> {
    return checkScanJobs(await this.request<unknown>("scans", undefined, NATIVE_LIMITS.list));
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
    try {
      const response = await requestNativeText(
        `/api/repositories/${encodeURIComponent(id)}/card.svg?${params.toString()}`,
        { headers }, { ...NATIVE_LIMITS.card, ...this.transport },
      );
      if (!response.ok) throw new BackendError(`The server answered ${response.status}.`, response.status);
      return checkSvgCard(response.text, response.status);
    } catch (reason) {
      return asBackendError(reason);
    }
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
    const health = await requestNativeText("/api/health", {}, { maxBytes: 65_536, deadlineMs: 5_000 });
    const body = health.ok ? decodeNativeJson(health) : null;
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
