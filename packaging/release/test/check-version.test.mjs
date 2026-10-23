import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import test from 'node:test';

const checker = resolve('packaging/release/check-version.mjs');
const files = [
  'package.json',
  'package-lock.json',
  'apps/desktop/package.json',
  'apps/web/package.json',
  'packages/schema/package.json',
  'packages/visualization/package.json',
  'packages/wasm/package.json',
  'apps/desktop/src-tauri/tauri.conf.json',
  'Cargo.toml',
  'apps/desktop/src-tauri/Cargo.toml',
  'Cargo.lock',
  'apps/desktop/src-tauri/Cargo.lock',
  'CHANGELOG.md',
];

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'repodna-release-version-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const file of files) {
    const destination = join(root, file);
    mkdirSync(dirname(destination), { recursive: true });
    copyFileSync(resolve(file), destination);
  }
  return root;
}

function check(root) {
  return spawnSync(process.execPath, [checker], { cwd: root, encoding: 'utf8' });
}

function editJson(root, file, change) {
  const path = join(root, file);
  const data = JSON.parse(readFileSync(path, 'utf8'));
  change(data);
  writeFileSync(path, JSON.stringify(data, null, 2) + '\n');
}

test('accepts synchronized release metadata from both package ecosystems', (t) => {
  const result = check(fixture(t));
  assert.equal(result.status, 0, result.stderr);
});

test('rejects an incorrect web dependency version even when the right string appears elsewhere', (t) => {
  const root = fixture(t);
  editJson(root, 'apps/web/package.json', (data) => {
    data.dependencies['@repodna/schema'] = '0.0.1';
  });
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /apps\/web\/package\.json: @repodna\/schema/);
});

test('rejects mismatched versions inside the web workspace lockfile entry', (t) => {
  const root = fixture(t);
  editJson(root, 'package-lock.json', (data) => {
    data.packages['apps/web'].dependencies['@repodna/wasm'] = '0.0.1';
  });
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /package-lock\.json: apps\/web @repodna\/wasm/);
});

test('rejects a missing internal dependency in the web workspace lockfile entry', (t) => {
  const root = fixture(t);
  editJson(root, 'package-lock.json', (data) => {
    delete data.packages['apps/web'].dependencies['@repodna/visualization'];
  });
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /package-lock\.json: apps\/web @repodna\/visualization/);
});
