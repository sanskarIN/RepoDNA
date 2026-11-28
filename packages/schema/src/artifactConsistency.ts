/** Cross-field checks complement schema validation: they verify internally
 * reported relationships, never the original repository or artifact provenance.
 * A capped/partial inventory is not treated as evidence of missing records. */
export interface ConsistencyIssue {
  path: string;
  code: string;
  message: string;
}
export interface ConsistencyReport {
  status: "consistent" | "observations" | "incomplete";
  issues: ConsistencyIssue[];
  observations: number;
  omittedIssues: number;
  checked: number;
  complete: boolean;
  skipped: string[];
}
interface Section {
  status: string;
}
interface Lines {
  total: number;
  code: number;
  comment: number;
  blank: number;
}
interface FileInventoryRecord {
  path: string;
  bytes: number;
  lines?: Lines | null;
  module?: string | null;
}
interface DirectoryInventoryRecord {
  path: string;
  depth: number;
  files: number;
  bytes: number;
  codeLines: number;
}
interface ModuleInventoryRecord {
  id: string;
  files: number;
  codeLines: number;
  fanIn: number;
  fanOut: number;
  instability: number;
  centrality: number;
  layer?: number | null;
}
interface ModuleInventoryEdge {
  from: string;
  to: string;
  weight: number;
}
interface FileInventoryEdge {
  from: string;
  to: string;
  line: number;
}
interface CycleInventoryRecord {
  id: string;
  level: string;
  members: readonly string[];
  path: readonly string[];
}
/** Minimum typed projection of the actual generated artifact model. */
export interface ConsistencyArtifact {
  schemaVersion: string;
  tool: { schemaVersion: string };
  analysisMetadata: { partial: boolean; suppressedFindings: number };
  structure: Section & {
    truncated?: boolean;
    files: readonly FileInventoryRecord[];
    directories: readonly DirectoryInventoryRecord[];
    totalFiles: number;
    totalBytes: number;
    totalLines: number;
    codeLines: number;
    commentLines: number;
    blankLines: number;
    categories: readonly { category: string; files: number; bytes: number; lines: number }[];
  };
  languages: Section & {
    languages: readonly { id: string; share: number; files: number }[];
    primary: readonly string[];
  };
  architecture: Section & {
    modules: readonly ModuleInventoryRecord[];
    moduleEdges: readonly ModuleInventoryEdge[];
    fileEdges: readonly FileInventoryEdge[];
    fileEdgesTruncated?: boolean;
    cycles: readonly CycleInventoryRecord[];
    layers: readonly { index: number; modules: readonly string[] }[];
    isolatedModules: readonly string[];
  };
  dependencies: Section & {
    directCount: number;
    dependencies: readonly { ecosystem: string; name: string; scope: string; source: string; manifests: readonly string[]; resolved?: readonly string[] }[];
    concentration: readonly { share: number }[];
  };
  codeQuality: Section & {
    duplication: Section & { analyzedLines: number; duplicatedLines: number; ratio: number; clusters: readonly { occurrenceCount: number; occurrences: readonly unknown[] }[] };
  };
  similarity: Section & { similarFiles: readonly { a: string; b: string; similarity: number }[] };
  tests: Section & { testRatio: number };
  git: Section & { commitCount: number; ownership: { contributors: number; contributorsForHalfOfCommits: number; topContributorShare: number } };
  findings: readonly { id: string; suppressed?: unknown }[];
}
export interface ConsistencyOptions {
  signal?: AbortSignal;
  /** Cap total work even for an artifact loaded in memory rather than a file. */
  maxSteps?: number;
  maxIssues?: number;
  batch?: number;
  yieldControl?: () => Promise<void>;
}
const DEFAULT_STEPS = 2_000_000;
const DEFAULT_ISSUES = 100;
const DEFAULT_BATCH = 1_000;

function safe(value: number): boolean {
  return Number.isSafeInteger(value) && value >= 0;
}
function safeSum(values: readonly number[]): number | null {
  let total = 0;
  for (const value of values) {
    if (!safe(value)) return null;
    total += value;
    if (!safe(total)) return null;
  }
  return total;
}
function differs(expected: number | null, actual: number): boolean {
  return expected !== null && safe(actual) && expected !== actual;
}
function ratio(value: number): boolean {
  return Number.isFinite(value) && value >= 0 && value <= 1;
}
function close(a: number, b: number): boolean {
  return Math.abs(a - b) <= 1e-9;
}
function pathValid(path: string, rootAllowed = false): boolean {
  if (path === "") return rootAllowed;
  return !path.includes("\\") && !path.includes("\0") && !/^[A-Za-z]:/.test(path) &&
    path.split("/").every((part) => part !== "" && part !== "." && part !== "..");
}
function sameVersion(a: string, b: string): boolean {
  const parts = (value: string) => value.trim().split(".");
  const [am, an = "0"] = parts(a);
  const [bm, bn = "0"] = parts(b);
  return am === bm && an === bn;
}
function edgeKey(from: string, to: string): string {
  return JSON.stringify([from, to]);
}
interface CheckSink {
  note(path: string, code: string, message: string): void;
  skip(reason: string): void;
}

function* verifyStructure(dna: ConsistencyArtifact, sink: CheckSink): Generator<void, Set<string>> {
  const structure = dna.structure;
  const filePaths = new Set<string>();
  const complete = structure.status === "analyzed" && !structure.truncated;
  if (!complete) sink.skip("Exact inventory totals and missing-file checks require a complete structure section.");
  if (differs(safeSum([structure.codeLines, structure.commentLines, structure.blankLines]), structure.totalLines)) {
    sink.note("/structure/totalLines", "line-total", "Code, comment and blank line totals do not add up to the reported total.");
  }
  let bytes: number | null = 0;
  let code: number | null = 0;
  let comments: number | null = 0;
  let blanks: number | null = 0;
  let lines: number | null = 0;
  const add = (total: number | null, next: number) => total === null ? null : safeSum([total, next]);
  for (const [i, file] of structure.files.entries()) {
    yield;
    const base = `/structure/files/${i}`;
    if (!pathValid(file.path)) sink.note(`${base}/path`, "file-path", "The file path is not a normalized repository-relative path.");
    if (filePaths.has(file.path)) sink.note(`${base}/path`, "duplicate-file", "Another file record uses the same path.");
    filePaths.add(file.path);
    bytes = add(bytes, file.bytes);
    if (file.lines) {
      const record = file.lines;
      if (differs(safeSum([record.code, record.comment, record.blank]), record.total)) {
        sink.note(`${base}/lines/total`, "line-total", "File line categories do not add up to its total.");
      }
      code = add(code, record.code);
      comments = add(comments, record.comment);
      blanks = add(blanks, record.blank);
      lines = add(lines, record.total);
    }
  }
  if (complete) {
    const totals: [string, number | null, number][] = [
      ["totalFiles", structure.files.length, structure.totalFiles],
      ["totalBytes", bytes, structure.totalBytes],
      ["totalLines", lines, structure.totalLines],
      ["codeLines", code, structure.codeLines],
      ["commentLines", comments, structure.commentLines],
      ["blankLines", blanks, structure.blankLines],
    ];
    for (const [field, actual, declared] of totals) {
      yield;
      if (actual === null) sink.skip("Some exact totals exceed JavaScript's safe integer range.");
      else if (differs(actual, declared)) sink.note(`/structure/${field}`, "inventory-total", "The reported total differs from the complete file inventory.");
    }
  }
  const directories = new Set<string>();
  for (const [i, directory] of structure.directories.entries()) {
    yield;
    const base = `/structure/directories/${i}`;
    if (directories.has(directory.path)) sink.note(`${base}/path`, "duplicate-directory", "Another directory record uses the same path.");
    directories.add(directory.path);
    if (!pathValid(directory.path, true)) sink.note(`${base}/path`, "directory-path", "The directory path is not normalized and repository-relative.");
    else if (directory.depth !== (directory.path ? directory.path.split("/").length : 0)) sink.note(`${base}/depth`, "directory-depth", "The reported directory depth differs from its path.");
    if (directory.path === "" && complete) {
      if (differs(directory.files, structure.totalFiles) || differs(directory.bytes, structure.totalBytes) || differs(directory.codeLines, structure.codeLines)) {
        sink.note(base, "root-directory-total", "Root directory totals differ from the structure totals.");
      }
    }
  }
  const categories = new Set<string>();
  let categoryFiles: number | null = 0;
  let categoryBytes: number | null = 0;
  let categoryLines: number | null = 0;
  for (const [i, category] of structure.categories.entries()) {
    yield;
    if (categories.has(category.category)) sink.note(`/structure/categories/${i}/category`, "duplicate-category", "The category is listed more than once.");
    categories.add(category.category);
    categoryFiles = add(categoryFiles, category.files);
    categoryBytes = add(categoryBytes, category.bytes);
    categoryLines = add(categoryLines, category.lines);
  }
  if (complete && (differs(categoryFiles, structure.totalFiles) || differs(categoryBytes, structure.totalBytes) || differs(categoryLines, structure.totalLines))) {
    sink.note("/structure/categories", "category-total", "Category totals differ from the complete structure totals.");
  }
  return filePaths;
}

function* verifyLanguages(dna: ConsistencyArtifact, sink: CheckSink): Generator<void> {
  const ids = new Set<string>();
  for (const [i, language] of dna.languages.languages.entries()) {
    yield;
    const path = `/languages/languages/${i}`;
    if (ids.has(language.id)) sink.note(`${path}/id`, "duplicate-language", "Another language record uses the same identifier.");
    ids.add(language.id);
    if (!ratio(language.share)) sink.note(`${path}/share`, "ratio-range", "The language share must be between zero and one.");
  }
  const primary = new Set<string>();
  for (const [i, id] of dna.languages.primary.entries()) {
    yield;
    const path = `/languages/primary/${i}`;
    if (primary.has(id)) sink.note(path, "duplicate-primary-language", "The primary language is listed more than once.");
    primary.add(id);
    if (dna.languages.status === "analyzed" && !ids.has(id)) sink.note(path, "unknown-language", "A primary language does not occur in the language inventory.");
  }
}

function* verifyArchitecture(dna: ConsistencyArtifact, sink: CheckSink, files: Set<string>): Generator<void> {
  const architecture = dna.architecture;
  const complete = architecture.status === "analyzed";
  const completeFiles = dna.structure.status === "analyzed" && !dna.structure.truncated;
  if (!complete) sink.skip("Missing module references and exact graph degree checks require complete architecture analysis.");
  const modules = new Map<string, ModuleInventoryRecord>();
  for (const [i, module] of architecture.modules.entries()) {
    yield;
    const path = `/architecture/modules/${i}`;
    if (modules.has(module.id)) sink.note(`${path}/id`, "duplicate-module", "Another module uses the same identifier.");
    modules.set(module.id, module);
    for (const [field, value] of [["instability", module.instability], ["centrality", module.centrality]] as const) {
      if (!ratio(value)) sink.note(`${path}/${field}`, "ratio-range", "The normalized measure must be between zero and one.");
    }
    const degree = safeSum([module.fanIn, module.fanOut]);
    if (degree !== null && degree > 0 && !close(module.instability, module.fanOut / degree)) sink.note(`${path}/instability`, "instability", "The instability does not match fan-out divided by total degree.");
  }
  const edges = new Set<string>();
  const incoming = new Map<string, Set<string>>();
  const outgoing = new Map<string, Set<string>>();
  const add = (index: Map<string, Set<string>>, from: string, to: string) => {
    const targets = index.get(from) ?? new Set<string>();
    targets.add(to);
    index.set(from, targets);
  };
  for (const [i, edge] of architecture.moduleEdges.entries()) {
    yield;
    const path = `/architecture/moduleEdges/${i}`;
    const key = edgeKey(edge.from, edge.to);
    if (edges.has(key)) sink.note(path, "duplicate-module-edge", "The same directed module edge occurs more than once.");
    edges.add(key);
    if (edge.weight === 0) sink.note(`${path}/weight`, "empty-edge", "A module edge reports no supporting file edges.");
    if (complete && (!modules.has(edge.from) || !modules.has(edge.to))) sink.note(path, "unknown-module-endpoint", "A module edge refers to a module absent from the analyzed graph.");
    add(outgoing, edge.from, edge.to);
    add(incoming, edge.to, edge.from);
  }
  if (complete) for (const [i, module] of architecture.modules.entries()) {
    yield;
    if (module.fanIn !== (incoming.get(module.id)?.size ?? 0) || module.fanOut !== (outgoing.get(module.id)?.size ?? 0)) {
      sink.note(`/architecture/modules/${i}`, "module-degree", "The recorded fan-in or fan-out differs from the module edge list.");
    }
  }
  const fileEdges = new Set<string>();
  for (const [i, edge] of architecture.fileEdges.entries()) {
    yield;
    const path = `/architecture/fileEdges/${i}`;
    fileEdges.add(edgeKey(edge.from, edge.to));
    if (completeFiles && (!files.has(edge.from) || !files.has(edge.to))) sink.note(path, "unknown-file-endpoint", "A file edge refers to a file absent from the complete inventory.");
    if (edge.line === 0) sink.note(`${path}/line`, "invalid-line", "A declaration line should be one-based.");
  }
  const layerByModule = new Map<string, number>();
  const layers = new Set<number>();
  for (const [i, layer] of architecture.layers.entries()) {
    yield;
    if (layers.has(layer.index)) sink.note(`/architecture/layers/${i}/index`, "duplicate-layer", "Another layer has the same index.");
    layers.add(layer.index);
    for (const [j, id] of layer.modules.entries()) {
      yield;
      const path = `/architecture/layers/${i}/modules/${j}`;
      if (layerByModule.has(id)) sink.note(path, "duplicate-layer-module", "A module occurs in multiple layer entries.");
      layerByModule.set(id, layer.index);
      if (complete && !modules.has(id)) sink.note(path, "unknown-layer-module", "A layer refers to a module absent from the analyzed graph.");
    }
  }
  for (const [i, module] of architecture.modules.entries()) {
    yield;
    const assigned = layerByModule.get(module.id);
    if (module.layer != null && assigned !== undefined && module.layer !== assigned) sink.note(`/architecture/modules/${i}/layer`, "module-layer", "The module's reported layer differs from its layer membership.");
  }
  const isolated = new Set<string>();
  for (const [i, id] of architecture.isolatedModules.entries()) {
    yield;
    const path = `/architecture/isolatedModules/${i}`;
    if (isolated.has(id)) sink.note(path, "duplicate-isolated-module", "An isolated module is listed more than once.");
    isolated.add(id);
    if (complete && !modules.has(id)) sink.note(path, "unknown-isolated-module", "An isolated module is absent from the module inventory.");
    if (complete && ((incoming.get(id)?.size ?? 0) || (outgoing.get(id)?.size ?? 0))) sink.note(path, "isolated-module-edge", "A supposedly isolated module has an incoming or outgoing edge.");
  }
  const cycleIds = new Set<string>();
  for (const [i, cycle] of architecture.cycles.entries()) {
    yield;
    const base = `/architecture/cycles/${i}`;
    if (cycleIds.has(cycle.id)) sink.note(`${base}/id`, "duplicate-cycle", "Another cycle uses the same identifier.");
    cycleIds.add(cycle.id);
    const members = new Set<string>();
    const moduleLevel = cycle.level === "module";
    const checkInventory = moduleLevel ? complete : completeFiles;
    const inventory = moduleLevel ? modules : files;
    for (const [j, id] of cycle.members.entries()) {
      yield;
      if (members.has(id)) sink.note(`${base}/members/${j}`, "duplicate-cycle-member", "A cycle member is listed more than once.");
      members.add(id);
      if (checkInventory && !inventory.has(id)) sink.note(`${base}/members/${j}`, "unknown-cycle-member", "The cycle member is absent from the corresponding complete inventory.");
    }
    if (cycle.path.length < 2 || cycle.path[0] !== cycle.path.at(-1)) sink.note(`${base}/path`, "open-cycle", "The cycle witness does not begin and end at the same node.");
    for (const [j, id] of cycle.path.entries()) {
      yield;
      if (!members.has(id)) sink.note(`${base}/path/${j}`, "cycle-witness-member", "A cycle witness node is absent from its member list.");
      const previous = cycle.path[j - 1];
      if (previous === undefined) continue;
      // File edge lists can be explicitly capped. A missing edge in such a
      // list is not proof that the cycle witness is invalid.
      if (moduleLevel ? complete : complete && !architecture.fileEdgesTruncated) {
        if (!(moduleLevel ? edges : fileEdges).has(edgeKey(previous, id))) sink.note(`${base}/path/${j}`, "cycle-witness-edge", "The witness step is absent from the corresponding complete edge list.");
      }
    }
  }
  if (architecture.fileEdgesTruncated) sink.skip("File cycle witness edges cannot be fully checked against a truncated edge list.");
}

function* verifyOtherSections(dna: ConsistencyArtifact, sink: CheckSink): Generator<void> {
  if (!sameVersion(dna.schemaVersion, dna.tool.schemaVersion)) sink.note("/tool/schemaVersion", "schema-version", "Tool metadata and the root declare different schema versions.");
  const dependencyKeys = new Set<string>();
  for (const [i, dependency] of dna.dependencies.dependencies.entries()) {
    yield;
    const path = `/dependencies/dependencies/${i}`;
    // Different scopes or sources may legitimately describe one package.
    const key = JSON.stringify([dependency.ecosystem, dependency.name, dependency.scope, dependency.source]);
    if (dependencyKeys.has(key)) sink.note(path, "duplicate-dependency", "Another dependency has the same ecosystem, name, scope and source.");
    dependencyKeys.add(key);
    for (const [field, values] of [["manifests", dependency.manifests], ["resolved", dependency.resolved ?? []]] as const) {
      const seen = new Set<string>();
      for (const [j, value] of values.entries()) {
        yield;
        if (seen.has(value)) sink.note(`${path}/${field}/${j}`, "duplicate-dependency-value", "The same value occurs more than once in this dependency list.");
        seen.add(value);
      }
    }
  }
  for (const [i, concentration] of dna.dependencies.concentration.entries()) {
    yield;
    if (!ratio(concentration.share)) sink.note(`/dependencies/concentration/${i}/share`, "ratio-range", "The import share must be between zero and one.");
  }
  const duplication = dna.codeQuality.duplication;
  if (!ratio(duplication.ratio)) sink.note("/codeQuality/duplication/ratio", "ratio-range", "The duplication ratio must be between zero and one.");
  if (duplication.status === "analyzed" && safe(duplication.analyzedLines) && safe(duplication.duplicatedLines)) {
    if (duplication.duplicatedLines > duplication.analyzedLines) sink.note("/codeQuality/duplication/duplicatedLines", "duplication-count", "Duplicated lines exceed the analyzed lines.");
    if (duplication.analyzedLines > 0 && !close(duplication.ratio, duplication.duplicatedLines / duplication.analyzedLines)) sink.note("/codeQuality/duplication/ratio", "duplication-ratio", "The ratio differs from duplicated lines divided by analyzed lines.");
  }
  for (const [i, cluster] of duplication.clusters.entries()) {
    yield;
    if (safe(cluster.occurrenceCount) && cluster.occurrences.length > cluster.occurrenceCount) sink.note(`/codeQuality/duplication/clusters/${i}/occurrenceCount`, "occurrence-count", "The retained occurrences exceed the reported total occurrence count.");
  }
  const pairs = new Set<string>();
  for (const [i, pair] of dna.similarity.similarFiles.entries()) {
    yield;
    const path = `/similarity/similarFiles/${i}`;
    if (pair.a === pair.b) sink.note(path, "self-similarity", "A file is compared with itself.");
    const key = pair.a < pair.b ? edgeKey(pair.a, pair.b) : edgeKey(pair.b, pair.a);
    if (pairs.has(key)) sink.note(path, "duplicate-similarity", "Another similarity record compares the same unordered pair.");
    pairs.add(key);
    if (!ratio(pair.similarity)) sink.note(`${path}/similarity`, "ratio-range", "The similarity must be between zero and one.");
  }
  if (!ratio(dna.tests.testRatio)) sink.note("/tests/testRatio", "ratio-range", "The test-line share must be between zero and one.");
  const { ownership, commitCount } = dna.git;
  if (!ratio(ownership.topContributorShare)) sink.note("/git/ownership/topContributorShare", "ratio-range", "The contributor share must be between zero and one.");
  if (dna.git.status === "analyzed" && safe(ownership.contributors) && safe(commitCount) && ownership.contributors > commitCount) sink.note("/git/ownership/contributors", "contributor-count", "The number of contributors exceeds the analyzed commits.");
  if (safe(ownership.contributors) && safe(ownership.contributorsForHalfOfCommits) && ownership.contributorsForHalfOfCommits > ownership.contributors) sink.note("/git/ownership/contributorsForHalfOfCommits", "contributor-count", "The subset of contributors exceeds the total contributor count.");
  let suppressed = 0;
  for (const finding of dna.findings) {
    yield;
    if (finding.suppressed != null) suppressed += 1;
  }
  if (!dna.analysisMetadata.partial && differs(suppressed, dna.analysisMetadata.suppressedFindings)) sink.note("/analysisMetadata/suppressedFindings", "suppression-count", "The reported suppression total differs from the findings marked suppressed.");
}

function* verify(dna: ConsistencyArtifact, sink: CheckSink): Generator<void> {
  const files = yield* verifyStructure(dna, sink);
  yield* verifyLanguages(dna, sink);
  yield* verifyArchitecture(dna, sink, files);
  yield* verifyOtherSections(dna, sink);
}
function limit(value: number | undefined, fallback: number, max: number): number {
  if (value === undefined) return fallback;
  if (!Number.isSafeInteger(value) || value < 1 || value > max) throw new RangeError("Invalid consistency-check limit.");
  return value;
}
function cancelled(signal?: AbortSignal): void {
  if (signal?.aborted) throw new DOMException("Consistency check cancelled.", "AbortError");
}

/** Run cross-field diagnostics only on a document already validated against the
 * matching schema. Observations are not proof of corruption; skip checks when
 * the corresponding section is partial. The iterator yields between records. */
export async function inspectConsistency(dna: ConsistencyArtifact, options: ConsistencyOptions = {}): Promise<ConsistencyReport> {
  const maxSteps = limit(options.maxSteps, DEFAULT_STEPS, 10_000_000);
  const maxIssues = limit(options.maxIssues, DEFAULT_ISSUES, 10_000);
  const batch = limit(options.batch, DEFAULT_BATCH, 100_000);
  const pause = options.yieldControl ?? (() => new Promise<void>((resolve) => setTimeout(resolve, 0)));
  const issues: ConsistencyIssue[] = [];
  const skipped = new Set<string>();
  let observations = 0;
  let checked = 0;
  let complete = false;
  const iterator = verify(dna, {
    note(path, code, message) {
      observations += 1;
      if (issues.length < maxIssues) issues.push({ path, code, message });
    },
    skip(reason) { skipped.add(reason); },
  });
  try {
    while (checked < maxSteps) {
      cancelled(options.signal);
      const next = iterator.next();
      if (next.done) { complete = true; break; }
      checked += 1;
      if (checked % batch === 0) await pause();
    }
    cancelled(options.signal);
  } finally {
    iterator.return?.(undefined);
  }
  if (!complete) skipped.add("The consistency checker reached its work limit.");
  return {
    status: !complete ? "incomplete" : observations ? "observations" : "consistent",
    issues,
    observations,
    omittedIssues: observations - issues.length,
    checked,
    complete,
    skipped: [...skipped],
  };
}
