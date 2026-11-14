# Next RepoDNA stable release — qualification record

> **Preparation only.** Latest published stable: [v1.5.0](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.5.0) (November 6, 2026).
> The next version, date, tag and feature set are **not yet approved**.
> Use **v1.5.1** for a compatible maintenance/security-only patch; consider
> **v1.6.0** only if reviewed, user-visible new features justify a minor release.
> Never re-publish v1.5.0 with post-release code.

This is a living, evidence-based checklist, not a declaration that anything below
passed. Check a box only after verifying the **exact frozen commit**. Keep historical
[v1.5.0 qualification](v1.5.0-checklist.md) intact.

## Audit snapshot (November 14, 2026)

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
| Proposed version | Pending scope decision |
| Frozen candidate SHA | Pending |
| Full CI run | Pending |
| CodeQL run | Pending |
| Non-publishing trial run | Pending |
| Windows/manual QA | Pending |
| Maintainer publication approval | **Not granted** |
| Public stable release | **Not published** |

Review this checklist against `main` again before release freeze; the audit
snapshot is not a promise that the same PRs, failures or versions remain current.
