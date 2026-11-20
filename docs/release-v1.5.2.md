# RepoDNA v1.5.2 — published stable release record

**Stable version:** [v1.5.2](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.5.2)  
**Published:** November 20, 2026 (14:51:16 UTC)  
**Status:** Public, latest stable; neither draft nor prerelease  
**Source tag:** [`v1.5.2`](https://github.com/sanskarIN/RepoDNA/tree/v1.5.2)  
**Exact tagged commit:** [`5832b9818c5cb657a9238180feefb72e0c6b2d8e`](https://github.com/sanskarIN/RepoDNA/commit/5832b9818c5cb657a9238180feefb72e0c6b2d8e)  
**Release assets:** 15 platform/install/WinGet files plus `SHA256SUMS.txt`, 16 total.

This file records a **completed public GitHub stable release**, not an
unpublished candidate. It complements the frozen v1.5.2 tag and does not
rewrite historical records for [v1.5.1](release-v1.5.1.md).

## Scope — patch from published v1.5.1

The v1.5.2 commit descends from the exact published v1.5.1 source
[`bbccf812148984ab45830c2c579729926cabf4f5`](https://github.com/sanskarIN/RepoDNA/commit/bbccf812148984ab45830c2c579729926cabf4f5).
It is six commits ahead and does **not** merge the feature-rich development
`main` branch. The v1.6.0 draft candidate [#282](https://github.com/sanskarIN/RepoDNA/pull/282)
was closed without merging when the maintainer selected v1.5.2.

### Fixes shipped

- Reject percent-encoded Git remote hostnames before downstream decoding can
  bypass literal host policy checks. Keep percent-encoded repository paths
  supported and preserve the explicit trusted-private-host opt-in for literal
  addresses.
- End the URL authority at the first path, query, or fragment delimiter to
  avoid confusing userinfo markers with the destination host.
- Remove URL queries, fragments and HTTP(S) credentials from displayed or
  persisted remote URL metadata; redact URL secrets echoed by Git during
  failed clones, including password-bearing SSH URL userinfo.
- Add Rust unit and CLI end-to-end regression coverage for the above cases.

**Not included:** The later branch/tag/commit/tree-reference analysis, folder
selection, and other features that appear on development `main`. The hosted
GitHub Pages site may update independently of tagged CLI and desktop binaries.
This patch is not a promise of complete SSRF prevention: remote DNS resolution
remains outside literal-host filtering.

## Official verified release inventory

The public [GitHub Release](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.5.2)
contains these **15 files** and a checksum list:

| Platform or purpose | Public asset filename |
| --- | --- |
| Windows x64 desktop setup | `RepoDNA_1.5.2_x64-setup.exe` |
| Windows x64 desktop MSI | `RepoDNA_1.5.2_x64_en-US.msi` |
| Windows ARM64 desktop setup | `RepoDNA_1.5.2_arm64-setup.exe` |
| Windows ARM64 desktop MSI | `RepoDNA_1.5.2_arm64_en-US.msi` |
| Windows x64 CLI | `repodna-1.5.2-x86_64-pc-windows-msvc.zip` |
| Windows ARM64 CLI | `repodna-1.5.2-aarch64-pc-windows-msvc.zip` |
| macOS universal desktop | `RepoDNA_1.5.2_universal.dmg` |
| macOS Intel CLI | `repodna-1.5.2-x86_64-apple-darwin.tar.gz` |
| macOS Apple silicon CLI | `repodna-1.5.2-aarch64-apple-darwin.tar.gz` |
| Linux Debian / Ubuntu x64 desktop | `RepoDNA_1.5.2_amd64.deb` |
| Linux RPM x64 desktop | `RepoDNA-1.5.2-1.x86_64.rpm` |
| Linux x64 static CLI | `repodna-1.5.2-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 static CLI | `repodna-1.5.2-aarch64-unknown-linux-musl.tar.gz` |
| WASI CLI WebAssembly | `repodna-1.5.2-wasm32-wasip1.zip` |
| WinGet community manifest bundle | `repodna-1.5.2-winget-manifests.zip` |

Also published: [`SHA256SUMS.txt`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.5.2/SHA256SUMS.txt).
Verify **the exact downloaded asset** against its published checksum
before installation. The official publisher produced this file from its
own uploaded candidates; GitHub exposes SHA-256 digests for the published
assets separately. The checksum ZIP from the earlier nonpublishing trial
belongs to a different build and must **not** be used to verify stable
downloads even if the filenames are identical.

## Qualification and release evidence

- [Maintenance candidate PR #283](https://github.com/sanskarIN/RepoDNA/pull/283):
  isolates the v1.5.2 patch from development `main`.
- [Exact-SHA CI #44748325759](https://github.com/sanskarIN/RepoDNA/actions/runs/44748325759):
  passed **17 of 17** jobs.
- [Exact-SHA CodeQL #44748325703](https://github.com/sanskarIN/RepoDNA/actions/runs/44748325703):
  passed all **four** analyses.
- [Nonpublishing full trial #44748314983](https://github.com/sanskarIN/RepoDNA/actions/runs/44748314983):
  passed all required platform, package, dependency-security, Windows installer,
  asset-inventory and SHA-256 verification gates, with `publish=false`.
- [Owner-authorized one-time tag gate #44750325952](https://github.com/sanskarIN/RepoDNA/actions/runs/44750325952):
  checked the exact candidate, created `v1.5.2` at the frozen commit and
  dispatched the official publisher. Its temporary workflow file was
  removed after successful dispatch.
- [Official stable publisher #44752365878](https://github.com/sanskarIN/RepoDNA/actions/runs/44752365878):
  succeeded on **attempt 2** and attached all **16 files**. A cancelled queued
  macOS ARM64 CLI job prevented attempt 1 from publishing; a retry completed
  the remaining job without a source-code change. The public release is
  marked **latest**, not draft and not prerelease.

The Windows build checks and Tauri publisher metadata were validated by CI.
Windows binaries and installers are **not Authenticode code-signed**; the
`RepoDNA` bundle publisher string alone is not a digital signature. As a
separate user-acceptance step, manually test MSI/EXE installation,
upgrade-from-v1.5.1, launch and uninstall on a physical or virtual Windows
system before claiming full real-device QA.

## Package-manager follow-up

- Official GitHub publication and WinGet indexing are different events.
  [WinGet submission #44757474107](https://github.com/sanskarIN/RepoDNA/actions/runs/44757474107)
  passed validation of the published Windows installers and six manifest files,
  but **failed to open upstream Microsoft pull requests** because its
  authorized GitHub token lacks the `workflow` scope (HTTP 422).
  Repair the submission permissions or avoid touching upstream workflow files,
  then retry; do not claim that WinGet 1.5.2 is indexed until both package
  IDs are verified in the public catalog. See [WinGet instructions](winget.md).
- The official release workflow succeeded at its npm and container-image jobs.
  Verify availability in each external registry separately before giving a
  platform-specific package-manager instruction as definitely indexed.

For instructions, see the [installation guide](installation.md),
[GitHub Action](github-action.md), [security policy](../SECURITY.md),
and [review/version boundaries](reviewing-changes.md).
