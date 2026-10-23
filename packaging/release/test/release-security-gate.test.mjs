import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const workflow = readFileSync('.github/workflows/release.yml', 'utf8');

function job(name) {
  const marker = '\n  ' + name + ':\n';
  const start = workflow.indexOf(marker);
  assert.notEqual(start, -1, 'missing release job: ' + name);
  const from = start + marker.length;
  const next = workflow.slice(from).search(/\n  [a-z][a-z-]*:\n/);
  return workflow.slice(from, next === -1 ? undefined : from + next);
}

test('release dependency validation audits both Rust lockfiles', () => {
  const security = job('security');
  assert.match(security, /tree: \[workspace, desktop\]/);
  assert.match(security, /manifest-path: .*matrix\.tree/);
  assert.match(security, /command: check advisories bans licenses sources/);
  assert.doesNotMatch(security, /continue-on-error:/);
});

test('every release publishing path waits for the security gate', () => {
  for (const name of ['container', 'web-image', 'npm', 'publish']) {
    assert.match(job(name), /needs: \[[^\]]*\bsecurity\b[^\]]*\]/, name);
  }
  assert.match(job('catalog'), /needs: \[[^\]]*\bpublish\b[^\]]*\]/);
});
