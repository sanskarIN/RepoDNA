import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const ci = readFileSync('.github/workflows/ci.yml', 'utf8');
const release = readFileSync('.github/workflows/release.yml', 'utf8');

const rootCommand = 'cargo metadata --locked --format-version 1 > /dev/null';
const desktopCommand =
  'cargo metadata --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --format-version 1 > /dev/null';

function job(workflow, name) {
  const startMarker = '\n  ' + name + ':\n';
  const start = workflow.indexOf(startMarker);
  assert.notEqual(start, -1, 'missing workflow job: ' + name);
  const remaining = workflow.slice(start + startMarker.length);
  const next = remaining.search(/\n  [a-z][a-z-]*:\n/);
  return remaining.slice(0, next === -1 ? undefined : next);
}

function assertFailClosedPreflight(body, subsequentStep) {
  const root = body.indexOf(rootCommand);
  const desktop = body.indexOf(desktopCommand);
  const subsequent = body.indexOf(subsequentStep);
  assert.ok(root !== -1, 'root Cargo.lock must be checked with --locked');
  assert.ok(desktop !== -1, 'desktop Cargo.lock must be checked with --locked');
  assert.ok(subsequent !== -1, 'expected next workflow step is missing');
  assert.ok(root < desktop && desktop < subsequent, 'lockfile preflight must run first');
  const segment = body.slice(body.lastIndexOf('- name:', root), subsequent);
  assert.match(segment, /set -euo pipefail/, 'preflight must stop after a failed command');
  assert.doesNotMatch(segment, /continue-on-error:\s*true/, 'lockfile errors cannot be ignored');
}

test('CI checks both Rust lockfiles before generating third-party notices', () => {
  assertFailClosedPreflight(
    job(ci, 'web'),
    '- name: Check the third-party notices',
  );
});

test('release prepare checks both Rust lockfiles before deciding to publish', () => {
  assertFailClosedPreflight(
    job(release, 'prepare'),
    '- id: version',
  );
});
