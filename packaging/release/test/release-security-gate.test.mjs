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
  // Finalization branch trials also check finalized notes, but never set TAG.
  assert.match(prepare, /if \\[ -n "\\$RELEASE_TAG" \\] \\|\\|/);
  assert.match(prepare, /release\\/v1\\.5\\.3-final-qualification-\\*/);
  assert.match(prepare, /node packaging\/release\/check-version\.mjs --publish/);
  assert.match(prepare, /if \[ -n "\$TAG" \]; then/);
  assert.match(prepare, /publish=false/);
});

test('v1.5.3 finalization trial runs native Windows CLI and installer integrity gates', () => {
  const native = job('trial-native-windows-cli');
  const windows = job('trial-windows-qa');
  const publish = job('publish');

  assert.match(native, /windows-11-arm/);
  assert.match(native, /RuntimeInformation\\]::OSArchitecture/);
  assert.match(native, /--no-store --quiet/);
  assert.match(native, /if: needs\\.prepare\\.outputs\\.publish == 'false'/);
  assert.match(windows, /needs: \\[prepare, trial-assets\\]/);
  assert.match(windows, /check-v1\\.5\\.3-windows-trial\\.ps1/);
  assert.match(windows, /if: needs\\.prepare\\.outputs\\.publish == 'false'/);
  assert.match(publish, /if: needs\\.prepare\\.outputs\\.publish == 'true'/);
});
