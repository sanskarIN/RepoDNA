import { describe, expect, it } from "vitest";
import {
  compileShape,
  DEFAULT_SHAPE_LIMITS,
  validateShape,
  validateShapeAsync,
  validTimestamp,
  type ShapeSchema,
  type ShapeLimits,
} from "../src/schemaValidation";

const limits = (overrides: Partial<ShapeLimits>) => ({ ...DEFAULT_SHAPE_LIMITS, ...overrides });
const check = (schema: ShapeSchema, value: unknown, options?: ShapeLimits) => validateShape(compileShape(schema), value, options);

function errorOf(callback: () => unknown): string {
  try { callback(); } catch (reason) { return reason instanceof Error ? reason.message : String(reason); }
  throw new Error("Expected an error");
}

async function rejectionOf(promise: Promise<unknown>): Promise<Error> {
  try { await promise; } catch (reason) { if (reason instanceof Error) return reason; throw reason; }
  throw new Error("Expected rejection");
}

const fixture: ShapeSchema = {
  type: "object",
  required: ["schemaVersion", "structure", "findings"],
  properties: {
    schemaVersion: { type: "string" },
    structure: { type: "object", required: ["files", "count"], properties: {
      files: { type: "array", items: { $ref: "#/$defs/File" } },
      count: { type: "integer", format: "uint64", minimum: 0 },
    } },
    findings: { type: "array", items: { $ref: "#/$defs/Evidence" } },
  },
  $defs: {
    File: { type: "object", required: ["path", "bytes"], properties: {
      path: { type: "string" }, bytes: { type: "integer", format: "uint64", minimum: 0 },
      language: { type: ["string", "null"] },
    } },
    Evidence: { oneOf: [
      { type: "object", required: ["kind", "path"], properties: {
        kind: { const: "file" }, path: { type: "string" }, line: { type: ["integer", "null"], minimum: 0 },
      } },
      { type: "object", required: ["kind", "metric", "value"], properties: {
        kind: { const: "metric" }, metric: { type: "string" }, value: { type: "number" },
      } },
    ] },
  },
};

function document() {
  return {
    schemaVersion: "1.0",
    structure: { files: [{ path: "src/main.rs", bytes: 32, language: "rust" }], count: 1 },
    findings: [{ kind: "metric", metric: "complexity", value: 2.5 }],
  };
}

describe("compiled RepoDNA schema subset", () => {
  it("validates nested objects, local references, arrays, nullable types and discriminated unions", () => {
    const result = check(fixture, document());
    expect(result.status).toBe("valid");
    expect(result.complete).toBe(true);
    expect(result.errors).toBe(0);
    expect(result.checked > 10).toBe(true);
  });

  it("allows unknown fields for forward compatibility without interpreting them", () => {
    const input = document();
    const result = check(fixture, { ...input, newerSection: { unknown: [1, 2, 3] }, structure: { ...input.structure, future: true } });
    expect(result.status).toBe("valid");
  });

  it("does not mutate the document or fill missing values from schema defaults", () => {
    const schema: ShapeSchema = { type: "object", required: ["items"], properties: { items: { type: "array", default: [] } } };
    const input = Object.freeze({});
    const result = check(schema, input);
    expect(result.status).toBe("invalid");
    expect(result.issues[0]?.path).toBe("/items");
    expect(input).toEqual({});
  });

  it("reports exact nested field paths without echoing sensitive instance values", () => {
    const input = document();
    const result = check(fixture, { ...input, structure: { count: 1, files: [{ path: "src/main.rs", bytes: "secret-token" }] } });
    expect(result.issues.map(({ path, code }) => [path, code])).toEqual([["/structure/files/0/bytes", "type"]]);
    expect(JSON.stringify(result).includes("secret-token")).toBe(false);
  });

  it("distinguishes missing required fields from present null or malformed values", () => {
    const schema: ShapeSchema = { type: "object", required: ["name"], properties: { name: { type: "string" } } };
    expect(check(schema, {}).issues[0]?.code).toBe("required");
    expect(check(schema, { name: null }).issues[0]?.code).toBe("type");
    expect(check(schema, { name: "" }).status).toBe("valid");
  });

  it("never treats inherited prototype properties as required JSON properties", () => {
    const value = Object.create({ name: "inherited" });
    expect(check({ type: "object", required: ["name"] }, value).issues[0]?.code).toBe("required");
  });

  it("escapes JSON pointer components", () => {
    const schema: ShapeSchema = { type: "object", properties: { "a/b~c": { type: "integer" } } };
    const result = check(schema, { "a/b~c": "not a number" });
    expect(result.issues[0]?.path).toBe("/a~1b~0c");
  });

  it("supports object maps with typed additional properties", () => {
    const schema: ShapeSchema = { type: "object", properties: { known: { type: "string" } }, additionalProperties: { type: "integer" } };
    expect(check(schema, { known: "value", a: 1, b: 2 }).status).toBe("valid");
    expect(check(schema, { known: "value", a: "wrong" }).issues[0]?.path).toBe("/a");
  });

  it("enforces explicitly closed object schemas", () => {
    const schema: ShapeSchema = { type: "object", properties: { known: true }, additionalProperties: false };
    expect(check(schema, { known: 1 }).status).toBe("valid");
    const result = check(schema, { known: 1, extra: "value" });
    expect(result.issues[0]?.code).toBe("additional-property");
    expect(result.issues[0]?.path).toBe("/extra");
  });

  it("honors boolean schemas", () => {
    expect(check(true, null).status).toBe("valid");
    expect(check(false, null).status).toBe("invalid");
    expect(check({ type: "array", items: false }, []).status).toBe("valid");
    expect(check({ type: "array", items: false }, [1]).status).toBe("invalid");
  });

  it("resolves escaped definition names and rejects prototype traversal", () => {
    const schema: ShapeSchema = { $ref: "#/$defs/a~1b~0c", $defs: { "a/b~c": { type: "integer" } } };
    expect(check(schema, 3).status).toBe("valid");
    expect(check(schema, "x").status).toBe("invalid");
    expect(errorOf(() => compileShape({ $ref: "#/constructor" }))).toContain("Unresolved local");
  });

  it("refuses external and unresolved schema references rather than fetching them", () => {
    expect(errorOf(() => compileShape({ $ref: "https://example.invalid/schema" }))).toContain("External schema");
    expect(errorOf(() => compileShape({ $ref: "#/$defs/Unknown" }))).toContain("Unresolved local");
    expect(errorOf(() => compileShape({ $ref: "#/%GG" }))).toContain("Malformed local");
  });

  it("rejects unsupported schema validation keywords and formats", () => {
    expect(errorOf(() => compileShape({ type: "string", pattern: "x" } as ShapeSchema))).toContain("Unsupported schema keyword");
    expect(errorOf(() => compileShape({ type: "string", format: "uri" }))).toContain("Unsupported schema format");
  });

  it("rejects empty unions and invalid schema declarations", () => {
    expect(errorOf(() => compileShape({ anyOf: [] }))).toContain("Empty or invalid schema union");
    expect(errorOf(() => compileShape({ type: [] }))).toContain("Invalid schema type");
    expect(errorOf(() => compileShape({ type: ["integer", "integer"] }))).toContain("Invalid schema type");
    expect(errorOf(() => compileShape({ minimum: Infinity }))).toContain("Invalid minimum");
  });

  it("applies reference sibling constraints rather than ignoring them", () => {
    const schema: ShapeSchema = { $ref: "#/$defs/Number", minimum: 10, $defs: { Number: { type: "integer" } } };
    expect(check(schema, 5).issues[0]?.code).toBe("minimum");
    expect(check(schema, 10).status).toBe("valid");
  });

  it("accepts recursively defined nested trees while terminating same-path reference loops", () => {
    const schema: ShapeSchema = { $ref: "#/$defs/Node", $defs: { Node: { type: "object", required: ["children"], properties: { children: { type: "array", items: { $ref: "#/$defs/Node" } } } } } };
    expect(check(schema, { children: [{ children: [{ children: [] }] }] }).status).toBe("valid");
    const loop = check({ $ref: "#" }, 1);
    expect(loop.status).toBe("incomplete");
    expect(loop.issues[0]?.code).toBe("cycle");
  });
});

describe("union branch diagnostics", () => {
  it("accepts exactly one matching variant and rejects overlapping oneOf variants", () => {
    const schema: ShapeSchema = { oneOf: [{ type: "integer" }, { type: "number" }] };
    expect(check(schema, 1.5).status).toBe("valid");
    expect(check(schema, 1).issues[0]?.message).toContain("More than one");
  });

  it("accepts anyOf when multiple variants match", () => {
    expect(check({ anyOf: [{ type: "integer" }, { type: "number" }] }, 1).status).toBe("valid");
  });

  it("checks required discriminators without treating optional ones as required", () => {
    const schema: ShapeSchema = { oneOf: [{ type: "object", properties: { kind: { const: "a" } } }, { type: "number" }] };
    expect(check(schema, {}).status).toBe("valid");
  });

  it("provides useful nested diagnostics from the closest discriminator branch", () => {
    const result = check(fixture, { ...document(), findings: [{ kind: "file", path: 10 }] });
    expect(result.issues.map(({ path, code }) => [path, code])).toEqual([["/findings/0", "union"], ["/findings/0/path", "type"]]);
  });

  it("does not report mismatching alternative fields for a valid union", () => {
    const result = check(fixture, { ...document(), findings: [{ kind: "file", path: "README.md", line: null }] });
    expect(result.issues).toEqual([]);
  });

  it("rejects unknown discriminator values without exposing their values", () => {
    const result = check(fixture, { ...document(), findings: [{ kind: "sensitive" }] });
    expect(result.errors).toBe(1);
    expect(result.issues[0]?.code).toBe("union");
    expect(JSON.stringify(result).includes("sensitive")).toBe(false);
  });

  it("checks scalar enum constants efficiently", () => {
    const schema: ShapeSchema = { oneOf: Array.from({ length: 100 }, (_, index) => ({ const: `item-${index}` })) };
    const result = check(schema, "item-99", limits({ nodes: 5 }));
    expect(result.status).toBe("valid");
    expect(result.checked).toBe(2);
  });
});

describe("numeric and date-time integrity", () => {
  it("rejects fractional integers and nonfinite numeric inputs", () => {
    expect(check({ type: "integer" }, 0.5).status).toBe("invalid");
    expect(check({ type: "number" }, Infinity).status).toBe("invalid");
    expect(check({ type: "number" }, NaN).status).toBe("invalid");
    expect(check({ type: "number" }, 1.5).status).toBe("valid");
  });

  it("enforces unsigned 32-bit bounds and signed 32-bit bounds", () => {
    for (const value of [-1, 2 ** 32]) expect(check({ type: "integer", format: "uint32" }, value).status).toBe("invalid");
    for (const value of [0, 2 ** 32 - 1]) expect(check({ type: "integer", format: "uint32" }, value).status).toBe("valid");
    expect(check({ type: "integer", format: "int32" }, -(2 ** 31)).status).toBe("valid");
    expect(check({ type: "integer", format: "int32" }, 2 ** 31).status).toBe("invalid");
  });

  it("warns rather than inventing exact precision for representable 64-bit numbers", () => {
    const result = check({ type: "integer", format: "uint64" }, 2 ** 60);
    expect(result.status).toBe("valid");
    expect(result.warnings).toBe(1);
    expect(result.issues[0]?.code).toBe("precision");
    expect(check({ type: "integer", format: "uint64" }, 2 ** 65).status).toBe("invalid");
    expect(check({ type: "integer", format: "int64" }, -(2 ** 63)).warnings).toBe(1);
  });

  it("does not apply numeric formats to nullable null fields", () => {
    expect(check({ type: ["integer", "null"], format: "uint64" }, null).status).toBe("valid");
  });

  it("validates RFC 3339 calendar dates, offsets, fractions and leap-year rules", () => {
    for (const timestamp of ["2026-11-28T09:00:00Z", "2000-02-29T12:30:30.123+05:30", "2024-02-29t23:59:60z", "2026-01-01T00:00:00-00:00"]) expect(validTimestamp(timestamp)).toBe(true);
    for (const timestamp of ["2026-02-29T00:00:00Z", "1900-02-29T00:00:00Z", "2026-04-31T00:00:00Z", "2026-01-01T24:00:00Z", "2026-01-01T00:60:00Z", "2026-01-01T00:00:61Z", "2026-01-01T00:00:00+24:00", "2026-01-01", "2026-01-01T00:00:00"]) expect(validTimestamp(timestamp)).toBe(false);
  });

  it("reports invalid timestamps at the original field path", () => {
    const result = check({ type: "object", properties: { generatedAt: { type: "string", format: "date-time" } } }, { generatedAt: "yesterday" });
    expect(result.issues[0]?.path).toBe("/generatedAt");
    expect(result.issues[0]?.code).toBe("format");
  });
});

describe("bounded and cooperative schema inspection", () => {
  it("counts every error but retains only the configured diagnostic sample", () => {
    const result = check({ type: "array", items: { type: "integer" } }, Array(10).fill("bad"), limits({ issues: 3 }));
    expect(result.errors).toBe(10);
    expect(result.issues.length).toBe(3);
    expect(result.omittedIssues).toBe(7);
    expect(result.complete).toBe(true);
  });

  it("cannot report valid when the work budget ended before the final field", () => {
    const result = check({ type: "array", items: { type: "integer" } }, [1, 2, 3, "late error"], limits({ nodes: 3 }));
    expect(result.status).toBe("incomplete");
    expect(result.complete).toBe(false);
    expect(result.checked).toBe(3);
    expect(result.issues[0]?.code).toBe("budget");
  });

  it("bounds nested data validation depth", () => {
    const schema: ShapeSchema = { type: "array", items: { $ref: "#" } };
    const result = check(schema, [[[[[[[]]]]]]], limits({ depth: 3 }));
    expect(result.status).toBe("incomplete");
    expect(result.issues[0]?.code).toBe("budget");
  });

  it("counts union probes toward the shared resource budget", () => {
    const schema: ShapeSchema = { oneOf: Array.from({ length: 20 }, () => ({ type: "object", required: ["absent"] })) };
    const result = check(schema, {}, limits({ nodes: 5 }));
    expect(result.status).toBe("incomplete");
    expect(result.complete).toBe(false);
  });

  it("rejects invalid resource limits instead of silently disabling them", () => {
    for (const options of [limits({ nodes: 0 }), limits({ depth: 257 }), limits({ batch: -1 }), limits({ issues: 10001 })]) expect(errorOf(() => check(true, null, options))).toContain("Integrity-check");
  });

  it("produces identical results with synchronous and cooperative scheduling", async () => {
    const shape = compileShape(fixture);
    const input = document();
    const options = limits({ batch: 2 });
    let yields = 0;
    const asynchronous = await validateShapeAsync(shape, input, { limits: options, yieldControl: async () => { yields++; } });
    expect(asynchronous).toEqual(validateShape(shape, input, options));
    expect(yields > 1).toBe(true);
  });

  it("honors cancellation before inspecting any field", async () => {
    const controller = new AbortController();
    controller.abort();
    const reason = await rejectionOf(validateShapeAsync(compileShape(fixture), document(), { signal: controller.signal }));
    expect(reason.name).toBe("AbortError");
  });

  it("honors cancellation at a cooperative yield without returning a false valid report", async () => {
    const controller = new AbortController();
    let yields = 0;
    const result = validateShapeAsync(compileShape({ type: "array", items: { type: "integer" } }), Array(1000).fill(1), { signal: controller.signal, limits: limits({ batch: 10 }), yieldControl: async () => { yields++; controller.abort(); } });
    expect((await rejectionOf(result)).name).toBe("AbortError");
    expect(yields).toBe(1);
  });
});
