// Typed access to RepoDNA artifacts. The types are generated from the artifact's JSON
// Schema, which is itself generated from the Rust model, so they cannot drift apart.

import type {
  Confidence,
  Evidence,
  Finding,
  FindingCategory,
  RepositoryDna,
  Severity,
} from "./generated";

export type * from "./generated";

/** Major schema version this package reads. */
export const SCHEMA_MAJOR = 1;
/** Minor schema version this package was generated from. */
export const SCHEMA_MINOR = 0;

/** An artifact that could not be read. */
export class ArtifactError extends Error {
  override name = "ArtifactError";
}

/** A parsed artifact with compatibility notes. */
export interface LoadedArtifact {
  artifact: RepositoryDna;
  warnings: string[];
}

const REQUIRED_SECTIONS = ["tool", "identity", "analysisMetadata"] as const;

/** Checks a parsed JSON value the way the Rust reader does and returns it typed. */
export function checkArtifact(value: unknown): LoadedArtifact {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new ArtifactError("The document is not a JSON object.");
  }
  const record = value as Record<string, unknown>;
  const version = record["schemaVersion"];
  if (typeof version !== "string") {
    throw new ArtifactError("The document has no schemaVersion, so it is not a RepoDNA artifact.");
  }
  const match = /^(\d+)(?:\.(\d+))?$/.exec(version.trim());
  if (!match) {
    throw new ArtifactError(`The schema version "${version}" is not valid.`);
  }
  const major = Number(match[1]);
  const minor = Number(match[2] ?? "0");
  if (major !== SCHEMA_MAJOR) {
    throw new ArtifactError(
      major > SCHEMA_MAJOR
        ? `This artifact uses schema ${version}, written by a newer RepoDNA. Update RepoDNA to read it.`
        : `This artifact uses schema ${version}, which this version of RepoDNA cannot read.`,
    );
  }
  for (const section of REQUIRED_SECTIONS) {
    if (typeof record[section] !== "object" || record[section] === null) {
      throw new ArtifactError(`The artifact has no ${section} section.`);
    }
  }
  const warnings: string[] = [];
  if (minor > SCHEMA_MINOR) {
    warnings.push(
      `This artifact uses schema ${version}, written by a newer RepoDNA; information this version does not know about is not shown.`,
    );
  }
  return { artifact: value as RepositoryDna, warnings };
}

/** Parses artifact JSON text. */
export function parseArtifact(text: string): LoadedArtifact {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch (error) {
    throw new ArtifactError(`The file is not valid JSON: ${(error as Error).message}`);
  }
  return checkArtifact(value);
}

/** Severities from most to least urgent. */
export const SEVERITIES: readonly Severity[] = ["critical", "warning", "attention", "info"];

const SEVERITY_RANK: Record<Severity, number> = { info: 0, attention: 1, warning: 2, critical: 3 };

/** Numeric rank of a severity (higher is more urgent). */
export function severityRank(severity: Severity): number {
  return SEVERITY_RANK[severity];
}

/** Human-readable severity. */
export function severityLabel(severity: Severity): string {
  return severity === "info" ? "Info" : severity.charAt(0).toUpperCase() + severity.slice(1);
}

/** Human-readable confidence. */
export function confidenceLabel(confidence: Confidence): string {
  return confidence.charAt(0).toUpperCase() + confidence.slice(1);
}

const CATEGORY_LABELS: Record<FindingCategory, string> = {
  structure: "Structure",
  architecture: "Architecture",
  dependencies: "Dependencies",
  activity: "Activity",
  contributors: "Contributors",
  complexity: "Complexity",
  duplication: "Duplication",
  maintainability: "Maintainability",
  tests: "Tests",
  build: "Build",
  documentation: "Documentation",
  security: "Security",
  evolution: "Evolution",
  plugin: "Plugin",
};

/** Human-readable finding category. */
export function categoryLabel(category: FindingCategory): string {
  return CATEGORY_LABELS[category] ?? category;
}

/** `true` when a finding was suppressed by configuration. */
export function isSuppressed(finding: Finding): boolean {
  return finding.suppressed !== undefined && finding.suppressed !== null;
}

/** Active (unsuppressed) findings per severity, plus the suppressed count. */
export interface FindingCounts {
  critical: number;
  warning: number;
  attention: number;
  info: number;
  suppressed: number;
}

/** Counts findings like `RepositoryDna::finding_counts` in Rust. */
export function findingCounts(dna: RepositoryDna): FindingCounts {
  const counts: FindingCounts = { critical: 0, warning: 0, attention: 0, info: 0, suppressed: 0 };
  for (const finding of dna.findings ?? []) {
    if (isSuppressed(finding)) {
      counts.suppressed += 1;
    } else {
      counts[finding.severity] += 1;
    }
  }
  return counts;
}

function formatNumber(value: number): string {
  if (Number.isInteger(value)) {
    return String(value);
  }
  return String(Math.round(value * 100) / 100);
}

function withSuffix(text: string, suffix: string | null | undefined): string {
  return suffix ? `${text} — ${suffix}` : text;
}

/** A compact one-line description of evidence, matching the Rust `describe`. */
export function describeEvidence(evidence: Evidence): string {
  switch (evidence.kind) {
    case "file": {
      const { path, line, endLine, note } = evidence;
      let location = path;
      if (line != null && endLine != null && endLine > line) {
        location = `${path}:${line}-${endLine}`;
      } else if (line != null) {
        location = `${path}:${line}`;
      }
      return withSuffix(location, note);
    }
    case "directory":
      return withSuffix(
        `${evidence.path === "" ? "(repository root)" : evidence.path}/`,
        evidence.note,
      );
    case "symbol":
      return evidence.line != null
        ? `${evidence.name} (${evidence.path}:${evidence.line})`
        : `${evidence.name} (${evidence.path})`;
    case "commit": {
      let text = `commit ${evidence.hash.slice(0, 7)}`;
      if (evidence.date) {
        text += ` (${evidence.date.slice(0, 10)})`;
      }
      return withSuffix(text, evidence.summary);
    }
    case "dependency-edge": {
      const edge = `dependency edge: ${evidence.from} → ${evidence.to}`;
      if (evidence.path && evidence.line != null) {
        return `${edge} (${evidence.path}:${evidence.line})`;
      }
      return evidence.path ? `${edge} (${evidence.path})` : edge;
    }
    case "package": {
      let text = `${evidence.ecosystem} package ${evidence.name}`;
      if (evidence.version) {
        text += ` ${evidence.version}`;
      }
      if (evidence.manifest) {
        text += ` (${evidence.manifest})`;
      }
      return text;
    }
    case "metric": {
      const unit = evidence.unit ? ` ${evidence.unit}` : "";
      const value = `${evidence.metric} = ${formatNumber(evidence.value)}${unit}`;
      return evidence.threshold != null
        ? `${value} (threshold ${formatNumber(evidence.threshold)}${unit})`
        : value;
    }
    case "observation":
      return evidence.text;
  }
}

/** The repository path evidence refers to, if any. */
export function evidencePath(evidence: Evidence): string | undefined {
  switch (evidence.kind) {
    case "file":
    case "directory":
    case "symbol":
      return evidence.path;
    case "dependency-edge":
      return evidence.path ?? undefined;
    case "package":
      return evidence.manifest ?? undefined;
    default:
      return undefined;
  }
}
