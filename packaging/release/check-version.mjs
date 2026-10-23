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
const npmLock = json('package-lock.json');
assert.equal(npmLock.version, version);
const requiredWorkspaces = [
  '',
  'apps/web',
  'apps/desktop',
  'packages/schema',
  'packages/visualization',
  'packages/wasm',
];
for (const name of requiredWorkspaces) {
  assert.ok(npmLock.packages?.[name], 'package-lock.json: missing workspace ' + (name || 'root'));
}
for (const [name, pkg] of Object.entries(npmLock.packages)) {
  if (name === '' || /^(apps\/(web|desktop)|packages\/(schema|visualization|wasm))$/.test(name)) {
    assert.equal(pkg.version, version, 'npm lockfile package ' + name);
  }
}
// Inspect the actual dependency objects: looking for version strings anywhere in the
// JSON can pass even when a particular workspace has an out-of-sync dependency.
const webManifest = json('apps/web/package.json');
const webLock = npmLock.packages['apps/web'];
assert.ok(webLock, 'package-lock.json: missing apps/web workspace');
for (const name of ['schema', 'visualization', 'wasm']) {
  const dependency = '@repodna/' + name;
  assert.equal(
    webManifest.dependencies?.[dependency],
    version,
    'apps/web/package.json: ' + dependency + ' must match ' + version,
  );
  assert.equal(
    webLock.dependencies?.[dependency],
    version,
    'package-lock.json: apps/web ' + dependency + ' must match ' + version,
  );
}

const workspace = read('Cargo.toml');
assert.equal(
  workspace.match(/^\[workspace\.package\]\r?\nversion = "([^"]+)"/m)?.[1],
  version,
  'Cargo.toml workspace.package version is out of sync',
);
const desktop = read('apps/desktop/src-tauri/Cargo.toml');
assert.equal(
  desktop.match(/^version = "([^"]+)"/m)?.[1],
  version,
  'desktop Cargo.toml version is out of sync',
);
for (const path of ['Cargo.lock', 'apps/desktop/src-tauri/Cargo.lock']) {
  const text = read(path);
  const matches = [...text.matchAll(/name = "(repodna-[^"]+)"\r?\nversion = "([^"]+)"/g)];
  assert.ok(matches.length >= 15, path + ': missing internal Rust crate versions');
  for (const [, name, lockedVersion] of matches) {
    assert.equal(lockedVersion, version, path + ': ' + name + ' is out of sync');
  }
  if (path === 'Cargo.lock') {
    assert.equal(
      text.match(/name = "xtask"\r?\nversion = "([^"]+)"/)?.[1],
      version,
      'xtask CLI workspace package version is out of sync',
    );
  }
}
assert.ok(
  read('CHANGELOG.md').includes('## [' + version + '] - '),
  'CHANGELOG.md needs a dated section for ' + version
);

console.log('Verified RepoDNA ' + version + ' versions in Rust, Tauri, npm and both lockfiles.');
