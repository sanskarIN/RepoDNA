#!/usr/bin/env node
// Ensure release metadata is in sync before packaging any binaries.
// Uses only Node built-ins and never writes to the working tree.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = (path) => readFileSync(path, 'utf8');
const json = (path) => JSON.parse(read(path));
const version = json('package.json').version;
assert.match(version, /^\d+\.\d+\.\d+$/, 'release version must be stable numeric semver');

const manifests = [
  'apps/desktop/package.json',
  'apps/web/package.json',
  'packages/schema/package.json',
  'packages/visualization/package.json',
  'packages/wasm/package.json',
  'apps/desktop/src-tauri/tauri.conf.json',
];
for (const path of manifests) {
  assert.equal(json(path).version, version, path + ' version is out of sync');
}
assert.equal(json('package-lock.json').version, version);
for (const [name, pkg] of Object.entries(json('package-lock.json').packages)) {
  if (name === '' || /^(apps\/(web|desktop)|packages\/(schema|visualization|wasm))$/.test(name)) {
    assert.equal(pkg.version, version, 'npm lockfile package ' + name);
  }
}
for (const path of ['apps/web/package.json', 'package-lock.json']) {
  const contents = read(path);
  for (const name of ['schema', 'visualization', 'wasm']) {
    assert.ok(
      contents.includes('"@repodna/' + name + '": "' + version + '"'),
      path + ': internal dependency @repodna/' + name + ' must match ' + version
    );
  }
}

const workspace = read('Cargo.toml');
assert.match(workspace, new RegExp('\\\\[workspace\\\\.package\\\\]\\\\s*version = "' + version.replaceAll('.', '\\\\.') + '"'));
const desktop = read('apps/desktop/src-tauri/Cargo.toml');
assert.match(desktop, new RegExp('\\\\[package\\\\][\\\\s\\\\S]*?\\\\nversion = "' + version.replaceAll('.', '\\\\.') + '"'));
for (const path of ['Cargo.lock', 'apps/desktop/src-tauri/Cargo.lock']) {
  const text = read(path);
  const matches = [...text.matchAll(/name = "(repodna-[^"]+)"\r?\nversion = "([^"]+)"/g)];
  assert.ok(matches.length >= 15, path + ': missing internal Rust crate versions');
  for (const [, name, lockedVersion] of matches) {
    assert.equal(lockedVersion, version, path + ': ' + name + ' is out of sync');
  }
}
assert.ok(
  read('CHANGELOG.md').includes('## [' + version + '] - '),
  'CHANGELOG.md needs a dated section for ' + version
);

console.log('Verified RepoDNA ' + version + ' versions in Rust, Tauri, npm and both lockfiles.');
