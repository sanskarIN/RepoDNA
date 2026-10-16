#!/usr/bin/env node
// Create reproducible WinGet community manifests from the exact published artifacts.
// No external npm dependencies, downloads, or secrets are required.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const args = process.argv.slice(2);
function option(name) {
  const i = args.indexOf(name);
  if (i < 0 || !args[i + 1] || args[i + 1].startsWith('--')) {
    throw new Error('Missing required option ' + name);
  }
  return args[i + 1];
}
function sha256(path) {
  if (!statSync(path).isFile() || statSync(path).size === 0) {
    throw new Error('Empty or non-file release artifact: ' + path);
  }
  return createHash('sha256').update(readFileSync(path)).digest('hex').toUpperCase();
}
const version = option('--version');
if (!/^[0-9]+\.[0-9]+\.[0-9]+$/.test(version)) {
  throw new Error('Only stable numeric releases can be submitted to WinGet: ' + version);
}
const archives = option('--archives');
const output = option('--out');
const repo = 'https://github.com/sanskarIN/RepoDNA';
const baseUrl = repo + '/releases/download/v' + version + '/';
// Keep this aligned with microsoft/winget-pkgs/.github/PULL_REQUEST_TEMPLATE.md.
const schema = '1.12.0';

const variants = [
  {
    id: 'SanskarIN.RepoDNA',
    name: 'RepoDNA',
    description: 'Local-first desktop repository intelligence and code archaeology.',
    kind: 'msi',
    installers: [
      { architecture: 'x64', file: 'RepoDNA_' + version + '_x64_en-US.msi' },
      { architecture: 'arm64', file: 'RepoDNA_' + version + '_arm64_en-US.msi' },
    ],
  },
  {
    id: 'SanskarIN.RepoDNA.CLI',
    name: 'RepoDNA CLI',
    description: 'Local-first repository analysis command-line tool with a built-in web interface.',
    kind: 'zip',
    installers: [
      { architecture: 'x64', target: 'x86_64-pc-windows-msvc' },
      { architecture: 'arm64', target: 'aarch64-pc-windows-msvc' },
    ],
  },
];

function emit(packageId, name, manifestType, body) {
  const dir = join(output, 'manifests', 's', ...packageId.split('.'), version);
  mkdirSync(dir, { recursive: true });
  const path = join(dir, packageId + name + '.yaml');
  const schemaHeader = `# yaml-language-server: $schema=https://aka.ms/winget-manifest.${manifestType}.${schema}.schema.json`;
  writeFileSync(path, [schemaHeader, '', ...body, ''].join('\n'));
  console.log('Generated ' + path);
}
for (const pkg of variants) {
  const id = pkg.id;
  emit(id, '', 'version', [
    'PackageIdentifier: ' + id,
    'PackageVersion: ' + version,
    'DefaultLocale: en-US',
    'ManifestType: version',
    'ManifestVersion: ' + schema,
  ]);
  emit(id, '.locale.en-US', 'defaultLocale', [
    'PackageIdentifier: ' + id,
    'PackageVersion: ' + version,
    'PackageLocale: en-US',
    'Publisher: Sanskar',
    'PublisherUrl: https://sanskarin.github.io',
    'PublisherSupportUrl: ' + repo + '/issues',
    'PrivacyUrl: ' + repo + '/blob/main/PRIVACY.md',
    'Author: Sanskar',
    'PackageName: ' + pkg.name,
    'PackageUrl: ' + repo,
    'License: Apache-2.0',
    'LicenseUrl: ' + repo + '/blob/v' + version + '/LICENSE',
    'ShortDescription: ' + pkg.description,
    'Tags:',
    '  - developer-tools',
    '  - git',
    '  - code-analysis',
    'ReleaseNotesUrl: ' + repo + '/releases/tag/v' + version,
    'ManifestType: defaultLocale',
    'ManifestVersion: ' + schema,
  ]);
  const content = [
    'PackageIdentifier: ' + id,
    'PackageVersion: ' + version,
    'InstallerType: ' + pkg.kind,
  ];
  if (pkg.kind === 'zip') {
    content.push('NestedInstallerType: portable');
  }
  content.push('Installers:');
  for (const item of pkg.installers) {
    const filename = item.file || 'repodna-' + version + '-' + item.target + '.zip';
    const filepath = join(archives, filename);
    const hash = sha256(filepath);
    content.push(
      '  - Architecture: ' + item.architecture,
      '    InstallerUrl: ' + baseUrl + filename,
      '    InstallerSha256: ' + hash,
    );
    if (pkg.kind === 'zip') {
      const nestedFile = 'repodna-' + version + '-' + item.target + '/repodna.exe';
      // Fail instead of publishing a valid hash for an archive with a wrong layout.
      const files = execFileSync('unzip', ['-Z1', filepath], { encoding: 'utf8' })
        .split(/\r?\n/).filter(Boolean).map((p) => p.replaceAll('\\', '/'));
      if (!files.includes(nestedFile)) {
        throw new Error(filename + ' is missing ' + nestedFile);
      }
      content.push(
        '    NestedInstallerFiles:',
        '      - RelativeFilePath: ' + nestedFile,
        '        PortableCommandAlias: repodna',
      );
    }
  }
  content.push('ManifestType: installer', 'ManifestVersion: ' + schema);
  emit(id, '.installer', 'installer', content);
}
