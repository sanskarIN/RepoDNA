import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
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
  for (const id of ['SanskarIN.RepoDNA', 'SanskarIN.RepoDNA.CLI']) {
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
});
test('refuses malformed or prerelease versions', () => {
  const paths = prepare();
  for (const value of ['1.4.1-alpha', '../1.4.1', 'v1.4.1']) {
    assert.notEqual(run(paths, value).status, 0);
  }
});
