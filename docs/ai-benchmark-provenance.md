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

The report now includes `provenanceSchema` and content-based SHA-256 values:
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

## Verify a saved benchmark later

```sh
cargo run --locked -p repodna-ai --bin verify-repodna-benchmark -- \
  benchmark-report.json MODEL.json \
  TRAIN.json VALIDATION.json TEST.json FAMILIES.json
```

Omit `FAMILIES.json` **only** if the original report also omitted it.
The verifier refuses missing manifests from reports that claim they were
checked. Its default **fast identity-check mode** validates each input size,
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
agreement. It never uses final-test performance to select a model.

The mode rejects missing, additional or shuffled candidate fields, altered
metric values, unsupported epoch counts or a different winner. Tiny
platform-specific rounding differences in floating-point scores are allowed;
integer counts, candidate order, and metadata must match exactly.
Replay is more computationally expensive than identity-only verification.

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

Verification establishes **artifact identity and report consistency**.
It does not independently reproduce all ten candidate trainings or the
published NDCG scores, confirm the model author, confirm license rights
or reviewer agreement, verify that reported fork relationships are true,
or establish explanation factuality. Training can still be biased or
overfit if validation is repeatedly reused or the final test guides
subsequent work.

To test the weight-training procedure, additionally run
`reproduce-repodna-ranker` on the saved checkpoint and training labels
with the chosen epoch count. For honest generalization claims, freeze a
genuinely human-reviewed benchmark and retain an untouched final-test
set. Keep sensitive raw manifests and labels local.

All operations stay local; no third-party AI API, hosted model, or
pretrained weights are involved.
