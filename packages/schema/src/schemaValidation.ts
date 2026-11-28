/** A deliberately bounded validator for the JSON Schema subset emitted by RepoDNA's
 * Rust model generator. This is not a general-purpose draft 2020-12 implementation.
 * Unknown instance properties are allowed unless the schema explicitly forbids them.
 */
export type SchemaType = "null" | "boolean" | "object" | "array" | "number" | "integer" | "string";
export type ShapeSchema = boolean | SchemaNode;
export interface SchemaNode {
  $schema?: string;
  $id?: string;
  $defs?: Record<string, ShapeSchema>;
  $ref?: string;
  title?: string;
  description?: string;
  default?: unknown;
  type?: SchemaType | SchemaType[];
  properties?: Record<string, ShapeSchema>;
  required?: string[];
  additionalProperties?: ShapeSchema;
  items?: ShapeSchema;
  oneOf?: ShapeSchema[];
  anyOf?: ShapeSchema[];
  const?: unknown;
  minimum?: number;
  format?: string;
}

export type ShapeIssueCode =
  | "type"
  | "required"
  | "additional-property"
  | "constant"
  | "minimum"
  | "format"
  | "precision"
  | "union"
  | "budget"
  | "cycle";

export interface ShapeIssue {
  /** RFC 6901 JSON pointer, empty at the root. No instance value is included. */
  path: string;
  code: ShapeIssueCode;
  severity: "error" | "warning";
  message: string;
}

export interface ShapeReport {
  status: "valid" | "invalid" | "incomplete";
  issues: ShapeIssue[];
  errors: number;
  warnings: number;
  checked: number;
  /** The issues list is a sample when more issues exist than its configured cap. */
  omittedIssues: number;
  /** Resource exhaustion means a clean-looking prefix cannot be called valid. */
  complete: boolean;
}

export interface ShapeLimits {
  nodes: number;
  depth: number;
  issues: number;
  /** Number of validation operations between cooperative yields. */
  batch: number;
}

export const DEFAULT_SHAPE_LIMITS: Readonly<ShapeLimits> = {
  nodes: 5_000_000,
  depth: 128,
  issues: 100,
  batch: 2_000,
};

export class UnsupportedShapeSchema extends Error {
  override name = "UnsupportedShapeSchema";
}

const TYPES = new Set<SchemaType>(["null", "boolean", "object", "array", "number", "integer", "string"]);
const KEYWORDS = new Set([
  "$schema", "$id", "$defs", "$ref", "title", "description", "default", "type", "properties",
  "required", "additionalProperties", "items", "oneOf", "anyOf", "const", "minimum", "format",
]);
const FORMATS = new Set(["date-time", "uint64", "uint32", "int64", "int32", "double"]);

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function own(value: object, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(value, key);
}

function escapePointer(key: string): string {
  return key.replaceAll("~", "~0").replaceAll("/", "~1");
}

function child(path: string, key: string | number): string {
  // A single arbitrary key may be very large. Never emit its content in full.
  const component = String(key);
  return `${path}/${escapePointer(component.length > 512 ? component.slice(0, 512) + "…" : component)}`;
}

/** Resolve only bundled document-local references; never fetch remote schemas. */
function resolveRef(root: ShapeSchema, reference: string): ShapeSchema {
  if (reference === "#") return root;
  if (!reference.startsWith("#/")) throw new UnsupportedShapeSchema("External schema references are not supported.");
  let target: unknown = root;
  for (const encoded of reference.slice(2).split("/")) {
    let key: string;
    try { key = decodeURIComponent(encoded).replaceAll("~1", "/").replaceAll("~0", "~"); }
    catch { throw new UnsupportedShapeSchema("Malformed local schema reference."); }
    if (!record(target) || !own(target, key)) throw new UnsupportedShapeSchema("Unresolved local schema reference.");
    target = target[key];
  }
  if (typeof target !== "boolean" && !record(target)) throw new UnsupportedShapeSchema("Reference does not point to a schema.");
  return target as ShapeSchema;
}

export interface CompiledShape {
  readonly root: ShapeSchema;
  readonly references: ReadonlyMap<string, ShapeSchema>;
}

/** Fail explicitly if the generated schema starts using an unsupported keyword.
 * This prevents a later schema change from silently weakening the integrity check.
 */
export function compileShape(root: ShapeSchema): CompiledShape {
  const references = new Map<string, ShapeSchema>();
  const visited = new Set<SchemaNode>();
  const stack: ShapeSchema[] = [root];
  while (stack.length) {
    const schema = stack.pop()!;
    if (typeof schema === "boolean") continue;
    if (!schema || typeof schema !== "object" || Array.isArray(schema)) throw new UnsupportedShapeSchema("Expected a schema object or boolean.");
    if (visited.has(schema)) continue;
    visited.add(schema);
    if (visited.size > 100_000) throw new UnsupportedShapeSchema("The schema exceeds the compilation budget.");
    for (const key of Object.keys(schema)) {
      if (!KEYWORDS.has(key)) throw new UnsupportedShapeSchema(`Unsupported schema keyword: ${key}.`);
    }
    if (schema.type !== undefined) {
      const values = Array.isArray(schema.type) ? schema.type : [schema.type];
      if (!values.length || new Set(values).size !== values.length || values.some((type) => !TYPES.has(type))) {
        throw new UnsupportedShapeSchema("Invalid schema type declaration.");
      }
    }
    if (schema.format !== undefined && !FORMATS.has(schema.format)) throw new UnsupportedShapeSchema("Unsupported schema format.");
    if (schema.minimum !== undefined && !Number.isFinite(schema.minimum)) throw new UnsupportedShapeSchema("Invalid minimum.");
    if (schema.required !== undefined && (!Array.isArray(schema.required) || schema.required.some((key) => typeof key !== "string"))) {
      throw new UnsupportedShapeSchema("Invalid required properties.");
    }
    if (schema.$ref !== undefined) {
      if (typeof schema.$ref !== "string") throw new UnsupportedShapeSchema("Invalid local schema reference.");
      const target = resolveRef(root, schema.$ref);
      references.set(schema.$ref, target);
      stack.push(target);
    }
    for (const map of [schema.properties, schema.$defs]) {
      if (map !== undefined) {
        if (!map || typeof map !== "object" || Array.isArray(map)) throw new UnsupportedShapeSchema("Invalid schema properties or definitions.");
        stack.push(...Object.values(map));
      }
    }
    for (const list of [schema.oneOf, schema.anyOf]) {
      if (list !== undefined) {
        if (!Array.isArray(list) || !list.length) throw new UnsupportedShapeSchema("Empty or invalid schema union.");
        stack.push(...list);
      }
    }
    if (schema.items !== undefined) stack.push(schema.items);
    if (schema.additionalProperties !== undefined) stack.push(schema.additionalProperties);
  }
  return { root, references };
}

function matchesType(value: unknown, type: SchemaType): boolean {
  switch (type) {
    case "null": return value === null;
    case "boolean": return typeof value === "boolean";
    case "string": return typeof value === "string";
    case "object": return record(value);
    case "array": return Array.isArray(value);
    case "number": return typeof value === "number" && Number.isFinite(value);
    case "integer": return typeof value === "number" && Number.isInteger(value);
  }
}

function sameConstant(a: unknown, b: unknown): boolean {
  // The generated schema uses scalar constants for discriminators and enum values.
  if (a === b) return true;
  if (Array.isArray(a) && Array.isArray(b)) return a.length === b.length && a.every((item, index) => sameConstant(item, b[index]));
  if (record(a) && record(b)) {
    const keys = Object.keys(a);
    return keys.length === Object.keys(b).length && keys.every((key) => own(b, key) && sameConstant(a[key], b[key]));
  }
  return false;
}

function leap(year: number): boolean { return year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0); }

/** RFC 3339 lexical/calendar check, including fractional seconds and UTC offsets.
 * Leap-second notation is permitted; this is not a historical leap-second table.
 */
export function validTimestamp(value: string): boolean {
  const match = /^(\d{4})-(\d{2})-(\d{2})[Tt](\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(?:[Zz]|([+-])(\d{2}):(\d{2}))$/.exec(value);
  if (!match) return false;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  const hour = Number(match[4]);
  const minute = Number(match[5]);
  const second = Number(match[6]);
  const days = [31, leap(year) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  return month >= 1 && month <= 12 && day >= 1 && day <= days[month - 1]! &&
    hour <= 23 && minute <= 59 && second <= 60 &&
    (!match[7] || (Number(match[8]) <= 23 && Number(match[9]) <= 59));
}

interface Sink {
  errors: number;
  warnings: number;
  issues: ShapeIssue[];
  max: number;
}

interface Context {
  shape: CompiledShape;
  limits: Readonly<ShapeLimits>;
  checked: number;
  exhausted: boolean;
  activeRefs: Map<string, Set<ShapeSchema>>;
  signal?: AbortSignal;
}

function sink(max: number): Sink { return { errors: 0, warnings: 0, issues: [], max }; }

function issue(sink: Sink, path: string, code: ShapeIssueCode, message: string, severity: ShapeIssue["severity"] = "error"): void {
  if (severity === "error") sink.errors++;
  else sink.warnings++;
  if (sink.issues.length < sink.max) sink.issues.push({ path, code, message, severity });
}

function merge(target: Sink, source: Sink): void {
  target.errors += source.errors;
  target.warnings += source.warnings;
  target.issues.push(...source.issues.slice(0, Math.max(0, target.max - target.issues.length)));
}

function abortIfNeeded(context: Context): void {
  if (context.signal?.aborted) throw new DOMException("Integrity check cancelled.", "AbortError");
}

function* tick(context: Context, result: Sink, path: string, depth: number): Generator<void, boolean> {
  abortIfNeeded(context);
  if (context.exhausted) return false;
  if (++context.checked > context.limits.nodes || depth > context.limits.depth) {
    context.exhausted = true;
    issue(result, path, "budget", "The integrity check reached its resource limit. Remaining fields were not checked.", "warning");
    return false;
  }
  if (context.checked % context.limits.batch === 0) yield;
  return true;
}

function checkFormat(value: unknown, format: string, result: Sink, path: string): void {
  if (format === "date-time") {
    if (typeof value === "string" && !validTimestamp(value)) issue(result, path, "format", "Expected an RFC 3339 timestamp.");
    return;
  }
  if (typeof value !== "number") return;
  const ranges: Record<string, [number, number]> = {
    uint32: [0, 4_294_967_295], int32: [-2_147_483_648, 2_147_483_647],
    uint64: [0, 2 ** 64], int64: [-(2 ** 63), 2 ** 63],
  };
  const range = ranges[format];
  if (!range) return;
  // JSON parsing rounds integers above 2^53. At the 64-bit upper boundary it
  // may round the largest legal integer to 2^64/2^63; do not call that invalid.
  if (value < range[0] || value > range[1] || !Number.isInteger(value)) {
    issue(result, path, "format", `Number outside the ${format} representation.`);
  } else if (!Number.isSafeInteger(value)) {
    issue(result, path, "precision", "This integer is outside JavaScript's exact integer range; exact 64-bit validation is unavailable.", "warning");
  }
}

/** Identify union candidates using only required constant discriminator properties.
 * This avoids probing every evidence variant for every record in a large artifact.
 */
function discriminatorMismatch(schema: ShapeSchema, value: unknown, shape: CompiledShape): boolean {
  if (typeof schema === "boolean") return schema === false;
  if (own(schema, "const") && !sameConstant(value, schema.const)) return true;
  if (!record(value)) return false;
  if (schema.$ref) {
    const resolved = shape.references.get(schema.$ref);
    if (resolved && resolved !== schema && typeof resolved !== "boolean" && !resolved.$ref) schema = resolved;
  }
  for (const key of schema.required ?? []) {
    const property = schema.properties?.[key];
    if (property && typeof property !== "boolean" && own(property, "const") && (!own(value, key) || !sameConstant(value[key], property.const))) return true;
  }
  return false;
}

function* union(
  branches: readonly ShapeSchema[], exactlyOne: boolean, value: unknown, context: Context, result: Sink, path: string, depth: number,
): Generator<void, void> {
  let successes = 0;
  let winning: Sink | null = null;
  let closest: Sink | null = null;
  for (const branch of branches) {
    if (discriminatorMismatch(branch, value, context.shape)) continue;
    const trial = sink(Math.min(result.max, 8));
    yield* visit(branch, value, context, trial, path, depth + 1);
    if (context.exhausted) { merge(result, trial); return; }
    if (!trial.errors) {
      successes++;
      winning = trial;
      if (!exactlyOne) { merge(result, trial); return; }
    } else if (!closest || trial.errors < closest.errors) closest = trial;
  }
  if (successes === 1 && winning) { merge(result, winning); return; }
  if (successes > 1) {
    issue(result, path, "union", "More than one mutually exclusive schema variant matches.");
    return;
  }
  issue(result, path, "union", "No allowed schema variant matches this value.");
  // A closest branch gives useful field-level diagnostics without printing
  // every mutually exclusive alternative or counting every alternative as invalid.
  if (closest) merge(result, closest);
}

function* visit(
  schema: ShapeSchema, value: unknown, context: Context, result: Sink, path: string, depth: number,
): Generator<void, void> {
  if (!(yield* tick(context, result, path, depth))) return;
  if (schema === true) return;
  if (schema === false) { issue(result, path, "type", "This value is not allowed by the schema."); return; }
  if (schema.$ref) {
    const target = context.shape.references.get(schema.$ref);
    if (target === undefined) throw new UnsupportedShapeSchema("Missing compiled reference.");
    // References may legitimately recur in nested data. A same-path cycle cannot
    // consume input and therefore cannot establish that the value is valid.
    const active = context.activeRefs.get(path) ?? new Set<ShapeSchema>();
    if (active.has(target)) {
      issue(result, path, "cycle", "A schema reference recurred without consuming an instance field.", "warning");
      context.exhausted = true;
      return;
    }
    active.add(target);
    context.activeRefs.set(path, active);
    yield* visit(target, value, context, result, path, depth + 1);
    active.delete(target);
    if (!active.size) context.activeRefs.delete(path);
    if (context.exhausted) return;
  }
  if (schema.type !== undefined) {
    const types = Array.isArray(schema.type) ? schema.type : [schema.type];
    if (!types.some((type) => matchesType(value, type))) {
      issue(result, path, "type", `Expected ${types.join(" or ")}.`);
      return;
    }
  }
  if (own(schema, "const") && !sameConstant(value, schema.const)) issue(result, path, "constant", "Value does not match the allowed constant.");
  if (schema.minimum !== undefined && typeof value === "number" && value < schema.minimum) issue(result, path, "minimum", `Expected a number at least ${schema.minimum}.`);
  if (schema.format) checkFormat(value, schema.format, result, path);
  if (schema.oneOf) yield* union(schema.oneOf, true, value, context, result, path, depth);
  if (context.exhausted) return;
  if (schema.anyOf) yield* union(schema.anyOf, false, value, context, result, path, depth);
  if (context.exhausted) return;
  if (Array.isArray(value) && schema.items !== undefined) {
    for (let index = 0; index < value.length && !context.exhausted; index++) yield* visit(schema.items, value[index], context, result, child(path, index), depth + 1);
  }
  if (record(value)) {
    for (const key of schema.required ?? []) if (!own(value, key)) issue(result, child(path, key), "required", "Required property is missing.");
    for (const [key, property] of Object.entries(schema.properties ?? {})) {
      if (context.exhausted) break;
      if (own(value, key)) yield* visit(property, value[key], context, result, child(path, key), depth + 1);
    }
    if (schema.additionalProperties !== undefined && schema.additionalProperties !== true) {
      for (const key of Object.keys(value)) {
        if (context.exhausted) break;
        if (own(schema.properties ?? {}, key)) continue;
        if (schema.additionalProperties === false) {
          if (!(yield* tick(context, result, child(path, key), depth + 1))) break;
          issue(result, child(path, key), "additional-property", "Property is not allowed by this schema.");
        } else yield* visit(schema.additionalProperties, value[key], context, result, child(path, key), depth + 1);
      }
    }
  }
}

function validateLimits(limits: Readonly<ShapeLimits>): void {
  for (const value of Object.values(limits)) {
    if (!Number.isSafeInteger(value) || value < 1) throw new RangeError("Integrity-check limits must be positive integers.");
  }
  if (limits.depth > 256) throw new RangeError("Integrity-check depth must not exceed 256.");
  if (limits.issues > 10_000) throw new RangeError("Integrity-check issue cap must not exceed 10,000.");
}

function* validate(
  shape: CompiledShape, value: unknown, limits: Readonly<ShapeLimits>, signal?: AbortSignal,
): Generator<void, ShapeReport> {
  validateLimits(limits);
  const context: Context = { shape, limits, checked: 0, exhausted: false, activeRefs: new Map(), signal };
  const result = sink(limits.issues);
  yield* visit(shape.root, value, context, result, "", 0);
  return {
    status: context.exhausted ? "incomplete" : result.errors ? "invalid" : "valid",
    errors: result.errors,
    warnings: result.warnings,
    checked: Math.min(context.checked, limits.nodes),
    issues: result.issues,
    omittedIssues: result.errors + result.warnings - result.issues.length,
    complete: !context.exhausted,
  };
}

/** Synchronous validation for small documents and tests. */
export function validateShape(shape: CompiledShape, value: unknown, limits = DEFAULT_SHAPE_LIMITS): ShapeReport {
  const iterator = validate(shape, value, limits);
  for (let step = iterator.next(); ; step = iterator.next()) if (step.done) return step.value;
}

export interface AsyncShapeOptions {
  limits?: Readonly<ShapeLimits>;
  signal?: AbortSignal;
  /** Injected scheduler for tests and other runtimes. */
  yieldControl?: () => Promise<void>;
}

/** Cooperative validation for the browser; each batch yields to the event loop
 * so a Cancel button remains usable. Never sends input or schema over the network.
 */
export async function validateShapeAsync(shape: CompiledShape, value: unknown, options: AsyncShapeOptions = {}): Promise<ShapeReport> {
  const pause = options.yieldControl ?? (() => new Promise<void>((resolve) => setTimeout(resolve, 0)));
  const iterator = validate(shape, value, options.limits ?? DEFAULT_SHAPE_LIMITS, options.signal);
  try {
    for (let step = iterator.next(); ; step = iterator.next()) {
      if (step.done) return step.value;
      await pause();
      if (options.signal?.aborted) throw new DOMException("Integrity check cancelled.", "AbortError");
    }
  } finally {
    iterator.return(undefined as never);
  }
}
