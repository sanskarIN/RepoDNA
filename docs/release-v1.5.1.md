# RepoDNA v1.5.1 — published stable release record

**Stable version:** [v1.5.1](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.5.1)  
**Published:** November 16, 2026  
**Status:** Public, latest stable, not a draft or prerelease  
**Source tag:** [`v1.5.1`](https://github.com/sanskarIN/RepoDNA/tree/v1.5.1)  
**Exact tag commit:** [`bbccf812148984ab45830c2c579729926cabf4f5`](https://github.com/sanskarIN/RepoDNA/commit/bbccf812148984ab45830c2c579729926cabf4f5)  
**GitHub Release assets:** 15 platform/install/manifest files plus `SHA256SUMS.txt` (16 attachments total)

This file records an **already published** release. The versioned source tag
is frozen and must not be changed to bring in later documentation or
features from `main`.

## What shipped

This **PATCH** release was built separately from the published v1.5.0
source; it does not contain newer, untagged feature development on `main`.

- Preserve saved analysis JSON artifacts when resetting the SQLite cache,
  with `repodna cache repair` support for restoring indexing (#221, #222).
- Refuse misleading cleanup when saved artifacts are unindexed.
- Reject arrays in required object positions of imported analysis artifacts (#264).
- Redact HTTP(S) credentials that appear in Git failure diagnostics (#229).
- Reject mixed-radix private IP aliases and special-use Git remote addresses
  by default, with explicit trusted-private-host opt-in (#233, #266).

**Not included:** GitHub branch-root `/tree/<ref>` analysis,
checked-out-branch UI, counts-only Findings exports, new AI evaluation work,
or unrelated post-v1.5.0 features on `main`.

For release notes, see the [published GitHub Release](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.5.1)
and [changelog](../CHANGELOG.md#151---2026-11-16).

## Release downloads

The release page has the following **verified 15-file inventory**:

| Platform / purpose | Asset |
| --- | --- |
| Windows x64 desktop setup | `RepoDNA_1.5.1_x64-setup.exe` |
| Windows x64 desktop MSI | `RepoDNA_1.5.1_x64_en-US.msi` |
| Windows ARM64 desktop setup | `RepoDNA_1.5.1_arm64-setup.exe` |
| Windows ARM64 desktop MSI | `RepoDNA_1.5.1_arm64_en-US.msi` |
| Windows x64 CLI | `repodna-1.5.1-x86_64-pc-windows-msvc.zip` |
| Windows ARM64 CLI | `repodna-1.5.1-aarch64-pc-windows-msvc.zip` |
| macOS universal desktop | `RepoDNA_1.5.1_universal.dmg` |
| macOS Intel CLI | `repodna-1.5.1-x86_64-apple-darwin.tar.gz` |
| macOS Apple silicon CLI | `repodna-1.5.1-aarch64-apple-darwin.tar.gz` |
| Debian / Ubuntu x64 desktop | `RepoDNA_1.5.1_amd64.deb` |
| RPM-based Linux x64 desktop | `RepoDNA-1.5.1-1.x86_64.rpm` |
| Linux x64 CLI | `repodna-1.5.1-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 CLI | `repodna-1.5.1-aarch64-unknown-linux-musl.tar.gz` |
| WASI WebAssembly | `repodna-1.5.1-wasm32-wasip1.zip` |
| WinGet community manifest bundle | `repodna-1.5.1-winget-manifests.zip` |

Verify each download against
[`SHA256SUMS.txt`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.5.1/SHA256SUMS.txt).
The release inventory and checksums were independently checked before
publication; the official publisher generated the public manifest.

## Qualification and publication evidence

- [PR #276 — frozen PATCH candidate and approval](https://github.com/sanskarIN/RepoDNA/pull/276).
- [Full candidate CI run #43819850096](https://github.com/sanskarIN/RepoDNA/actions/runs/43819850096):
  passed 17/17 jobs on the approved commit.
- [Candidate CodeQL #43819850068](https://github.com/sanskarIN/RepoDNA/actions/runs/43819850068)
  and [release-branch CodeQL #43819872979](https://github.com/sanskarIN/RepoDNA/actions/runs/43819872979):
  both passed.
- [Unpublished exact-SHA trial #43819873028](https://github.com/sanskarIN/RepoDNA/actions/runs/43819873028):
  succeeded, with 15/15 files and checksum verification.
- [Windows installation and upgrade QA #43821612954](https://github.com/sanskarIN/RepoDNA/actions/runs/43821612954):
  passed the v1.5.0-to-v1.5.1 installer upgrade, preservation and uninstall checks.
- [Approved tag/publication bootstrap #43825700079](https://github.com/sanskarIN/RepoDNA/actions/runs/43825700079):
  succeeded.
- [Official publishing workflow #43825718118](https://github.com/sanskarIN/RepoDNA/actions/runs/43825718118):
  completed successfully and published 16 attachments. The public GitHub
  release is marked latest, not draft, not prerelease.

The historical checklist within the frozen v1.5.1 source tag was written
**before publication** and therefore still describes an unpublished
candidate. This page and [PR #276](https://github.com/sanskarIN/RepoDNA/pull/276)
record the later explicit maintainer approval and completed publication.
Do not rewrite the version tag or merge the isolated maintenance candidate
into current `main` solely to change this historical status.

## Package-manager follow-up

GitHub publication and WinGet availability are separate. The automatic
[v1.5.1 WinGet submission #43827597415](https://github.com/sanskarIN/RepoDNA/actions/runs/43827597415)
passed public installer and manifest validation, then **failed to open
Microsoft community PRs** because the authorized token lacked GitHub's
`workflow` scope (HTTP 422). The token/workflow submission path must be
repaired and rerun; do **not** claim WinGet v1.5.1 is indexed until verified
from the public catalog. See [WinGet documentation](winget.md).

For instructions, use the [installation guide](installation.md), the
[GitHub Action guide](github-action.md), and [review/version boundary](reviewing-changes.md).
