#!/usr/bin/env node
// Ensure release metadata is in sync before packaging any binaries.
// Uses only Node built-ins and never writes to the working tree.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const argumentsGiven = process.argv.slice(2);
assert.ok(
  argumentsGiven.length === 0 || (argumentsGiven.length === 1 && argumentsGiven[0] === '--publish'),
  'check-version.mjs accepts only the optional --publish argument',
);
const publish = argumentsGiven[0] === '--publish';

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
// Tauri otherwise derives the Windows CompanyName from the second label in
// "io.github.sanskarin.repodna" and incorrectly displays "github" as publisher.
// Keep the identifier itself stable so existing installations can upgrade.
const tauriConfig = json('apps/desktop/src-tauri/tauri.conf.json');
assert.equal(
  tauriConfig.bundle?.publisher,
  'RepoDNA',
  'desktop Tauri publisher must be RepoDNA, not inferred from the identifier',
);
assert.equal(
  tauriConfig.identifier,
  'io.github.sanskarin.repodna',
  'desktop Tauri identifier must remain stable for installer compatibility',
);
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
// Check the declared path dependencies as well as the lockfile. Cargo.lock
// can remain at the expected version while a forgotten workspace dependency
// constraint still points at an older release.
// Verify the entire known release-workspace dependency inventory, not merely a
// minimum count: losing one path dependency could otherwise leave enough
// entries to pass the check while breaking the packaged release.
const expectedInternalCrates = [
  'repodna-ai',
  'repodna-app',
  'repodna-architecture',
  'repodna-core',
  'repodna-dependencies',
  'repodna-discovery',
  'repodna-engine',
  'repodna-evolution',
  'repodna-git',
  'repodna-parser',
  'repodna-plugin',
  'repodna-project',
  'repodna-quality',
  'repodna-report',
  'repodna-security',
  'repodna-server',
  'repodna-store',
  'repodna-testkit',
];
const internalCrates = [...workspace.matchAll(/^(repodna-[\w-]+)\s*=\s*\{([^}\n]+)\}/gm)];
assert.deepEqual(
  internalCrates.map(([, name]) => name).sort(),
  expectedInternalCrates,
  'Cargo.toml: required internal workspace dependency inventory is missing, duplicated or unexpected',
);
for (const [, name, fields] of internalCrates) {
  assert.equal(
    fields.match(/\bpath\s*=\s*"([^"]+)"/)?.[1],
    'crates/' + name,
    'Cargo.toml workspace.dependencies ' + name + ' path is out of sync',
  );
  const dependencyVersion = fields.match(/\bversion\s*=\s*"([^"]+)"/)?.[1];
  if (name === 'repodna-testkit') {
    assert.equal(
      dependencyVersion,
      undefined,
      'Cargo.toml workspace.dependencies repodna-testkit must remain unversioned',
    );
  } else {
    assert.equal(
      dependencyVersion,
      version,
      'Cargo.toml workspace.dependencies ' + name + ' version is out of sync',
    );
  }
}
const desktop = read('apps/desktop/src-tauri/Cargo.toml');
assert.equal(
  desktop.match(/^version = "([^"]+)"/m)?.[1],
  version,
  'desktop Cargo.toml version is out of sync',
);
// Lockfile validation must cover the exact local package inventory, too.
// Checking 15+ entries would silently accept a missing crate, an unexpected
// repodna-* package, or a duplicated package name if versions still matched.
const expectedRootLockPackages = [
  ...expectedInternalCrates,
  'repodna-cli',
  'repodna-wasm',
  'xtask',
].sort();
const expectedDesktopLockPackages = [
  ...expectedInternalCrates.filter((name) => name !== 'repodna-testkit'),
  'repodna-desktop',
].sort();
for (const path of ['Cargo.lock', 'apps/desktop/src-tauri/Cargo.lock']) {
  const locked = read(path);
  const matches = [...locked.matchAll(/^name = "(repodna-[^"]+|xtask)"\r?\nversion = "([^"]+)"/gm)];
  const expected = path === 'Cargo.lock' ? expectedRootLockPackages : expectedDesktopLockPackages;
  assert.deepEqual(
    matches.map(([, name]) => name).sort(),
    expected,
    path + ': internal Rust package inventory is missing, duplicated or unexpected',
  );
  for (const [, name, lockedVersion] of matches) {
    assert.equal(lockedVersion, version, path + ': ' + name + ' is out of sync');
  }
}
const changelog = read('CHANGELOG.md');
assert.ok(
  changelog.includes('## [' + version + '] - '),
  'CHANGELOG.md needs a dated section for ' + version,
);

// Trial builds may deliberately contain a provisional changelog. Only a tag
// publication must prove that the shipped What's New and GitHub release notes
// describe a finalized, dated release. Never apply this gate to branch trials.
if (publish) {
  const heading = '## [' + version + '] - ';
  const headings = changelog.split(/\r?\n/).filter((line) => line.startsWith(heading));
  assert.equal(headings.length, 1, 'published releases need exactly one matching changelog heading');

  const releaseDate = headings[0].slice(heading.length);
  assert.match(releaseDate, /^\d{4}-\d{2}-\d{2}$/, 'published release needs an ISO date');
  const date = new Date(releaseDate + 'T00:00:00.000Z');
  assert.ok(
    !Number.isNaN(date.getTime()) && date.toISOString().slice(0, 10) === releaseDate,
    'published release date must be a real calendar day',
  );

  const start = changelog.indexOf(headings[0]) + headings[0].length;
  const afterHeading = changelog.slice(start);
  const nextHeading = afterHeading.search(/^## \[/m);
  const notes = nextHeading >= 0 ? afterHeading.slice(0, nextHeading) : afterHeading;
  assert.doesNotMatch(
    notes,
    /\b(?:not released|candidate preparation date|trial candidate|do not publish)\b/i,
    'published release notes must not contain provisional release wording',
  );
  assert.match(notes, /^- .+/m, 'published release notes must include an actual change');
}

console.log(
  'Verified RepoDNA ' + version + ' versions in Rust, Tauri, npm and both lockfiles' +
    (publish ? ' (publication notes finalized).' : '.'),
);
