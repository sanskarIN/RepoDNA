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

test('tag publications refuse provisional changelog wording before platform builds', () => {
  const prepare = job('prepare');
  // The same tag selection drives both the publication preflight and the
  // version job; release/v* branch trials must not set a publication tag.
  assert.match(
    prepare,
    /RELEASE_TAG: \$\{\{ github\.event_name == 'push' && github\.ref_type == 'tag' && github\.ref_name \|\| inputs\.tag \}\}/,
  );
  assert.match(prepare, /if \[ -n "\$RELEASE_TAG" \]; then/);
  assert.match(prepare, /node packaging\/release\/check-version\.mjs --publish/);
  assert.match(prepare, /if \[ -n "\$TAG" \]; then/);
  assert.match(prepare, /publish=false/);
});
