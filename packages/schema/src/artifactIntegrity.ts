import artifactSchema from "../../../schemas/repodna-artifact.schema.json";
import {
  compileShape,
  validateShapeAsync,
  type AsyncShapeOptions,
  type CompiledShape,
  type ShapeReport,
  type ShapeSchema,
} from "./schemaValidation";

import { inspectConsistency, type ConsistencyArtifact, type ConsistencyReport } from "./artifactConsistency";

export type { ConsistencyIssue, ConsistencyReport } from "./artifactConsistency";
export type { ShapeIssue, ShapeReport, ShapeLimits } from "./schemaValidation";

export const INTEGRITY_SCHEMA_VERSION = "1.0";
export const MAX_INTEGRITY_FILE_BYTES = 64 * 1024 * 1024;

let compiled: CompiledShape | null = null;

/** Lazily compile the exact checked-in Rust-generated artifact schema. */
export function artifactShape(): CompiledShape {
  compiled ??= compileShape(artifactSchema as unknown as ShapeSchema);
  return compiled;
}

export interface ArtifactIntegrityReport extends ShapeReport {
  /** The input claims this version; null when no readable version was supplied. */
  artifactVersion: string | null;
  checkedAgainst: string;
  consistency: ConsistencyReport | null;
  compatibility: "same-version" | "different-minor" | "different-major" | "unknown";
}

function claimedVersion(value: unknown): string | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const version = (value as Record<string, unknown>).schemaVersion;
  return typeof version === "string" && /^\d{1,6}(?:\.\d{1,6})?$/.test(version.trim()) ? version.trim() : null;
}

function compatibility(version: string | null): ArtifactIntegrityReport["compatibility"] {
  if (!version) return "unknown";
  const [major, minor = "0"] = version.split(".");
  if (Number(major) !== 1) return "different-major";
  return Number(minor) === 0 ? "same-version" : "different-minor";
}

/** Check the structure of the document without rewriting it, filling default
 * sections, or silently upgrading its schema version. This is diagnostic only;
 * the existing tolerant artifact-loading API remains unchanged.
 */
export async function inspectArtifact(value: unknown, options?: AsyncShapeOptions): Promise<ArtifactIntegrityReport> {
  const artifactVersion = claimedVersion(value);
  const report = await validateShapeAsync(artifactShape(), value, options);
  const result: ArtifactIntegrityReport = {
    ...report,
    artifactVersion,
    checkedAgainst: INTEGRITY_SCHEMA_VERSION,
    consistency: null,
    compatibility: compatibility(artifactVersion),
  };
  // A shape match on a different/unknown version is not proof of conformance
  // to that version's schema. Preserve the concrete issues but qualify status.
  if (result.compatibility !== "same-version" && result.status === "valid") result.status = "incomplete";
  if (result.status === "valid" && result.complete) {
    result.consistency = await inspectConsistency(value as ConsistencyArtifact, { signal: options?.signal });
  }
  return result;
}

/** Read a bounded file locally; errors never include the contents of its JSON. */
export async function inspectArtifactFile(file: Blob, options?: AsyncShapeOptions): Promise<ArtifactIntegrityReport> {
  if (file.size > MAX_INTEGRITY_FILE_BYTES) throw new Error("The integrity checker supports files up to 64 MiB. The normal analysis loader has a separate limit.");
  if (options?.signal?.aborted) throw new DOMException("Integrity check cancelled.", "AbortError");
  const text = await file.text();
  if (options?.signal?.aborted) throw new DOMException("Integrity check cancelled.", "AbortError");
  let value: unknown;
  try { value = JSON.parse(text); }
  catch { throw new Error("The selected document is not valid JSON."); }
  return inspectArtifact(value, options);
}

/** Export only validation paths and diagnostic descriptions, not analysis data. */
export function integrityReportJson(report: ArtifactIntegrityReport): string {
  return JSON.stringify({
    reportFormat: "repodna-artifact-integrity-1",
    ...report,
  }, null, 2) + "\n";
}
