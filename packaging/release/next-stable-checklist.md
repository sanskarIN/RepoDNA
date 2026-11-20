# Next RepoDNA stable release — qualification record

> **Latest published stable:** [v1.5.2](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.5.2),
> November 20, 2026. It was published from the isolated v1.5.1 maintenance line.
> The [verified v1.5.2 publication record](../../docs/release-v1.5.2.md)
> is separate from this future-release checklist. The v1.5.2 tag must never be reused.

This checklist is for **a future release after v1.5.2**. No subsequent
version, exact source, date or feature set is approved. Require a fresh release
decision, exact-SHA CI, CodeQL, dependency and license/security checks, complete
cross-platform trial artifacts, checksums, and owner authorization before
publishing anything new.

## Published v1.5.2 checkpoint — November 20, 2026

- **Stable source:** `5832b9818c5cb657a9238180feefb72e0c6b2d8e`, tagged
  `v1.5.2`; 15 platform/manifests files and `SHA256SUMS.txt` were published.
- **Exact source CI:** [run #44748325759](https://github.com/sanskarIN/RepoDNA/actions/runs/44748325759)
  passed 17/17 jobs; [CodeQL #44748325703](https://github.com/sanskarIN/RepoDNA/actions/runs/44748325703)
  passed all 4 analyses.
- **Unpublished asset trial:** [run #44748314983](https://github.com/sanskarIN/RepoDNA/actions/runs/44748314983)
  succeeded with the full 15-file inventory and checksum verification.
- **Stable publisher:** [run #44752365878](https://github.com/sanskarIN/RepoDNA/actions/runs/44752365878)
  passed on attempt 2. A cancelled macOS arm64 runner job in attempt 1 was
  retried; no patch/source change was needed.
- **WinGet is not confirmed:** [submission #44757474107](https://github.com/sanskarIN/RepoDNA/actions/runs/44757474107)
  verified installers and manifests, but GitHub rejected the upstream fork
  push without the authorized token's `workflow` scope (HTTP 422). This is
  separate from the completed stable GitHub publication.
- The v1.6.0 source-only candidate PR [#282](https://github.com/sanskarIN/RepoDNA/pull/282)
  was closed without merging. The owner selected a v1.5.2 patch instead.
  Development `main` has additional features **not included in the patch**.

The following earlier analysis is retained as a **historical pre-publication
snapshot**, not the current stable-release status.

## Historical pre-v1.5.2 qualification snapshot (November 20, 2026)

This section supersedes historical branch/PR statuses below; historical evidence
remains for auditability. The target is **release qualification**, not publication.

- **Latest public stable:** `v1.5.1`, released November 16, 2026 from the
  isolated maintenance commit `bbccf812148984ab45830c2c579729926cabf4f5`.
  The public release contains 15 platform artifacts and `SHA256SUMS.txt`.
  The v1.5.1 release-candidate PR
  [#276](https://github.com/sanskarIN/RepoDNA/pull/276) was closed without
  merging into `main` after the approved release was published.
- **Development `main` baseline:** `c4b2c0a6cf6043b9b89a3eb098cd56825a4d526a`,
  containing the merged revision-aware remote analysis
  [#278](https://github.com/sanskarIN/RepoDNA/pull/278).
  [Full CI run #44545069543](https://github.com/sanskarIN/RepoDNA/actions/runs/44545069543)
  passed on retry (attempt 2). Its first attempt was not green because a
  macOS GitHub Action job was cancelled. The
  [main CodeQL run #44545069790](https://github.com/sanskarIN/RepoDNA/actions/runs/44545069790)
  succeeded. Do not substitute either for future exact-candidate checks.
- **Pending Git URL hardening:** [#279](https://github.com/sanskarIN/RepoDNA/pull/279)
  adds percent-encoded hostname rejection with regression tests and security
  documentation. It was **open and unmerged** at this snapshot; a passed
  final-head CI and review are required before it can be called shipped.
- **Version metadata is not candidate-ready:** current `main` still declares
  `1.5.0` in root `Cargo.toml`, `package.json`, and Tauri config.
  A new trial requires a synchronized chosen version in the manifests and
  **both** Cargo lockfiles plus npm lockfile, a dated changelog entry, and
  `cargo metadata --locked` passing in both Rust workspaces. The Release
  workflow intentionally rejects a candidate that lacks those gates.
- **Still-open dependency PRs:** [#267](https://github.com/sanskarIN/RepoDNA/pull/267),
  [#268](https://github.com/sanskarIN/RepoDNA/pull/268),
  [#271](https://github.com/sanskarIN/RepoDNA/pull/271), and
  [#272](https://github.com/sanskarIN/RepoDNA/pull/272).
  The Rust and desktop changes overlap in dependency families and lockfiles;
  refresh them from current `main`, coordinate updates, and run both locked
  metadata/notices gates before choosing any merge.
- Earlier draft [#266](https://github.com/sanskarIN/RepoDNA/pull/266) and
  npm updates [#269](https://github.com/sanskarIN/RepoDNA/pull/269) and
  [#270](https://github.com/sanskarIN/RepoDNA/pull/270) **have been merged**
  since the historical snapshot below.

### Version and source decision (historical; now resolved for v1.5.2)

- **`v1.5.2` maintenance patch:** branch from the *published `v1.5.1` commit*,
  include only narrowly scoped, backportable fixes and requalify all release
  assets. Do not silently pull the feature-rich `main` branch into this patch.
- **`v1.6.0` next feature release:** branch from current `main` after a feature
  freeze, because new branch/tag/commit selection and browser routing features
  have shipped to source but not to stable binaries. Evaluate compatibility,
  dependency upgrades, and user-facing release notes before choosing this route.

The maintainer must choose which scope to ship, then freeze an exact commit.
The presence of a proposed version here is **not** approval to tag or publish.

## Historical pre-v1.5.1 audit snapshot (November 14, 2026)

- Stable tag: `v1.5.0`; main head at initial audit:
  `21320619e8f8156cfcca83718004a23729139002`.
- Unreleased work already merged into `main` is described under
  [CHANGELOG.md — Unreleased](../../CHANGELOG.md), including GitHub branch-root
  URL handling, safer Git diagnostics/streaming, and a counts-only Findings CSV.
  These changes are **not** in the published v1.5.0 download.
- [#266](https://github.com/sanskarIN/RepoDNA/pull/266), literal-IP remote
  hardening, is a **draft**. Do not list it as shipped until it is reviewed,
  merged, and tested on the release candidate.
- [#267](https://github.com/sanskarIN/RepoDNA/pull/267),
  [#268](https://github.com/sanskarIN/RepoDNA/pull/268),
  [#269](https://github.com/sanskarIN/RepoDNA/pull/269),
  [#270](https://github.com/sanskarIN/RepoDNA/pull/270),
  [#271](https://github.com/sanskarIN/RepoDNA/pull/271), and
  [#272](https://github.com/sanskarIN/RepoDNA/pull/272)
  were open Dependabot changes at this snapshot. **Do not bulk-merge**:
  several touch overlapping Rust dependency families and two lockfiles.
- The failing `Web interface → Check the third-party notices` step on
  [PR #268's run](https://github.com/sanskarIN/RepoDNA/actions/runs/43551375945)
  is specifically blocked at `cargo metadata --locked` because
  `apps/desktop/src-tauri/Cargo.lock` would change. Treat the underlying
  problem as an out-of-sync desktop lockfile, not evidence that notice contents
  are necessarily inaccurate.
- [#173](https://github.com/sanskarIN/RepoDNA/issues/173) tracks independent
  human-reviewed native AI benchmark labels; no unsupported AI accuracy claims.

## A. Feature and security freeze

- [ ] Choose and record the release version, scope and exact merge cutoff.
- [ ] Reconcile each open dependency PR against **both** Cargo lockfiles and
  `package-lock.json`; document merge order and drop superseded updates.
- [ ] For Rust dependency changes, regenerate affected lockfiles with Cargo
  on a compatible toolchain, rerun `cargo metadata --locked` for the root
  workspace and the desktop workspace, and regenerate/check
  `THIRD-PARTY-NOTICES.txt` using `cargo xtask notices` as appropriate.
  Never weaken `--locked` or remove the notice check to make CI green.
- [ ] Resolve #266 (or explicitly exclude it from scope), including final-head
  CodeQL and full CI; preserve the documented DNS/rebinding limitations.
- [ ] Preserve secure handling of untrusted Git URLs, error strings and refs.
  Test `/tree/<branch>` links, slash-containing branch names, invalid refs,
  credential redaction, local-host rejection and explicit trusted-host opt-in.
- [ ] Test saved-analysis deletion without deleting user source repositories.
- [ ] Check that the counts-only Findings export contains no filenames,
  paths, snippets or evidence. Full exports still require explicit privacy review.
- [ ] Do not claim generative AI, independent native-model accuracy, or
  complete SSRF prevention without corresponding evidence.

## B. Final candidate commit: automated gates

Record the immutable candidate commit SHA here **after feature freeze**:
`<pending>`

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace --locked`
- [ ] `cargo metadata --format-version 1 --locked`
- [ ] `cargo metadata --manifest-path apps/desktop/src-tauri/Cargo.toml --format-version 1 --locked`
- [ ] `cargo xtask notices --check`
- [ ] `npm ci && npm run format:check && npm run typecheck && npm test`
- [ ] `npm run build` and the WebAssembly analysis tests; prove the browser
  build uses the locally bundled engine.
- [ ] Full GitHub CI and CodeQL succeed on this **exact commit**, including
  Windows/macOS/Linux CLI and desktop, web, WASM and GitHub Action jobs.
- [ ] Rust advisory, bans, license and source checks pass for **both**
  workspaces, plus the release HTTPS root trust smoke test.
- [ ] Check current dependency advisories and check that every artifact uses
  reviewed source and permitted license information.

## C. Release-specific metadata

For a candidate version (for example, `1.6.0` only after choosing feature-release
scope), preview synchronized **local** metadata updates with
`npm run release:prepare -- 1.6.0`; apply them explicitly with
`npm run release:prepare -- 1.6.0 --write`. The script validates both Rust
lockfile package inventories, npm workspace dependencies, and Tauri metadata
before writing. It intentionally does **not** update unrelated dependency
versions, regenerate third-party notices, create a changelog entry, tag code,
or publish a release. Add and review the dated changelog separately; then
re-run both Cargo locked-metadata commands, notice checks, and full CI.

- [ ] Set the final stable version consistently in the Rust workspace crates,
  desktop Rust/Tauri config, npm workspaces, both Cargo lockfiles and npm lock.
- [ ] Keep the Tauri identifier `io.github.sanskarin.repodna` stable, and
  Windows executable/installer `CompanyName`/publisher as **RepoDNA**.
- [ ] Add a dated `[X.Y.Z]` changelog entry including only verified,
  merged changes; clearly separate it from `[Unreleased]`.
- [ ] Update the tested What's New content, CLI and website docs, installer
  examples, installation instructions and supported downloads.
- [ ] Verify official support URLs, including
  `https://buymeacoffee.com/sanskarIN` and the exact
  `https://razorpay.me/@SanskarCodes`, without inserting obsolete URLs.
- [ ] Pass `npm run release:check`, `npm run links:check` and
  `npm run links:test` on the frozen candidate.
- [ ] Record the exact release tag, commit, workflow run IDs and artifact
  hashes; never carry over v1.5.0 checkmarks to a new build.

## D. Non-publishing trial and human acceptance

- [ ] Run the Release workflow on the exact frozen candidate with the
  `tag` input **empty**. Confirm the prepare job reports
  `publish=false`. **Do not** supply an existing release tag to a
  manual dispatch; that would enter the publishing path.
- [ ] Inspect every successful matrix job and its artifacts, not only the
  workflow's summary. Verify archives, checksums and architecture.
- [ ] Verify x64 and ARM64 Windows installer metadata, executable launch,
  clean install, update from v1.5.0, uninstall and user-data retention.
- [ ] Manually test Windows 11 desktop startup, window sizing, scrolling,
  keyboard access, import, analysis, cancel, export preview, deletion and
  What's New; also assess macOS and Linux launches.
- [ ] Test offline operation without telemetry, third-party AI keys,
  silent model downloads or mandatory code upload.
- [ ] Ensure no critical bug, security issue, blocked CI job or
  unreviewed breaking change is ignored merely to meet a target date.
- [ ] Publish a list of candidate assets and non-publishing trial links
  for maintainer testing. Collect explicit approval **after** trial QA.

## E. Publication: stop until maintainer approval

- [ ] **Maintainer explicitly approves the exact version and commit.**
  Preparation, a passing PR and a passing trial are not publication approval.
- [ ] Only then create the approved `vX.Y.Z` tag and verify its
  referenced commit/tree before allowing the tag-triggered workflow.
- [ ] Inspect the final GitHub Release, asset inventory and SHA256 checksums.
- [ ] Verify Windows ZIP/MSI/EXE, Linux DEB/RPM, macOS DMG and CLI artifacts.
- [ ] Confirm npm/container/WinGet indexing **independently** before
  announcing package-manager availability.

## Evidence and decision log

| Field | Verified value |
| --- | --- |
| Proposed version | Future version not selected; v1.5.2 already published |
| Frozen candidate SHA | Pending |
| Full CI run | Pending |
| CodeQL run | Pending |
| Non-publishing trial run | Pending |
| Windows/manual QA | Pending |
| Maintainer publication approval | Not granted for any future release beyond v1.5.2 |
| Public stable release | **v1.5.2 published**; future release not published |

Review this checklist against `main` again before a future release freeze; the
November 14 audit is historical and does not supersede the published v1.5.1 record.
