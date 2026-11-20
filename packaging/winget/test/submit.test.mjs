import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  chmodSync, mkdtempSync, mkdirSync, readFileSync,
  rmSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import test from 'node:test';

const script = resolve('packaging/winget/submit.sh');

function setup(t, behind = '0') {
  const dir = mkdtempSync(join(tmpdir(), 'repodna-winget-fork-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const root = join(dir, 'manifest-root');
  for (const id of ['Sanskar.RepoDNA', 'Sanskar.RepoDNA.CLI']) {
    const folder = join(root, 's', ...id.split('.'), '1.5.2');
    mkdirSync(folder, { recursive: true });
    for (const suffix of ['installer', 'locale', 'version']) {
      writeFileSync(join(folder, suffix + '.yaml'), 'PackageIdentifier: ' + id + '\n');
    }
  }
  const bin = join(dir, 'bin');
  mkdirSync(bin);
  const log = join(dir, 'gh-calls.log');
  // gh API stub exercises the real submit.sh branch and PR creation code.
  // Upstream sync is forbidden: any attempted call fails the test.
  const mock = String.raw`#!/usr/bin/env node
const { appendFileSync } = require('node:fs');
const args = process.argv.slice(2);
const target = args.find(s => s.startsWith('repos/')) ?? (args.includes('user') ? 'user' : '');
appendFileSync(process.env.MOCK_GH_LOG, JSON.stringify(args) + '\n');
const reply = text => { process.stdout.write(text + '\n'); };
if (target.includes('/merge-upstream')) process.exit(80);
if (target === 'user') reply('sanskarIN');
else if (target === 'repos/sanskarIN/winget-pkgs') {
  reply(args.includes('.parent.full_name // ""')
    ? 'microsoft/winget-pkgs'
    : '{"parent":{"full_name":"microsoft/winget-pkgs"}}');
}
else if (target === 'repos/sanskarIN/winget-pkgs/git/ref/heads/master') {
  reply(args.includes('.object.sha') ? 'ancestral-fork-head' : '{"object":{"sha":"ancestral-fork-head"}}');
}
else if (target.includes('/compare/master...microsoft:master')) {
  reply(process.env.MOCK_BEHIND);
}
else if (target.includes('/git/commits/ancestral-fork-head')) reply('ancestral-tree');
else if (target.includes('/git/ref/heads/repodna-')) process.exit(1);
else if (target.includes('/git/trees') && args.includes('POST')) reply('generated-tree');
else if (target.includes('/git/commits') && args.includes('POST')) reply('manifest-commit');
else if (target.includes('/git/refs') && args.includes('POST')) reply('');
else if (target.includes('repos/microsoft/winget-pkgs/pulls?')) reply('');
else if (target === 'repos/microsoft/winget-pkgs/pulls' && args.includes('POST'))
  reply('https://github.com/microsoft/winget-pkgs/pull/example');
else {
  console.error('Unexpected gh call:', args);
  process.exit(81);
}
`;
  const fakeGh = join(bin, 'gh');
  writeFileSync(fakeGh, mock);
  chmodSync(fakeGh, 0o755);
  const env = {
    ...process.env,
    PATH: bin + ':' + process.env.PATH,
    GH_TOKEN: 'not-a-real-token-for-tests',
    MOCK_GH_LOG: log,
    MOCK_BEHIND: behind,
  };
  return { root, log, env };
}

function run(fixture) {
  return spawnSync('bash', [script, fixture.root, 'v1.5.2'], {
    env: fixture.env,
    encoding: 'utf8',
  });
}

function calls(fixture) {
  return readFileSync(fixture.log, 'utf8').split('\n').filter(Boolean).map(JSON.parse);
}

test('creates two manifest-only Microsoft PRs from ancestral fork without syncing workflow files', t => {
  const fixture = setup(t);
  const result = run(fixture);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Sanskar\.RepoDNA PR created/);
  assert.match(result.stdout, /Sanskar\.RepoDNA\.CLI PR created/);
  const commands = calls(fixture);
  assert.equal(commands.filter(parts => parts.join(' ').includes('/compare/master...microsoft:master')).length, 1);
  assert.equal(commands.filter(parts => parts.join(' ').includes('merge-upstream')).length, 0);
  assert.equal(commands.filter(parts => parts.includes('repos/microsoft/winget-pkgs/pulls') && parts.includes('POST')).length, 2);
  assert.equal(commands.filter(parts => parts.includes('repos/sanskarIN/winget-pkgs/git/trees') && parts.includes('POST')).length, 2);
});

test('rejects a diverged fork before making Git commits or upstream PRs', t => {
  const fixture = setup(t, '1');
  const result = run(fixture);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /fork master has commits not present on microsoft/);
  const commands = calls(fixture);
  assert.equal(commands.some(parts => parts.join(' ').includes('merge-upstream')), false);
  assert.equal(commands.some(parts => parts.includes('POST')), false);
});
