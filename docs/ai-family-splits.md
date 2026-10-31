# Manual repository-family provenance for native AI benchmarks

A repository ID is **not** proof of independence. Two differently named
repositories can be a fork, a code mirror, a vendored copy, or a repackaged
version of the same project. If related repositories appear in training and
validation or final test, native evidence-ranking benchmarks can exaggerate
generalization.

RepoDNA supports an optional, **manually curated** repository-family manifest
for three-way training, validation and final-test datasets. The family validator
is strict when a manifest is supplied, but cannot discover forks automatically.

## Format

Prepare a local JSON array with **one record for every repository ID** across
all three label files:

```json
[
  {
    "repository": "my-project-primary",
    "family": "my-project-source-family",
    "revision": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "license": "NOASSERTION"
  },
  {
    "repository": "my-project-fork",
    "family": "my-project-source-family",
    "revision": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    "license": "NOASSERTION"
  }
]
```

This is a schema illustration, **not verified provenance**. In real use,
replace placeholder revision hashes with exact checked-out Git commit IDs,
and record the observed SPDX expression or `NOASSERTION`. Recording
`NOASSERTION` never grants training rights. Review ownership, licensing,
and annotation rights independently.

A family ID groups repositories known to share lineage, even when their
repository names are entirely different. Do not claim independence based
solely on having different GitHub repository URLs. Record the evidence for
family membership in a private review log, not in shared AI output.

The included
`examples/native-ai/families.demo.json` contains **synthetic metadata**
to exercise parsing and tests. It establishes nothing about real projects.

## Commands

Without a manifest, the original repository-ID checks remain in effect:

```sh
cargo run --locked -p repodna-ai --bin audit-repodna-labels -- \
  TRAIN.json VALIDATION.json TEST.json
```

For additional lineage checks, provide the manifest as an optional
**final positional argument**:

```sh
cargo run --locked -p repodna-ai --bin audit-repodna-labels -- \
  TRAIN.json VALIDATION.json TEST.json FAMILIES.json

cargo run --locked -p repodna-ai --bin compare-repodna-rankers -- \
  TRAIN.json VALIDATION.json TEST.json OUTPUT.json FAMILIES.json
```

The validator requires:

- Every labeled repository to have exactly one manifest record. Unused,
  missing, or duplicate case-/whitespace-normalized repository IDs fail.
- Repository IDs and family IDs to be nonempty and bounded in length.
- An exact 40- or 64-character hexadecimal Git revision and nonempty
  recorded license expression for every entry.
- **No family** to occur in more than one of training, validation, and
  final-test sets, even when repository identifiers differ.
- A maximum of 10,000 manifest entries and a 2 MiB file limit in both CLIs.

The commands reject invalid manifests **before** fitting candidate models,
exporting a checkpoint, or emitting a success report. The output includes
`familyManifestChecked` and an aggregate `familySplit` summary with
counts only. Raw repository and family names are not emitted.

If no manifest is passed, `familyManifestChecked` is `false` and
`familySplit` is `null`. This is backward compatibility, **not** proof
of independent repositories. Reproducible weight training and matching
dataset fingerprints similarly cannot prove independent lineage.

## Limitations and responsible reporting

This is a **curated claim enforcement mechanism**, not a content similarity
detector, fork detector, verification service, or cryptographic attestation.
A malicious, incomplete, or incorrect manifest can wrongly describe two
related repositories as independent. The separate exact-question/evidence
duplication audit may catch some copied content but is not sufficient
for clone verification.

For trustworthy results, independently review the lineage, revision,
rights and provenance of all evaluation repositories before freezing the
manifest. Protect the untouched final test set from training decisions.
Never publish private repository names, raw annotations, proprietary source
code, credentials or reviewer identities with a benchmark report.
