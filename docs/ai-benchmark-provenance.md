# Reproducible local AI benchmark identities

RepoDNA's native evidence ranker can be evaluated against distinct training,
validation and final-test repository sets. This workflow records **the exact
typed inputs and selected checkpoint identity**, not just filenames that may
be overwritten later.

## Create a versioned report

Run the model comparison and **save its stdout as JSON**:

```sh
cargo run --locked -p repodna-ai --bin compare-repodna-rankers -- \
  TRAIN.json VALIDATION.json TEST.json MODEL.json FAMILIES.json \
  > benchmark-report.json
```

The final `FAMILIES.json` argument is optional for compatibility; for
research claims about independent codebases, curate and supply it.
Use actual reviewed labels; the included synthetic files test the pipeline
only, not real-world model quality.

New reports use `provenanceSchema: "repodna-benchmark-provenance/3"`
and include content-based SHA-256 values:
`trainingDatasetSha256`, `validationDatasetSha256`,
`finalTestDatasetSha256`, and `checkpointSha256`. When provided, the
reviewer-curated family file is represented by `familyManifestSha256`,
`familyManifestChecked: true`, and aggregate `familySplit` counts.

The existing **content** fingerprints use complete, typed label records sorted
into canonical order: JSON indentation and annotation-array order do not change
those hashes. In v3, `validationOrderSha256` and `finalTestOrderSha256`
also bind the exact **typed JSON array order** of the validation and final-test
labels. Changing whitespace or JSON key order does not change either order
fingerprint, but reordering evidence records does. This matters because the
original-order baseline and stable score ties can depend on candidate order
within a repository/question. Training continues using its canonical
order-insensitive fingerprint, because training sorts examples itself.
The manifest fingerprint normalizes repository/family IDs,
revision hex casing and record order; license text remains
case-sensitive. The checkpoint fingerprint includes model format, every
weight, and training metadata independent of file indentation.

The fingerprint formats use separate domains so a model hash cannot be
confused with a family manifest hash. This does not turn them into
authenticated signatures.

### Repository sensitivity and evaluation order in benchmark v3

The v2-and-later `finalTestRepositoryStability` field summarizes the final test
**only after** selecting the objective and epochs on validation NDCG@5.
It reports aggregate per-repository NDCG@5 ranges, numbers of repositories
improving or regressing against the original-order baseline, and the
range of equal-repository-weight NDCG@5 and baseline deltas when one
answerable repository is excluded in turn. Repositories with no relevant
labeled evidence are counted but excluded from these ranking score ranges.
With fewer than two answerable repositories, leave-one-out ranges are
`null`, not estimated zeros. The report still includes
`finalTestRepositoryBalancedRetrieval`.

These are sensitivity diagnostics, **not** confidence intervals, causal
evidence of generalization, a substitute for human review, or a reason
to select a model based on the final test. No repository IDs or raw
labels appear in the output. Keep potentially sensitive dataset
fingerprints private.

## Verify a saved benchmark later

```sh
cargo run --locked -p repodna-ai --bin verify-repodna-benchmark -- \
  benchmark-report.json MODEL.json \
  TRAIN.json VALIDATION.json TEST.json FAMILIES.json
```

Omit `FAMILIES.json` **only** if the original report also omitted it.
The verifier refuses missing manifests from reports that claim they were
checked. All local JSON inputs must be regular files, have fixed byte limits,
and are read with bounded streams even when their reported sizes become stale. Its default **fast identity-check mode** validates each input size,
all three repository split boundaries, the saved checkpoint format, complete
training label identity, each dataset fingerprint, curated family identity and
split counts when applicable, and consistency of the reported selection
metadata.

For a stronger independent check, append `--replay` as the **final
argument**, after `FAMILIES.json` if the benchmark uses one:

```sh
cargo run --locked -p repodna-ai --bin verify-repodna-benchmark -- \
  benchmark-report.json MODEL.json \
  TRAIN.json VALIDATION.json TEST.json FAMILIES.json --replay
```

Replay mode locally **retrains all ten pointwise/pairwise models** at the
original five epoch counts. It recomputes every candidate's validation
NDCG@5, evaluated-query counts, and pair-ordering diagnostics, verifies
the original deterministic winner and saved checkpoint weights within
a strict floating-point tolerance, then independently recalculates the
final-test retrieval metrics, original-order baseline, deltas and pair-order
agreement, equal-repository-weight final-test retrieval metrics, and
(for v2 reports) per-repository stability and leave-one-repository-out
ranges. It never uses final-test performance to select a model.

The mode rejects missing, additional or shuffled candidate fields, altered
metric values, unsupported epoch counts or a different winner. Tiny
platform-specific rounding differences in floating-point scores are allowed;
integer counts, candidate order, and metadata must match exactly.
Replay is more computationally expensive than identity-only verification.

Older v1 and v2 benchmark reports remain supported in both modes. Their
`finalTestRepositoryBalancedRetrieval` is replay-checked, but v1 reports
do not require the v2 `finalTestRepositoryStability` field. New v3 reports
and earlier v2 reports must include the stability object, or fast identity
verification fails.

In v3, **even fast identity verification** recomputes both evaluation-order
fingerprints and refuses missing or mismatched hashes. The verifier emits
`evaluationOrderVerified: true` only for v3. On v1/v2, this value is
`false`: the older sorted content hashes cannot establish that the exact
candidate row sequence is unchanged, even after a successful replay.
Retain the original files for those reports; do not silently represent
legacy verification as fully order-bound. Identity-only mode still **does
not independently verify numeric metric contents**; use `--replay` to
detect metric tampering.
The report schema version is metadata, not an authenticated signature,
so no version should be trusted without its original provenance.
Order fingerprints are also unsigned identifiers, not attestations of
reviewer independence or integrity against an attacker who can replace the
report and its inputs together. They ensure that accidental reorderings
cannot quietly pass the v3 identity check.

A successful `replayVerified: true` establishes that the saved evaluation
is **numerically reproducible from these local labels and this particular
training implementation**. It is not proof that these labels were honestly
reviewed, the checkpoint came from a trusted author, the declared
repository families are correct, or that the model is accurate in production.

The CLI prints only aggregate fingerprints and a boolean `verified`.
It does not print repository names, source excerpts, question text or
reviewer identities. However, hashes can still reveal that the same
dataset was used in two reports; handle them as potentially sensitive.

## What a successful verification does *not* prove

In default **fast identity-check mode**, verification establishes artifact
identity and report consistency. It does **not** independently reproduce
candidate trainings or the published NDCG scores.

With `--replay`, verification additionally reruns the ten candidate
trainings, checks the validation-based winner and recomputes final-test
metrics from the supplied local datasets. This establishes numerical
reproducibility under the checked implementation and reported tolerances,
**not** independent accuracy, author authenticity, dataset permissions,
reviewer agreement, fork-family truth or explanation factuality.
Training can still be biased or overfit if validation is repeatedly reused
or the final test guides subsequent work.

To test the weight-training procedure, additionally run
`reproduce-repodna-ranker` on the saved checkpoint and training labels
with the chosen epoch count. For honest generalization claims, freeze a
genuinely human-reviewed benchmark and retain an untouched final-test
set. Keep sensitive raw manifests and labels local.

All operations stay local; no third-party AI API, hosted model, or
pretrained weights are involved.
