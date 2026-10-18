#!/usr/bin/env node
// Create reproducible WinGet community manifests from the exact published artifacts.
// No external npm dependencies, downloads, or secrets are required.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { closeSync, lstatSync, mkdirSync, openSync, readSync, writeFileSync } from 'node:fs';
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
  const info = lstatSync(path);
  if (!info.isFile() || info.size === 0) {
    throw new Error('Empty or non-file release artifact: ' + path);
  }
  // Streaming-sized chunks avoid loading whole MSI installers into Node memory.
  const hash = createHash('sha256');
  const buffer = Buffer.allocUnsafe(1024 * 1024);
  const fd = openSync(path, 'r');
  try {
    let bytes;
    while ((bytes = readSync(fd, buffer, 0, buffer.length, null)) !== 0) {
      hash.update(buffer.subarray(0, bytes));
    }
  } finally {
    closeSync(fd);
  }
  return hash.digest('hex').toUpperCase();
}
function inspectZipPaths(filepath, filename, nestedFile) {
  // Release archives are untrusted inputs until their layout is verified.
  const entries = execFileSync('unzip', ['-Z1', filepath], {
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  }).split(/\r?\n/).filter(Boolean);
  const seen = new Set();
  for (const entry of entries) {
    const normalized = entry.replaceAll('\\', '/');
    const segments = normalized.split('/');
    if (
      !normalized ||
      normalized.startsWith('/') ||
      /^[A-Za-z]:/.test(normalized) ||
      segments.some((part) => part === '.' || part === '..') ||
      entry.includes('\\') ||
      entry.includes('\0')
    ) {
      throw new Error(filename + ' contains an unsafe ZIP entry: ' + entry);
    }
    const key = normalized.toLowerCase();
    if (seen.has(key)) {
      throw new Error(filename + ' contains a duplicate ZIP entry: ' + entry);
    }
    seen.add(key);
  }
  if (!seen.has(nestedFile.toLowerCase()) || !entries.includes(nestedFile)) {
    throw new Error(filename + ' is missing ' + nestedFile);
  }
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
    id: 'Sanskar.RepoDNA',
    name: 'RepoDNA',
    description: 'Local-first desktop repository intelligence and code archaeology.',
    kind: 'msi',
    installers: [
      { architecture: 'x64', file: 'RepoDNA_' + version + '_x64_en-US.msi' },
      { architecture: 'arm64', file: 'RepoDNA_' + version + '_arm64_en-US.msi' },
    ],
  },
  {
    id: 'Sanskar.RepoDNA.CLI',
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
      // Reject unsafe archive entries and require the exact portable binary path.
      inspectZipPaths(filepath, filename, nestedFile);
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
