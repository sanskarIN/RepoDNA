#!/usr/bin/env node
// Stamp the built GitHub Pages artifact with its *published source commit*.
// This file contains no environment dump, credentials or repository analysis.
import { existsSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

function fail(message) {
  throw new Error(`Pages provenance: ${message}`);
}

export function manifestFromEnvironment(env) {
  const sha = env.GITHUB_SHA;
  const runId = env.GITHUB_RUN_ID;
  const attempt = env.GITHUB_RUN_ATTEMPT;
  if (!/^[0-9a-f]{40}$/.test(sha ?? "")) {
    fail("GITHUB_SHA must be a lowercase 40-character Git commit SHA");
  }
  if (!/^[1-9][0-9]*$/.test(runId ?? "") || !Number.isSafeInteger(Number(runId))) {
    fail("GITHUB_RUN_ID must be a positive safe integer");
  }
  if (!/^[1-9][0-9]*$/.test(attempt ?? "") || !Number.isSafeInteger(Number(attempt))) {
    fail("GITHUB_RUN_ATTEMPT must be a positive safe integer");
  }
  return {
    schema: "repodna-pages-deployment/1",
    commitSha: sha,
    workflowRunId: Number(runId),
    workflowRunAttempt: Number(attempt),
  };
}

export function stampDeployment(outputDirectory, env = process.env) {
  if (!outputDirectory || !existsSync(outputDirectory) || !statSync(outputDirectory).isDirectory()) {
    fail("output must be an existing built site directory");
  }
  const manifest = manifestFromEnvironment(env);
  const destination = join(outputDirectory, "deployment.json");
  // Refuse accidental overwrite from a previous build or workflow attempt.
  writeFileSync(destination, `${JSON.stringify(manifest, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
  });
  return destination;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    if (process.argv.length !== 3) {
      fail("usage: node packaging/docs/stamp-deployment.mjs SITE_DIST");
    }
    stampDeployment(process.argv[2]);
  } catch (error) {
    console.error(error instanceof Error ? error.message : "Pages provenance failed");
    process.exitCode = 1;
  }
}
