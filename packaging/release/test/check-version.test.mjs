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

test('rejects a missing Windows executable and installer publisher', (t) => {
  const root = fixture(t);
  editJson(root, 'apps/desktop/src-tauri/tauri.conf.json', (config) => {
    delete config.bundle.publisher;
  });
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /desktop Tauri publisher must be SanskarIN/);
});

test('rejects the old inferred github publisher', (t) => {
  const root = fixture(t);
  editJson(root, 'apps/desktop/src-tauri/tauri.conf.json', (config) => {
    config.bundle.publisher = 'github';
  });
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /desktop Tauri publisher must be SanskarIN/);
});

test('preserves the existing desktop application identifier for upgrades', (t) => {
  const root = fixture(t);
  editJson(root, 'apps/desktop/src-tauri/tauri.conf.json', (config) => {
    config.identifier = 'io.sanskarin.repodna';
  });
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /desktop Tauri identifier must remain stable/);
});

test('rejects a stale internal Rust workspace dependency despite synchronized lockfiles', (t) => {
  const root = fixture(t);
  const path = join(root, 'Cargo.toml');
  const contents = readFileSync(path, 'utf8');
  const currentVersion = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version;
  const original = `repodna-core = { path = "crates/repodna-core", version = "${currentVersion}" }`;
  assert.ok(contents.includes(original));
  writeFileSync(path, contents.replace(original, 'repodna-core = { path = "crates/repodna-core", version = "0.0.1" }'));
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Cargo\.toml workspace\.dependencies repodna-core version/);
});

test('rejects a missing version constraint on a released internal crate', (t) => {
  const root = fixture(t);
  const path = join(root, 'Cargo.toml');
  const contents = readFileSync(path, 'utf8');
  const currentVersion = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version;
  const original = `repodna-report = { path = "crates/repodna-report", version = "${currentVersion}" }`;
  assert.ok(contents.includes(original));
  writeFileSync(path, contents.replace(original, 'repodna-report = { path = "crates/repodna-report" }'));
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Cargo\.toml workspace\.dependencies repodna-report version/);
});

test('rejects a missing internal Rust workspace dependency even with enough others present', (t) => {
  const root = fixture(t);
  const path = join(root, 'Cargo.toml');
  const contents = readFileSync(path, 'utf8');
  const version = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version;
  const entry = `repodna-app = { path = "crates/repodna-app", version = "${version}" }\n`;
  assert.ok(contents.includes(entry));
  writeFileSync(path, contents.replace(entry, ''));
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /required internal workspace dependency inventory/);
});

test('rejects a duplicate internal Rust workspace dependency', (t) => {
  const root = fixture(t);
  const path = join(root, 'Cargo.toml');
  const contents = readFileSync(path, 'utf8');
  const version = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version;
  const entry = `repodna-app = { path = "crates/repodna-app", version = "${version}" }\n`;
  assert.ok(contents.includes(entry));
  writeFileSync(path, contents.replace(entry, entry + entry));
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /required internal workspace dependency inventory/);
});

test('rejects internal Rust workspace dependency paths pointing to the wrong crate', (t) => {
  const root = fixture(t);
  const path = join(root, 'Cargo.toml');
  const contents = readFileSync(path, 'utf8');
  const entry = 'repodna-app = { path = "crates/repodna-app"';
  assert.ok(contents.includes(entry));
  writeFileSync(path, contents.replace(entry, 'repodna-app = { path = "crates/repodna-server"'));
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /workspace.dependencies repodna-app path is out of sync/);
});

test('requires the local testkit workspace dependency to remain unversioned', (t) => {
  const root = fixture(t);
  const path = join(root, 'Cargo.toml');
  const contents = readFileSync(path, 'utf8');
  const entry = 'repodna-testkit = { path = "crates/repodna-testkit" }';
  assert.ok(contents.includes(entry));
  writeFileSync(
    path,
    contents.replace(entry, 'repodna-testkit = { path = "crates/repodna-testkit", version = "0.0.1" }'),
  );
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /repodna-testkit must remain unversioned/);
});

test('rejects a missing root Cargo.lock internal package even if enough entries remain', (t) => {
  const root = fixture(t);
  const path = join(root, 'Cargo.lock');
  const contents = readFileSync(path, 'utf8');
  const name = 'name = "repodna-report"';
  assert.ok(contents.includes(name));
  writeFileSync(path, contents.replace(name, 'name = "missing-report"'));
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Cargo\.lock: internal Rust package inventory/);
});

test('rejects an unexpected internal package in root Cargo.lock', (t) => {
  const root = fixture(t);
  const path = join(root, 'Cargo.lock');
  const contents = readFileSync(path, 'utf8');
  const name = 'name = "repodna-report"';
  assert.ok(contents.includes(name));
  writeFileSync(path, contents.replace(name, 'name = "repodna-unexpected"'));
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Cargo\.lock: internal Rust package inventory/);
});

test('rejects duplicated internal packages in the desktop Cargo.lock', (t) => {
  const root = fixture(t);
  const path = join(root, 'apps/desktop/src-tauri/Cargo.lock');
  const contents = readFileSync(path, 'utf8');
  const name = 'name = "repodna-app"';
  assert.ok(contents.includes(name));
  writeFileSync(path, contents.replace(name, 'name = "repodna-core"'));
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /apps\/desktop\/src-tauri\/Cargo\.lock: internal Rust package inventory/);
});

test('rejects an out-of-sync xtask package in root Cargo.lock', (t) => {
  const root = fixture(t);
  const path = join(root, 'Cargo.lock');
  const contents = readFileSync(path, 'utf8');
  const version = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version;
  const entry = `name = "xtask"\nversion = "${version}"`;
  assert.ok(contents.includes(entry));
  writeFileSync(path, contents.replace(entry, 'name = "xtask"\nversion = "0.0.1"'));
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Cargo\.lock: xtask is out of sync/);
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

test('rejects missing workspace records in the npm lockfile', (t) => {
  const root = fixture(t);
  editJson(root, 'package-lock.json', (data) => {
    delete data.packages['packages/schema'];
  });
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /missing workspace packages\/schema/);
});

test('rejects missing root entry in the npm lockfile', (t) => {
  const root = fixture(t);
  editJson(root, 'package-lock.json', (data) => {
    delete data.packages[''];
  });
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /missing workspace root/);
});
