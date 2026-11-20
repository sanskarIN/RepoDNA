# RepoDNA v1.5.2 — release notes

**Status:** Public stable release, published November 20, 2026.  
**Version tag:** [`v1.5.2`](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.5.2)

RepoDNA v1.5.2 is a focused maintenance update from the v1.5.1 source,
preserving established CLI, web and desktop interfaces.

## Fixed

- Reject percent-encoded remote hostnames before Git can reinterpret their
  meaning, retaining encoded repository paths and explicit trusted literal hosts.
- Parse remote URL authority before query or fragment delimiters so stray
  userinfo markers cannot disguise the destination host.
- Remove URL queries, fragments and HTTP(S) credentials from remote URLs
  displayed or saved in local analysis metadata; scrub URL secrets echoed by
  Git in failure diagnostics (including password-bearing SSH URLs).

No new branch, tag, commit or directory analysis features from development
`main` are included in this maintenance patch.

**Downloads:** [GitHub Release](https://github.com/sanskarIN/RepoDNA/releases/tag/v1.5.2) —
15 binaries, installers and manifests plus `SHA256SUMS.txt`.
**Verification:** [CI](https://github.com/sanskarIN/RepoDNA/actions/runs/44748325759),
[CodeQL](https://github.com/sanskarIN/RepoDNA/actions/runs/44748325703),
[trial](https://github.com/sanskarIN/RepoDNA/actions/runs/44748314983),
[publisher](https://github.com/sanskarIN/RepoDNA/actions/runs/44752365878).
