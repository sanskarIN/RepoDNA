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

New reports use `provenanceSchema: "repodna-benchmark-provenance/2"`
and include content-based SHA-256 values:
`trainingDatasetSha256`, `validationDatasetSha256`,
`finalTestDatasetSha256`, and `checkpointSha256`. When provided, the
reviewer-curated family file is represented by `familyManifestSha256`,
`familyManifestChecked: true`, and aggregate `familySplit` counts.

Dataset fingerprints use **complete, typed label records**, sorted into
canonical order: JSON indentation and annotation-array order do not change
the identity. The manifest fingerprint normalizes repository/family IDs,
revision hex casing and record order; license text remains
case-sensitive. The checkpoint fingerprint includes model format, every
weight, and training metadata independent of file indentation.

The fingerprint formats use separate domains so a model hash cannot be
confused with a family manifest hash. This does not turn them into
authenticated signatures.

### Repository sensitivity captured in benchmark v2

The new `finalTestRepositoryStability` field summarizes the final test
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

Older v1 benchmark reports remain supported in both modes. Their
`finalTestRepositoryBalancedRetrieval` is also replay-checked, but they
do not require the new v2 `finalTestRepositoryStability` field. Newly
created v2 reports **must** include that object: removing it causes
even fast identity verification to fail. Identity-only mode checks that
the required metric objects exist but **does not independently verify
their numeric contents**; use `--replay` to detect metric tampering.
The report schema version is metadata, not an authenticated signature,
so no version should be trusted without its original provenance.

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
