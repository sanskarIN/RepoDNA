# RepoDNA v1.5.2 — Official stable release

**Published:** November 20, 2026 (14:51:16 UTC)  
**Tag:** [`v1.5.2`](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.5.2)  
**Exact source:** [`5832b9818c5cb657a9238180feefb72e0c6b2d8e`](https://github.com/sanskarIN/RepoDNA/commit/5832b9818c5cb657a9238180feefb72e0c6b2d8e)  
**Status:** Published, latest stable, neither draft nor prerelease  
**Downloads:** 15 platform and manifest files plus `SHA256SUMS.txt` (16 files).

## What changed

RepoDNA v1.5.2 is a focused PATCH release based on v1.5.1.

- Reject percent-encoded Git remote hostnames before downstream clients could
  reinterpret a blocked private or literal host. Preserve encoded path names.
- Parse URL authority before query and fragment delimiters.
- Remove query parameters, fragments, and HTTP(S) credentials from stored,
  displayed, and Git error URL metadata; redact password-bearing SSH userinfo.
- Add Rust and CLI regression tests for these cases.

It does **not** include development-`main` branch, tag, commit or folder
selection features. Read the [published release notes](release-notes.md).

## Downloads and installation

Download files from the [official v1.5.2 GitHub Release](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.5.2).

| Platform | Desktop installer | CLI download |
| --- | --- | --- |
| Windows x64 | `RepoDNA_1.5.2_x64-setup.exe` or `RepoDNA_1.5.2_x64_en-US.msi` | `repodna-1.5.2-x86_64-pc-windows-msvc.zip` |
| Windows ARM64 | `RepoDNA_1.5.2_arm64-setup.exe` or `RepoDNA_1.5.2_arm64_en-US.msi` | `repodna-1.5.2-aarch64-pc-windows-msvc.zip` |
| macOS Intel / Apple silicon | `RepoDNA_1.5.2_universal.dmg` | `repodna-1.5.2-x86_64-apple-darwin.tar.gz` or `repodna-1.5.2-aarch64-apple-darwin.tar.gz` |
| Debian / Ubuntu x64 | `RepoDNA_1.5.2_amd64.deb` | `repodna-1.5.2-x86_64-unknown-linux-musl.tar.gz` |
| RPM-based Linux x64 | `RepoDNA-1.5.2-1.x86_64.rpm` | `repodna-1.5.2-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 | CLI only | `repodna-1.5.2-aarch64-unknown-linux-musl.tar.gz` |
| WASI | — | `repodna-1.5.2-wasm32-wasip1.zip` |

The release also includes `repodna-1.5.2-winget-manifests.zip`. The
initial WinGet submission failed on unrelated fork workflow-file permissions,
but a [manifest-only fix](https://github.com/sanskarIN/RepoDNA/pull/289)
allowed a [successful retry](https://github.com/sanskarIN/RepoDNA/actions/runs/44763093413).
Microsoft has [Desktop PR #485086](https://github.com/microsoft/winget-pkgs/pull/485086)
and [CLI PR #485087](https://github.com/microsoft/winget-pkgs/pull/485087).
The public WinGet catalog is **not confirmed** until Microsoft merges and indexes both versions.

Verify every downloaded file against
[`SHA256SUMS.txt`](https://github.com/sanskarIN/RepoDNA/releases/download/v1.5.2/SHA256SUMS.txt).
The binaries and installers are unsigned; unsigned apps may prompt warnings
from Windows SmartScreen and macOS Gatekeeper. See the
[installation guide](https://github.com/sanskarIN/RepoDNA/blob/main/docs/installation.md).

## Publication evidence

The exact candidate passed [CI (17/17)](https://github.com/sanskarIN/RepoDNA/actions/runs/44748325759),
[CodeQL (4/4)](https://github.com/sanskarIN/RepoDNA/actions/runs/44748325703),
and a [full nonpublishing trial](https://github.com/sanskarIN/RepoDNA/actions/runs/44748314983)
including checksum inventory. The [official publisher](https://github.com/sanskarIN/RepoDNA/actions/runs/44752365878)
passed on attempt 2; the first attempt had one cancelled queued Apple Silicon
CLI runner job, which succeeded on retry without source changes.

The screenshots and hosted web interface may be newer than this tagged
maintenance release. For the frozen source code, use the
[`v1.5.2` tag](https://github.com/sanskarIN/RepoDNA/tree/v1.5.2).
