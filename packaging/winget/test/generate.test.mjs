import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdtempSync, mkdirSync, readFileSync, symlinkSync, unlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';

const script = resolve('packaging/winget/generate.mjs');
const version = '1.4.1';
function prepare() {
  const root = mkdtempSync(join(tmpdir(), 'repodna-winget-'));
  const archives = join(root, 'archives');
  const out = join(root, 'out');
  mkdirSync(archives);
  for (const target of ['x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc']) {
    const name = 'repodna-' + version + '-' + target;
    const dir = join(root, name);
    mkdirSync(dir);
    writeFileSync(join(dir, 'repodna.exe'), 'portable exe fixture');
    execFileSync('zip', ['-q', '-r', join(archives, name + '.zip'), name], { cwd: root });
  }
  for (const arch of ['x64', 'arm64']) {
    writeFileSync(join(archives, 'RepoDNA_' + version + '_' + arch + '_en-US.msi'), 'msi fixture ' + arch);
  }
  return { root, archives, out };
}
function run({ archives, out }, v = version) {
  return spawnSync(process.execPath, [script, '--version', v, '--archives', archives, '--out', out], { encoding: 'utf8' });
}
test('generates two complete and correctly hashed multi-file manifests', () => {
  const paths = prepare();
  const result = run(paths);
  assert.equal(result.status, 0, result.stderr);
  for (const id of ['Sanskar.RepoDNA', 'Sanskar.RepoDNA.CLI']) {
    const dir = join(paths.out, 'manifests', 's', ...id.split('.'), version);
    const installer = readFileSync(join(dir, id + '.installer.yaml'), 'utf8');
    const metadata = readFileSync(join(dir, id + '.locale.en-US.yaml'), 'utf8');
    const versionFile = readFileSync(join(dir, id + '.yaml'), 'utf8');
    assert.match(installer, /Architecture: x64/);
    assert.match(installer, /Architecture: arm64/);
    assert.match(metadata, /ManifestType: defaultLocale/);
    assert.match(versionFile, /DefaultLocale: en-US/);
    assert.ok(versionFile.startsWith('# yaml-language-server: $schema=https://aka.ms/winget-manifest.version.1.12.0.schema.json\n'));
    assert.ok(metadata.startsWith('# yaml-language-server: $schema=https://aka.ms/winget-manifest.defaultLocale.1.12.0.schema.json\n'));
    assert.ok(installer.startsWith('# yaml-language-server: $schema=https://aka.ms/winget-manifest.installer.1.12.0.schema.json\n'));
    assert.match(installer, /ManifestType: installer/);
    assert.match(installer, /ManifestVersion: 1\.12\.0/);
    const filenames = id.endsWith('.CLI')
      ? ['repodna-' + version + '-x86_64-pc-windows-msvc.zip', 'repodna-' + version + '-aarch64-pc-windows-msvc.zip']
      : ['RepoDNA_' + version + '_x64_en-US.msi', 'RepoDNA_' + version + '_arm64_en-US.msi'];
    for (const file of filenames) {
      const hash = createHash('sha256').update(readFileSync(join(paths.archives, file))).digest('hex').toUpperCase();
      assert.ok(installer.includes('InstallerSha256: ' + hash));
      assert.ok(installer.includes('/releases/download/v' + version + '/' + file));
    }
    if (id.endsWith('.CLI')) {
      assert.match(installer, /NestedInstallerType: portable/);
      assert.match(installer, /PortableCommandAlias: repodna/g);
      assert.match(installer, /repodna-1\.4\.1-aarch64-pc-windows-msvc\/repodna\.exe/);
    } else {
      assert.match(installer, /InstallerType: msi/);
      assert.doesNotMatch(installer, /NestedInstallerFiles/);
    }
  }
});
test('refuses missing release files', () => {
  const paths = prepare();
  const bad = run({ archives: join(paths.root, 'missing'), out: paths.out });
  assert.notEqual(bad.status, 0);
  assert.equal(existsSync(paths.out), false, 'preflight must not write incomplete YAML');
});
test('refuses malformed or prerelease versions', () => {
  const paths = prepare();
  for (const value of ['1.4.1-alpha', '../1.4.1', 'v1.4.1']) {
    assert.notEqual(run(paths, value).status, 0);
  }
});

test('rejects a portable ZIP that is missing the nested executable', () => {
  const paths = prepare();
  const name = 'repodna-' + version + '-x86_64-pc-windows-msvc';
  execFileSync('zip', ['-d', join(paths.archives, name + '.zip'), name + '/repodna.exe']);
  const result = run(paths);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /missing.*repodna\.exe/);
});

test('rejects unsafe relative paths in portable ZIP files', () => {
  const paths = prepare();
  const name = 'repodna-' + version + '-x86_64-pc-windows-msvc';
  writeFileSync(join(paths.root, 'outside'), 'not part of this release');
  execFileSync('zip', [
    '-q', join(paths.archives, name + '.zip'), '../outside',
  ], { cwd: join(paths.root, name) });
  const result = run(paths);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /unsafe ZIP entry/);
});

test('rejects a zero-length installer rather than publishing its hash', () => {
  const paths = prepare();
  writeFileSync(join(paths.archives, 'RepoDNA_' + version + '_arm64_en-US.msi'), '');
  const result = run(paths);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Empty or non-file release artifact/);
});

test('rejects out-of-folder ZIP entries before creating partial manifests', () => {
  const paths = prepare();
  const name = 'repodna-' + version + '-x86_64-pc-windows-msvc';
  writeFileSync(join(paths.root, 'unrelated.txt'), 'not part of the portable package');
  execFileSync('zip', ['-q', join(paths.archives, name + '.zip'), 'unrelated.txt'], {
    cwd: paths.root,
  });
  const result = run(paths);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /unsafe ZIP entry/);
  assert.equal(existsSync(paths.out), false, 'invalid archives must not emit YAML');
});

test('rejects duplicate case-insensitive ZIP entry names', () => {
  const paths = prepare();
  const name = 'repodna-' + version + '-x86_64-pc-windows-msvc';
  writeFileSync(join(paths.root, name, 'REPODNA.EXE'), 'duplicate casing');
  execFileSync('zip', ['-q', join(paths.archives, name + '.zip'), name + '/REPODNA.EXE'], {
    cwd: paths.root,
  });
  const result = run(paths);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /duplicate ZIP entry/);
  assert.equal(existsSync(paths.out), false);
});

test('rejects a portable ZIP symlink before generating manifests', {
  skip: process.platform === 'win32',
}, () => {
  const paths = prepare();
  const name = 'repodna-' + version + '-x86_64-pc-windows-msvc';
  symlinkSync('repodna.exe', join(paths.root, name, 'alias'));
  execFileSync('zip', ['-q', '-y', join(paths.archives, name + '.zip'), name + '/alias'], {
    cwd: paths.root,
  });
  const result = run(paths);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /contains a symbolic link/);
  assert.equal(existsSync(paths.out), false);
});

test('checks ZIP payload integrity, not only filename listings', () => {
  const paths = prepare();
  const name = 'repodna-' + version + '-x86_64-pc-windows-msvc';
  const filepath = join(paths.archives, name + '.zip');
  unlinkSync(filepath);
  execFileSync('zip', ['-q', '-0', '-r', filepath, name], { cwd: paths.root });
  const data = readFileSync(filepath);
  const offset = data.indexOf(Buffer.from('portable exe fixture'));
  assert.ok(offset >= 0, 'stored ZIP fixture must expose test payload');
  data[offset] ^= 1;
  writeFileSync(filepath, data);
  const result = run(paths);
  assert.notEqual(result.status, 0);
  assert.equal(existsSync(paths.out), false);
});

test('rejects Windows device names, alternate streams, and trailing-dot archive entries', {
  skip: process.platform === 'win32',
}, () => {
  for (const badName of ['CON.txt', 'COM¹.txt', 'LPT²', 'bad<name', 'bad|name', 'bad"quote', 'readme:payload', 'notes.', 'trailing ']) {
    const paths = prepare();
    const name = 'repodna-' + version + '-x86_64-pc-windows-msvc';
    writeFileSync(join(paths.root, name, badName), 'invalid Windows path');
    execFileSync('zip', [
      '-q', join(paths.archives, name + '.zip'), name + '/' + badName,
    ], { cwd: paths.root });
    const result = run(paths);
    assert.notEqual(result.status, 0, badName);
    assert.match(result.stderr, /contains an unsafe ZIP entry/, badName);
    assert.equal(existsSync(paths.out), false, badName);
  }
});
