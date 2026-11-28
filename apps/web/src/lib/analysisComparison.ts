import type {
  DependencyRecord,
  FileRecord,
  Finding,
  LanguageStat,
  ModuleRecord,
  ModuleEdge,
  DependencyCycle,
  RemoteInfo,
  RepositoryDna,
  SectionStatus,
} from "@repodna/schema";

export type RepositoryRelation = "same" | "different" | "unknown";

export interface RelationEvidence {
  relation: RepositoryRelation;
  reason: string;
}

const HOSTED = new Set(["github.com", "gitlab.com", "bitbucket.org", "codeberg.org"]);

function compareText(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

/** Only canonicalize cross-transport remotes for known hosting providers.
 * Never treat a shared display name, fork upstream, or owner alone as proof. */
export function remoteIdentity(remote: string): string | null {
  // Reject malformed inputs before WHATWG URL normalizes path or authority.
  if (/[\x00-\x20\x7f-\x9f\\%]/.test(remote) || /(?:^|\/)(?:\.|\.\.)(?:\/|$)/.test(remote)) return null;
  let host: string;
  let path: string;
  let transport: string;
  let port = "";
  const scp = /^(?:[^\s/@:]+@)?([^\s/:]+\.[^\s/:]+):([^\s]+)$/.exec(remote);
  if (scp && !remote.includes("://")) {
    host = scp[1]!.toLowerCase();
    path = scp[2]!;
    transport = "scp";
  } else {
    try {
      if (!/^(?:https?|ssh|git|git\+ssh|ssh\+git):\/\//i.test(remote)) return null;
      const url = new URL(remote);
      transport = url.protocol.slice(0, -1).toLowerCase();
      if (!["https", "http", "ssh", "git", "git+ssh", "ssh+git"].includes(transport)) return null;
      if (url.search || url.hash) return null;
      host = url.hostname.toLowerCase();
      port = url.port;
      path = url.pathname;
    } catch {
      return null;
    }
  }
  if (!host || /[\s\\%]/.test(host)) return null;
  if (host === "www.github.com") host = "github.com";
  // Avoid silently merging aliases encoded differently or paths with traversal.
  if (/[\\%?#\s]/.test(path)) return null;
  const parts = path.replace(/^\/+|\/+$/g, "").split("/");
  if (parts.length < 2 || parts.some((part) => !part || part === "." || part === "..")) return null;
  if (["github.com", "bitbucket.org", "codeberg.org"].includes(host) && parts.length !== 2) return null;
  const joined = parts.join("/");
  const normalized = host === "github.com" ? joined.toLowerCase().replace(/\.git$/, "") : joined.replace(/\.git$/, "");
  if (!normalized || normalized.endsWith("/")) return null;
  if (port === "22" && ["ssh", "git+ssh", "ssh+git"].includes(transport)) port = "";
  if (port === "9418" && transport === "git") port = "";
  if (HOSTED.has(host) && !port) return `${host}/${normalized}`;
  // A self-hosted SSH relative path and an HTTPS path may be unrelated.
  // Preserve the transport, port and absolute/relative path distinction.
  return `${transport}://${host}${port ? `:${port}` : ""}/${path.startsWith("/") ? "/" : ""}${normalized}`;
}

function primaryRemote(remotes: readonly RemoteInfo[]): string | null {
  const origins = remotes.filter((remote) => remote.name === "origin");
  if (origins.length > 1) return null;
  const candidate = origins[0] ?? (remotes.length === 1 ? remotes[0] : undefined);
  return candidate ? remoteIdentity(candidate.url) : null;
}

/** A local library selection is stronger evidence than a display-name match. */
export function repositoryRelation(
  current: RepositoryDna,
  baseline: RepositoryDna,
  currentStoredId?: string | null,
  baselineStoredId?: string | null,
): RelationEvidence {
  if (currentStoredId && currentStoredId === baselineStoredId) {
    return { relation: "same", reason: "Both analyses belong to the same local repository record." };
  }
  if (current.analysisMetadata.id && current.analysisMetadata.id === baseline.analysisMetadata.id) {
    return { relation: "same", reason: "Both documents identify the same analysis run." };
  }
  const a = primaryRemote(current.identity.remotes ?? []);
  const b = primaryRemote(baseline.identity.remotes ?? []);
  if (a && b) {
    if (a === b) return { relation: "same", reason: "The primary repository remotes match." };
    // For known hosted providers distinct canonical paths are useful evidence.
    if (!a.includes("://") && !b.includes("://")) {
      return { relation: "different", reason: "The primary hosted repository remotes differ." };
    }
    return { relation: "unknown", reason: "The remote identities differ; self-hosted aliases cannot be verified." };
  }
  return {
    relation: "unknown",
    reason: "There is no shared repository record or matching primary remote. Display names alone do not establish identity.",
  };
}

export interface ComparisonCoverage {
  current: "complete" | "partial" | "unavailable";
  baseline: "complete" | "partial" | "unavailable";
  /** Whether absence from either inventory can be interpreted confidently. */
  absenceComparable: boolean;
  notes: string[];
}

export type ChangeKind =
  | "added"
  | "removed"
  | "modified"
  | "metadata"
  | "possible-move"
  | "only-current"
  | "only-baseline"
  | "suppressed"
  | "unsuppressed";

export interface AnalysisChange {
  key: string;
  label: string;
  kind: ChangeKind;
  before: string;
  after: string;
  details: string[];
}

export interface ChangeGroup {
  coverage: ComparisonCoverage;
  changes: AnalysisChange[];
  ambiguousKeys: number;
  /** Matching keys whose content equality was not established. */
  unverified: number;
}

export interface AnalysisComparison {
  files: ChangeGroup;
  dependencies: ChangeGroup;
  languages: ChangeGroup;
  modules: ChangeGroup;
  moduleEdges: ChangeGroup;
  cycles: ChangeGroup;
  findings: ChangeGroup;
}

function completeness(status: SectionStatus, inventoryComplete = true): ComparisonCoverage["current"] {
  if (status === "skipped" || status === "unavailable") return "unavailable";
  return status === "analyzed" && inventoryComplete ? "complete" : "partial";
}

function coverage(
  current: ComparisonCoverage["current"],
  baseline: ComparisonCoverage["baseline"],
  metadataComparable: boolean,
  extra: readonly string[] = [],
  allowAbsence = true,
): ComparisonCoverage {
  const notes = [...extra];
  if (current !== "complete" || baseline !== "complete") {
    notes.push("At least one inventory is incomplete; absence is not evidence of a removal.");
  }
  if (!metadataComparable) {
    notes.push("Analysis configuration, version, profile or privacy settings differ; inventory changes may reflect measurement differences.");
  }
  return {
    current,
    baseline,
    absenceComparable: allowAbsence && current === "complete" && baseline === "complete" && metadataComparable,
    notes,
  };
}

function comparableMetadata(a: RepositoryDna, b: RepositoryDna): boolean {
  const x = a.analysisMetadata;
  const y = b.analysisMetadata;
  return (
    !x.partial && !y.partial &&
    !!x.configHash && x.configHash === y.configHash &&
    x.profile === y.profile &&
    !!a.tool.version && a.tool.version === b.tool.version &&
    a.schemaVersion === b.schemaVersion &&
    x.privacy.preset === y.privacy.preset &&
    sameStrings(x.privacy.redactions, y.privacy.redactions)
  );
}

interface UniqueIndex<T> {
  values: Map<string, T>;
  ambiguous: Set<string>;
}

/** Conflicting duplicate keys must not silently become arbitrary last-write wins. */
function uniqueIndex<T>(values: readonly T[], key: (value: T) => string): UniqueIndex<T> {
  const map = new Map<string, T>();
  const ambiguous = new Set<string>();
  for (const item of values) {
    const id = key(item);
    if (ambiguous.has(id)) continue;
    if (map.has(id)) {
      map.delete(id);
      ambiguous.add(id);
    } else {
      map.set(id, item);
    }
  }
  return { values: map, ambiguous };
}

function mergedKeys<T>(a: Map<string, T>, b: Map<string, T>): string[] {
  return [...new Set([...a.keys(), ...b.keys()])].sort(compareText);
}

function sameStrings(a: readonly string[] = [], b: readonly string[] = []): boolean {
  const x = [...new Set(a)].sort(compareText);
  const y = [...new Set(b)].sort(compareText);
  return x.length === y.length && x.every((item, index) => item === y[index]);
}

function missingKind(available: ComparisonCoverage, side: "current" | "baseline"): ChangeKind {
  if (!available.absenceComparable) return side === "current" ? "only-current" : "only-baseline";
  return side === "current" ? "added" : "removed";
}

function validHash(file: FileRecord): string | null {
  return typeof file.hash === "string" && /^[0-9a-f]{16}$/i.test(file.hash)
    ? file.hash.toLowerCase()
    : null;
}

function fileLabel(file: FileRecord): string {
  return `${file.bytes.toLocaleString("en-US")} bytes${file.lines ? ` · ${file.lines.code.toLocaleString("en-US")} code lines` : ""}`;
}

function fileChanges(before: FileRecord, after: FileRecord): { details: string[]; contentChanged: boolean; verified: boolean } {
  const details: string[] = [];
  const oldHash = validHash(before);
  const newHash = validHash(after);
  const verified = oldHash !== null && newHash !== null;
  const sizeChanged = before.bytes !== after.bytes;
  const contentChanged = sizeChanged || (verified && oldHash !== newHash);
  if (sizeChanged) details.push("Byte size changed");
  if (verified && oldHash !== newHash) details.push("Recorded content hash changed");
  if (before.category !== after.category) details.push(`Classification: ${before.category} → ${after.category}`);
  if ((before.language ?? null) !== (after.language ?? null)) details.push("Detected language changed");
  if ((before.module ?? null) !== (after.module ?? null)) details.push("Module assignment changed");
  if (!!before.binary !== !!after.binary) details.push("Binary classification changed");
  if (!!before.generated !== !!after.generated) details.push("Generated-file classification changed");
  if (!!before.vendored !== !!after.vendored) details.push("Vendored-file classification changed");
  if (before.lines && after.lines) {
    const a = before.lines;
    const b = after.lines;
    if (a.code !== b.code || a.comment !== b.comment || a.blank !== b.blank || a.total !== b.total) {
      details.push("Recorded line counts changed");
    }
  } else if (!!before.lines !== !!after.lines) {
    details.push("Line-count availability changed");
  }
  if ((before.skipped ?? null) !== (after.skipped ?? null)) details.push("Content-analysis skip status changed");
  return { details, contentChanged, verified: verified || sizeChanged };
}

function hashIndex(files: readonly FileRecord[]): Map<string, FileRecord[]> {
  const index = new Map<string, FileRecord[]>();
  for (const file of files) {
    const hash = validHash(file);
    if (!hash) continue;
    const key = `${hash}:${file.bytes}`;
    const bucket = index.get(key);
    if (bucket) bucket.push(file);
    else index.set(key, [file]);
  }
  return index;
}

function filesComparison(a: RepositoryDna, b: RepositoryDna, metadata: boolean): ChangeGroup {
  const available = coverage(
    completeness(a.structure.status, !a.structure.truncated && a.structure.files.length === a.structure.totalFiles),
    completeness(b.structure.status, !b.structure.truncated && b.structure.files.length === b.structure.totalFiles),
    metadata,
    ["A matching short hash and byte size suggest matching content; this is not a Git rename or content diff."],
  );
  const current = uniqueIndex(a.structure.files, (file) => file.path);
  const baseline = uniqueIndex(b.structure.files, (file) => file.path);
  const ambiguous = new Set([...current.ambiguous, ...baseline.ambiguous]);
  const changes: AnalysisChange[] = [];
  const added: FileRecord[] = [];
  const removed: FileRecord[] = [];
  let unverified = 0;
  for (const key of mergedKeys(current.values, baseline.values)) {
    if (ambiguous.has(key)) continue;
    const before = baseline.values.get(key);
    const after = current.values.get(key);
    if (before && after) {
      const comparison = fileChanges(before, after);
      if (!comparison.verified) unverified += 1;
      if (comparison.details.length > 0) {
        changes.push({
          key: `file:${key}`, label: key,
          kind: comparison.contentChanged ? "modified" : "metadata",
          before: fileLabel(before), after: fileLabel(after), details: comparison.details,
        });
      }
    } else if (after) added.push(after);
    else if (before) removed.push(before);
  }
  // Pair unique hashes only when both full inventories are comparable. Copies
  // or multiple candidates remain separate additions/removals, not fake moves.
  const usedAdded = new Set<string>();
  const usedRemoved = new Set<string>();
  if (available.absenceComparable && ambiguous.size === 0) {
    const currentHashes = hashIndex(a.structure.files);
    const baselineHashes = hashIndex(b.structure.files);
    for (const file of removed) {
      const hash = validHash(file);
      if (!hash) continue;
      const hashKey = `${hash}:${file.bytes}`;
      const old = baselineHashes.get(hashKey);
      const next = currentHashes.get(hashKey);
      if (old?.length !== 1 || next?.length !== 1) continue;
      const candidate = next[0]!;
      if (baseline.values.has(candidate.path)) continue;
      usedRemoved.add(file.path);
      usedAdded.add(candidate.path);
      changes.push({
        key: `move:${JSON.stringify([file.path, candidate.path])}`,
        label: `${file.path} → ${candidate.path}`,
        kind: "possible-move", before: file.path, after: candidate.path,
        details: ["Unique recorded short hash and byte size match"],
      });
    }
  }
  for (const file of added) {
    if (usedAdded.has(file.path)) continue;
    changes.push({ key: `file:${file.path}`, label: file.path, kind: missingKind(available, "current"), before: "—", after: fileLabel(file), details: [] });
  }
  for (const file of removed) {
    if (usedRemoved.has(file.path)) continue;
    changes.push({ key: `file:${file.path}`, label: file.path, kind: missingKind(available, "baseline"), before: fileLabel(file), after: "—", details: [] });
  }
  changes.sort((x, y) => compareText(x.label, y.label) || compareText(x.key, y.key));
  return { coverage: available, changes, ambiguousKeys: ambiguous.size, unverified };
}

function dependencyKey(dependency: DependencyRecord): string {
  return JSON.stringify([dependency.ecosystem, dependency.name]);
}

function dependencyGroups(records: readonly DependencyRecord[]): Map<string, DependencyRecord[]> {
  const groups = new Map<string, DependencyRecord[]>();
  for (const item of records) {
    const key = dependencyKey(item);
    const group = groups.get(key);
    if (group) group.push(item);
    else groups.set(key, [item]);
  }
  return groups;
}

function normalizedDependency(dependency: DependencyRecord): string {
  return JSON.stringify([
    dependency.scope,
    dependency.source,
    dependency.requirement ?? null,
    [...new Set(dependency.resolved ?? [])].sort(compareText),
    [...new Set(dependency.manifests)].sort(compareText),
    !!dependency.internal,
  ]);
}

function groupText(records: readonly DependencyRecord[]): string {
  return [...new Set(records.map((dependency) => {
    const version = dependency.requirement ?? "unspecified";
    const resolved = dependency.resolved?.length ? `; resolved ${[...new Set(dependency.resolved)].sort(compareText).join(", ")}` : "";
    return `${dependency.scope} · ${dependency.source} · ${version}${resolved}`;
  }))].sort(compareText).join(" | ");
}

function dependenciesComparison(a: RepositoryDna, b: RepositoryDna, metadata: boolean): ChangeGroup {
  const available = coverage(completeness(a.dependencies.status), completeness(b.dependencies.status), metadata);
  const current = dependencyGroups(a.dependencies.dependencies);
  const baseline = dependencyGroups(b.dependencies.dependencies);
  const changes: AnalysisChange[] = [];
  for (const key of mergedKeys(current, baseline)) {
    const before = baseline.get(key);
    const after = current.get(key);
    const sample = (after ?? before)?.[0];
    if (!sample) continue;
    const label = `${sample.ecosystem}: ${sample.name}`;
    if (before && after) {
      if (sameStrings(before.map(normalizedDependency), after.map(normalizedDependency))) continue;
      const details: string[] = [];
      const fields: [string, (record: DependencyRecord) => readonly string[]][] = [
        ["Declared requirement", (record) => [JSON.stringify(record.requirement ?? null)]],
        ["Resolved versions", (record) => record.resolved ?? []],
        ["Source kind", (record) => [record.source]],
        ["Usage scope", (record) => [record.scope]],
        ["Declaring manifests", (record) => record.manifests],
        ["Internal-package classification", (record) => [String(!!record.internal)]],
      ];
      for (const [name, select] of fields) {
        if (!sameStrings(before.flatMap(select), after.flatMap(select))) details.push(`${name} changed`);
      }
      if (details.length === 0) details.push("Association of requirements, scopes or manifests changed");
      changes.push({ key, label, kind: "modified", before: groupText(before), after: groupText(after), details });
    } else if (after) {
      changes.push({ key, label, kind: missingKind(available, "current"), before: "—", after: groupText(after), details: [] });
    } else if (before) {
      changes.push({ key, label, kind: missingKind(available, "baseline"), before: groupText(before), after: "—", details: [] });
    }
  }
  return { coverage: available, changes, ambiguousKeys: 0, unverified: 0 };
}

function findingLabel(finding: Finding): string {
  return `${finding.severity} · ${finding.confidence} confidence${finding.suppressed != null ? " · suppressed" : ""}`;
}

function findingsComparison(a: RepositoryDna, b: RepositoryDna, metadata: boolean): ChangeGroup {
  // Findings are observations, not issue-tracker truth. Even equivalent scans
  // cannot prove that an absent finding was repaired.
  const available = coverage(
    a.analysisMetadata.partial ? "partial" : "complete",
    b.analysisMetadata.partial ? "partial" : "complete",
    metadata,
    ["Finding absence means not reported in this artifact, not necessarily fixed."],
    false,
  );
  const current = uniqueIndex(a.findings, (finding) => finding.id);
  const baseline = uniqueIndex(b.findings, (finding) => finding.id);
  const ambiguous = new Set([...current.ambiguous, ...baseline.ambiguous]);
  const changes: AnalysisChange[] = [];
  for (const id of mergedKeys(current.values, baseline.values)) {
    if (ambiguous.has(id)) continue;
    const before = baseline.values.get(id);
    const after = current.values.get(id);
    if (before && after) {
      const oldSuppressed = before.suppressed != null;
      const newSuppressed = after.suppressed != null;
      const details: string[] = [];
      if (oldSuppressed !== newSuppressed) details.push("Suppression status changed");
      if (before.severity !== after.severity) details.push("Severity changed");
      if (before.confidence !== after.confidence) details.push("Confidence changed");
      if (before.title !== after.title || before.summary !== after.summary) details.push("Observation wording changed");
      if (before.rule !== after.rule || before.category !== after.category) details.push("Finding classification changed");
      if (before.method !== after.method) details.push("Analysis method changed");
      if (!sameStrings(before.paths, after.paths)) details.push("Reported paths changed");
      if (before.evidence.length !== after.evidence.length) details.push("Evidence count changed");
      if (oldSuppressed && newSuppressed && (before.suppressed?.reason !== after.suppressed?.reason || before.suppressed?.source !== after.suppressed?.source)) details.push("Suppression explanation changed");
      if (details.length === 0) continue;
      changes.push({
        key: id, label: after.title,
        kind: oldSuppressed !== newSuppressed ? (newSuppressed ? "suppressed" : "unsuppressed") : "modified",
        before: findingLabel(before), after: findingLabel(after), details,
      });
    } else if (after) {
      changes.push({ key: id, label: after.title, kind: "only-current", before: "—", after: findingLabel(after), details: [after.rule] });
    } else if (before) {
      changes.push({ key: id, label: before.title, kind: "only-baseline", before: findingLabel(before), after: "—", details: [before.rule] });
    }
  }
  return { coverage: available, changes, ambiguousKeys: ambiguous.size, unverified: 0 };
}

interface InventoryOptions<T> {
  current: readonly T[];
  baseline: readonly T[];
  coverage: ComparisonCoverage;
  key(item: T): string;
  label(item: T): string;
  summary(item: T): string;
  details(before: T, after: T): string[];
}

/** Shared deterministic inventory join; duplicate IDs are never arbitrarily paired. */
function compareInventory<T>(options: InventoryOptions<T>): ChangeGroup {
  const current = uniqueIndex(options.current, options.key);
  const baseline = uniqueIndex(options.baseline, options.key);
  const ambiguous = new Set([...current.ambiguous, ...baseline.ambiguous]);
  const changes: AnalysisChange[] = [];
  for (const key of mergedKeys(current.values, baseline.values)) {
    if (ambiguous.has(key)) continue;
    const before = baseline.values.get(key);
    const after = current.values.get(key);
    if (before !== undefined && after !== undefined) {
      const details = options.details(before, after);
      if (details.length) changes.push({
        key, label: options.label(after), kind: "modified",
        before: options.summary(before), after: options.summary(after), details,
      });
    } else if (after !== undefined) {
      changes.push({
        key, label: options.label(after), kind: missingKind(options.coverage, "current"),
        before: "—", after: options.summary(after), details: [],
      });
    } else if (before !== undefined) {
      changes.push({
        key, label: options.label(before), kind: missingKind(options.coverage, "baseline"),
        before: options.summary(before), after: "—", details: [],
      });
    }
  }
  return { coverage: options.coverage, changes, ambiguousKeys: ambiguous.size, unverified: 0 };
}

function displayCount(value: number): string {
  return value.toLocaleString("en-US");
}

function displayRatio(value: number): string {
  return `${(value * 100).toFixed(2)}%`;
}

function observedFields<T>(before: T, after: T, fields: readonly [string, (item: T) => string | number | boolean | null | undefined][]): string[] {
  const changes: string[] = [];
  for (const [label, read] of fields) {
    const a = read(before);
    const b = read(after);
    if (a !== b) changes.push(`${label}: ${a ?? "not recorded"} → ${b ?? "not recorded"}`);
  }
  return changes;
}

function languagesComparison(a: RepositoryDna, b: RepositoryDna, metadata: boolean): ChangeGroup {
  const available = coverage(
    completeness(a.languages.status, !a.structure.truncated),
    completeness(b.languages.status, !b.structure.truncated), metadata,
    ["Language shares describe the analyzed composition, not the percentage of all repository bytes."],
  );
  return compareInventory<LanguageStat>({
    current: a.languages.languages,
    baseline: b.languages.languages,
    coverage: available,
    key: (item) => item.id,
    label: (item) => item.name,
    summary: (item) => `${displayCount(item.files)} files · ${displayCount(item.codeLines)} code lines · ${displayRatio(item.share)}`,
    details: (before, after) => observedFields(before, after, [
      ["Files", (item) => item.files],
      ["Code lines", (item) => item.codeLines],
      ["Comment-only lines", (item) => item.commentLines],
      ["Blank lines", (item) => item.blankLines],
      ["Bytes", (item) => item.bytes],
      ["Composition share", (item) => item.share],
      ["Analysis capability", (item) => item.capability],
      ["Language family", (item) => item.kind],
      ["Display name", (item) => item.name],
    ]),
  });
}

/** A change in graph inference method is a change in the measurement, not a
 * trustworthy indication that a module or dependency disappeared. */
function architectureCoverage(a: RepositoryDna, b: RepositoryDna, metadata: boolean, fileLevel = false): ComparisonCoverage {
  const aComplete = !a.structure.truncated && (!fileLevel || !a.architecture.fileEdgesTruncated);
  const bComplete = !b.structure.truncated && (!fileLevel || !b.architecture.fileEdgesTruncated);
  const methodMatches = !!a.architecture.method && a.architecture.method === b.architecture.method;
  return coverage(
    completeness(a.architecture.status, aComplete),
    completeness(b.architecture.status, bComplete),
    metadata && methodMatches,
    ["Architecture is inferred from the recorded dependency graph; differences may reflect changes in inference or source discovery.",
      ...(!methodMatches ? ["The recorded architecture inference methods differ."] : [])],
  );
}

function moduleSummary(item: ModuleRecord): string {
  return `${displayCount(item.files)} files · ${displayCount(item.codeLines)} code lines · ${item.fanIn} incoming / ${item.fanOut} outgoing`;
}

function modulesComparison(a: RepositoryDna, b: RepositoryDna, metadata: boolean): ChangeGroup {
  return compareInventory<ModuleRecord>({
    current: a.architecture.modules,
    baseline: b.architecture.modules,
    coverage: architectureCoverage(a, b, metadata),
    key: (item) => item.id,
    label: (item) => item.path || "(root)",
    summary: moduleSummary,
    details: (before, after) => {
      const details = observedFields(before, after, [
        ["Display name", (item) => item.name],
        ["Boundary path", (item) => item.path],
        ["Boundary kind", (item) => item.kind],
        ["Inferred boundary", (item) => item.inferred],
        ["Boundary confidence", (item) => item.confidence],
        ["Primary language", (item) => item.language ?? null],
        ["Files", (item) => item.files],
        ["Code lines", (item) => item.codeLines],
        ["Incoming modules", (item) => item.fanIn],
        ["Outgoing modules", (item) => item.fanOut],
        ["Layer", (item) => item.layer ?? null],
        ["Instability", (item) => item.instability],
        ["Centrality", (item) => item.centrality],
      ]);
      if (!sameStrings(before.externalDependencies, after.externalDependencies)) details.push("Recorded external dependencies changed");
      return details;
    },
  });
}

function edgeKey(edge: ModuleEdge): string {
  // Direction and boundary are preserved even when an ID contains separators.
  return JSON.stringify([edge.from, edge.to]);
}

function moduleEdgesComparison(a: RepositoryDna, b: RepositoryDna, metadata: boolean): ChangeGroup {
  return compareInventory<ModuleEdge>({
    current: a.architecture.moduleEdges,
    baseline: b.architecture.moduleEdges,
    coverage: architectureCoverage(a, b, metadata),
    key: edgeKey,
    label: (edge) => `${edge.from} → ${edge.to}`,
    summary: (edge) => `${displayCount(edge.weight)} supporting file edges · ${edge.confidence} confidence`,
    details: (before, after) => observedFields(before, after, [
      ["Supporting file-edge count", (item) => item.weight],
      ["Confidence", (item) => item.confidence],
    ]),
  });
}

function cycleKey(cycle: DependencyCycle): string {
  // Do not equate file and module cycles, even when identifiers are equal.
  return JSON.stringify([cycle.level, cycle.id]);
}

function cyclesComparison(a: RepositoryDna, b: RepositoryDna, metadata: boolean): ChangeGroup {
  // The combined list includes file cycles; treat file-edge truncation as a
  // possible inventory limitation rather than interpreting absence as repair.
  const available = architectureCoverage(a, b, metadata, true);
  available.notes.push("A cycle missing from the later analysis is not proof that the underlying source was repaired.");
  available.absenceComparable = false;
  return compareInventory<DependencyCycle>({
    current: a.architecture.cycles,
    baseline: b.architecture.cycles,
    coverage: available,
    key: cycleKey,
    label: (cycle) => `${cycle.level} cycle: ${cycle.members.join(", ")}`,
    summary: (cycle) => `${cycle.members.length} members · ${cycle.confidence} confidence`,
    details: (before, after) => {
      const details = observedFields(before, after, [["Confidence", (item) => item.confidence]]);
      if (!sameStrings(before.members, after.members)) details.push("Recorded component membership changed");
      if (!sameStrings(before.languages, after.languages)) details.push("Recorded languages changed");
      // A shortest witness may change even if the underlying SCC is identical.
      if (JSON.stringify(before.path) !== JSON.stringify(after.path)) details.push("Recorded cycle witness changed");
      return details;
    },
  });
}

/** Compare saved measurements; neither a source-code diff nor a Git ancestry test. */
export function compareAnalyses(current: RepositoryDna, baseline: RepositoryDna): AnalysisComparison {
  const metadata = comparableMetadata(current, baseline);
  return {
    files: filesComparison(current, baseline, metadata),
    dependencies: dependenciesComparison(current, baseline, metadata),
    languages: languagesComparison(current, baseline, metadata),
    modules: modulesComparison(current, baseline, metadata),
    moduleEdges: moduleEdgesComparison(current, baseline, metadata),
    cycles: cyclesComparison(current, baseline, metadata),
    findings: findingsComparison(current, baseline, metadata),
  };
}

export const CHANGE_LABELS: Record<ChangeKind, string> = {
  added: "Added",
  removed: "Removed",
  modified: "Changed",
  metadata: "Metadata changed",
  "possible-move": "Possible move",
  "only-current": "Only in current",
  "only-baseline": "Only in baseline",
  suppressed: "Suppressed",
  unsuppressed: "Unsuppressed",
};

export function filterAnalysisChanges(
  changes: readonly AnalysisChange[],
  query: string,
  kind: ChangeKind | "all",
): AnalysisChange[] {
  const needle = query.trim().toLowerCase();
  return changes.filter((change) => {
    if (kind !== "all" && change.kind !== kind) return false;
    if (!needle) return true;
    return [change.label, change.key, change.before, change.after, ...change.details]
      .some((value) => value.toLowerCase().includes(needle));
  });
}
