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

function check(root, args = []) {
  return spawnSync(process.execPath, [checker, ...args], { cwd: root, encoding: 'utf8' });
}

function currentVersion(root) {
  return JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version;
}

function replaceReleaseHeading(root, replacement) {
  const path = join(root, 'CHANGELOG.md');
  const changelog = readFileSync(path, 'utf8');
  const prefix = '## [' + currentVersion(root) + '] - ';
  const heading = changelog.split(/\r?\n/).find((line) => line.startsWith(prefix));
  assert.ok(heading, 'test fixture must contain the current release heading');
  writeFileSync(path, changelog.replace(heading, replacement));
  return path;
}

// The repository can be on a draft maintenance branch whose changelog says
// "Not released". Positive publication tests must control their own finalized
// version section rather than assume the checkout is already published.
function finalizedFixture(t) {
  const root = fixture(t);
  const path = join(root, 'CHANGELOG.md');
  const changelog = readFileSync(path, 'utf8');
  const heading = '## [' + currentVersion(root) + '] - ';
  const start = changelog.indexOf(heading);
  const next = changelog.indexOf('\n## [', start + heading.length);
  assert.ok(start >= 0 && next > start, 'test fixture needs a release section');
  const finalized = heading + '2026-11-23\n\n### Fixed\n\n- Verified release notes for tests.\n';
  writeFileSync(path, changelog.slice(0, start) + finalized + changelog.slice(next));
  return root;
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

test('publication mode accepts a finalized, dated release with actual highlights', (t) => {
  const result = check(finalizedFixture(t), ['--publish']);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /publication notes finalized/);
});

test('provisional changelog dates remain usable for trials but block tag publication', (t) => {
  const root = fixture(t);
  replaceReleaseHeading(root, '## [' + currentVersion(root) + '] - Unreleased');
  assert.equal(check(root).status, 0, 'trial metadata may be intentionally provisional');
  const result = check(root, ['--publish']);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /published release needs an ISO date/);
});

test('publication rejects impossible calendar dates instead of normalizing them', (t) => {
  const root = fixture(t);
  replaceReleaseHeading(root, '## [' + currentVersion(root) + '] - 2026-02-30');
  assert.equal(check(root).status, 0, 'the normal metadata check should not gate trials');
  const result = check(root, ['--publish']);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /published release date must be a real calendar day/);
});

test('publication rejects explicit unreleased notices even with a valid date', (t) => {
  const root = fixture(t);
  const heading = '## [' + currentVersion(root) + '] - 2026-11-23';
  const path = replaceReleaseHeading(root, heading);
  const contents = readFileSync(path, 'utf8');
  writeFileSync(
    path,
    contents.replace(heading, heading + '\n\n> **Not released:** Candidate preparation date only.'),
  );
  assert.equal(check(root).status, 0);
  const result = check(root, ['--publish']);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /published release notes must not contain provisional release wording/);
});

test('publication requires an actual change rather than an empty section', (t) => {
  const root = finalizedFixture(t);
  const path = join(root, 'CHANGELOG.md');
  const contents = readFileSync(path, 'utf8');
  const heading = '## [' + currentVersion(root) + '] - ';
  const start = contents.indexOf(heading);
  const next = contents.indexOf('\n## [', start + heading.length);
  assert.ok(start >= 0 && next > start, 'the fixture needs a following release section');
  const section = contents.slice(start, next).replace(/^- /gm, '* ');
  writeFileSync(path, contents.slice(0, start) + section + contents.slice(next));
  assert.equal(check(root).status, 0);
  const result = check(root, ['--publish']);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /published release notes must include an actual change/);
});

test('publication ignores draft wording outside the selected release section', (t) => {
  const root = finalizedFixture(t);
  const path = join(root, 'CHANGELOG.md');
  const contents = readFileSync(path, 'utf8');
  writeFileSync(path, contents.replace('## [Unreleased]', '## [Unreleased]\n\n> **Not released:** Future work.'));
  const result = check(root, ['--publish']);
  assert.equal(result.status, 0, result.stderr);
});

test('publication rejects duplicate release headings', (t) => {
  const root = fixture(t);
  const path = join(root, 'CHANGELOG.md');
  const contents = readFileSync(path, 'utf8');
  const version = currentVersion(root);
  const heading = contents.split(/\r?\n/).find((line) => line.startsWith('## [' + version + '] - '));
  assert.ok(heading);
  writeFileSync(path, contents + '\n' + heading + '\n\n- Duplicate.\n');
  const result = check(root, ['--publish']);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /published releases need exactly one matching changelog heading/);
});

test('publication mode rejects unsupported checker arguments', (t) => {
  const result = check(fixture(t), ['--publsh']);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /accepts only the optional --publish argument/);
});

test('rejects a missing Windows executable and installer publisher', (t) => {
  const root = fixture(t);
  editJson(root, 'apps/desktop/src-tauri/tauri.conf.json', (config) => {
    delete config.bundle.publisher;
  });
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /desktop Tauri publisher must be RepoDNA/);
});

test('rejects the old inferred github publisher', (t) => {
  const root = fixture(t);
  editJson(root, 'apps/desktop/src-tauri/tauri.conf.json', (config) => {
    config.bundle.publisher = 'github';
  });
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /desktop Tauri publisher must be RepoDNA/);
});

test('rejects an outdated personal publisher instead of the RepoDNA company', (t) => {
  const root = fixture(t);
  editJson(root, 'apps/desktop/src-tauri/tauri.conf.json', (config) => {
    config.bundle.publisher = 'SanskarIN';
  });
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /desktop Tauri publisher must be RepoDNA/);
});

test('rejects a spaced publisher instead of the exact RepoDNA name', (t) => {
  const root = fixture(t);
  editJson(root, 'apps/desktop/src-tauri/tauri.conf.json', (config) => {
    config.bundle.publisher = 'Repo DNA';
  });
  const result = check(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /desktop Tauri publisher must be RepoDNA/);
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
