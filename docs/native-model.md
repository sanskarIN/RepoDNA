# RepoDNA Native Model — Development Plan

Status: **offline extractive native provider with an optional, locally trained
evidence-relevance ranker**; not a generative language model.

## Goal

Make RepoDNA explanations independent of cloud services, API keys, model runtimes,
and mandatory network access while retaining measurable, cited evidence.

## Current prototype (`provider = "native"`)

- Pure Rust, in-process evidence digest; no external model runtime, GPU, or
  automatic downloads. The default digest does not need model weights; users can
  explicitly opt into a validated, locally stored ranker checkpoint.
- Reuses bounded context selection, citation validation, cancellation and caching.
- Converts up to twelve selected facts into cited statements without inference.
- Does not execute repository commands or reproduce source excerpts.
- Clearly declares that it is not a trained generative model.
- Remains opt-in through user configuration; repository configuration cannot enable it.

## Phase 1 — Trainable evidence ranking

Create an original or appropriately licensed annotation dataset for relevance across
repository, architecture, dependencies, history, module, hotspot, and onboarding tasks.
Split training/validation/test by **repository**, avoiding commit or snippet leakage.
Implement a CPU-capable native ranker, a reproducible training/export pipeline,
and holdout tests against the current deterministic context order.

## Phase 2 — Learned language generation

Select a feasible model architecture **after** memory and latency benchmarks.
Train or fine-tune only with data whose licensing permits the intended use.
Use locally stored, versioned and verified model weights; never download
weights silently. Keep CPU inference and memory limits as release requirements.
Preserve structured citations, add semantic claim verification and abstention,
and fall back to the current extractive mode on unsupported hardware.

## Evaluation gates

Measure grounded claim precision with manual adjudication (a valid citation ID
does not prove a true claim), citation coverage, invalid references, abstention,
task relevance, prompt-injection resistance, privacy, cancellation, CPU latency,
peak RAM, installation size, and accuracy across unrelated codebases.

Do not label a ranking baseline as a generative LLM or report unmeasured accuracy.

## Try the keyless prototype

Put this in the *user* config discovered with `repodna config path`:

```toml
[ai]
provider = "native"
```

```sh
repodna explain --about repository
repodna explain --about dependencies --format markdown
repodna explain --dry-run
```

Trained neural checkpoints and generative text inference are **not included yet**.

## Experimental trainable ranker (developer tooling)

The experimental CPU-only linear relevance model can now be trained **locally** from
human-labeled examples, without pre-trained weights, an AI API or a GPU. It ranks
RepoDNA evidence; **it cannot generate language**, understand runtime behavior, or
prove whether a cited observation is true. It is opt-in within the native explanation provider: without a checkpoint, the
original safe evidence-selection order remains unchanged. A checkpoint only
reorders already selected evidence and cannot generate new text or facts.

Prepare two JSON arrays of LabeledEvidence, one for training and one for evaluation.
Every record needs repository, query, evidence (id, kind, reference, detail), and
a boolean relevant label. Repository IDs **must not overlap** between splits.
For leakage prevention, repository IDs are compared after trimming surrounding
whitespace and lowercasing; `repo-a`, ` Repo-A `, and `REPO-A` are treated
as the same repository. Use a stable, unique ID for each source repository.
The same normalization applies to duplicate-label detection and the training
repository count; it does not rewrite the input dataset.
Train only on data you have permission to use, and never include secrets.
Training sorts labeled examples by repository, exact question, and evidence ID
before deterministic SGD, so changing the JSON array's row order does not
change the resulting checkpoint weights. New checkpoints also store a SHA-256
fingerprint of the complete typed training records, sorted independently of
input row order and JSON whitespace. It includes relevance labels and evidence
metadata without embedding the raw records in the checkpoint. This does not make labels, rankings,
or checkpoint outputs independent of the actual training data.

```sh
cargo run --locked -p repodna-ai --bin train-repodna-ranker -- \
  examples/native-ai/train.demo.json \
  examples/native-ai/holdout.demo.json \
  ranker-local.json
```

The command writes a versioned JSON checkpoint and prints holdout classification
and per-question retrieval metrics (MRR, precision@5, recall@5, NDCG@5),
**alongside the original evidence-order baseline on the same holdout**. The `retrievalDelta` object reports signed
trained-minus-baseline differences for all four metrics (positive is better).
A positive delta measures label ranking on this holdout, **not** semantic
correctness or a guarantee of improvement on unseen repositories. Existing output files are not overwritten. The example datasets are
**synthetic pipeline smoke tests only**, not evidence of model quality.
Ranking metrics group examples by canonical repository ID (trimmed and
lowercased) and exact question; aliases of the same repository are one question
group, not separate opportunities to inflate the denominators. This applies to
the original-order baseline, learned retrieval metrics, and hypothetical
answerability diagnostics. Direct evaluation also rejects duplicate evidence
IDs across repository aliases and source excerpts. Groups without any
known relevant evidence are counted separately and excluded from ranking averages.
Precision@5 divides by five even when a group has fewer than five candidates.
These numbers do not establish that cited statements are correct.

The trainer includes `relevanceScoreDiagnostics` with held-out
Brier score (mean squared error against binary relevance labels), binary
log loss, and sample-weighted expected calibration error across ten fixed
relevance-score bands. Nonempty bands report sample count, average score,
and observed positive-label frequency. All metrics are **offline diagnostics**
of relevance labels; they do not measure factual truth, semantic confidence,
or claim correctness. Small or imbalanced holdout datasets produce unstable
estimates. Use a separate validation split for model selection, and keep the
final repository-disjoint test split untouched until evaluation.

The trainer also reports `answerability`, `answerableDetectionRate`, and
`unanswerableAbstentionRate`. These are **diagnostics** of a hypothetical
score-threshold rule (`0.5`): a question is predicted answerable when at
least one candidate evidence item scores at or above the threshold.
The `answerabilitySweep` array compares thresholds `0.25`, `0.5`, and
`0.75`, reporting query-level false positives and missed answerable queries
at each operating point. The Rust library can evaluate a caller-supplied
strictly increasing list of up to 32 finite thresholds between zero and one.
Use the sweep to understand sensitivity, **not** to choose a deployable
threshold from the final test set. Any operating-point selection needs a
separate, repository-disjoint validation set and an untouched test set. The report
counts true/false positives and negatives at the **repository/question** level,
including unanswerable questions omitted from retrieval averages. A threshold
score is not a calibrated probability of factual correctness, and RepoDNA
does **not** use these diagnostics to suppress answers at runtime. Always test
on independently labeled, repository-disjoint evaluation data.
Real validation must use many unrelated, clearly licensed repositories, reviewed
labels, human-measured citation correctness and representative tasks. Do not
publish accuracy claims from these synthetic samples.

The library API exposes deterministic feature extraction, logistic weight fitting,
checkpoint validation, relevance scoring, stable evidence ordering and
repository-disjoint evaluation. The model stays separate from code execution.

## Opt-in ranker inference in RepoDNA

After training and inspecting the checkpoint, add its **local path** to the user
configuration. No repository-supplied configuration may activate this option.

```toml
[ai]
provider = "native"
model = "/path/to/ranker-local.json"
```

The checkpoint is bounded to 64 KiB, validated before use and identified
by its content hash in explanation provenance and cache keys. Changes to
weights invalidate cached explanations. Source excerpts remain excluded
from the native digest even when the ranker is active. The checkpoint does
not turn RepoDNA into a generative language model.

## Prepare human-reviewed evidence labels (offline)

To train on real repositories, first produce an analysis artifact **locally**.
This workflow generates review templates from RepoDNA's existing evidence
selection; it never fetches code or assigns relevance automatically.

```sh
repodna analyze . --format json > repodna.json
cargo run --locked -p repodna-ai --bin repodna-ranker-labels -- \
  prepare repodna.json my-repo-id dependencies dependencies-unreviewed.json
```

Open `dependencies-unreviewed.json` and review **every** evidence item against
the question/task. Each `"relevant": null` must be explicitly changed to
`true` or `false`. An empty or missing label is never treated as negative.
The template contains no source excerpts, but analysis facts, paths and
descriptions can still be sensitive: inspect the file before distributing it.
Use only repositories and labels for which you hold appropriate rights.

Then save the edited file as `dependencies-reviewed.json` and finalize:

```sh
cargo run --locked -p repodna-ai --bin repodna-ranker-labels -- \
  finalize dependencies-reviewed.json dependencies-labeled.json
```

You may provide multiple reviewed input batches to `finalize` before the
output path. Duplicate evidence IDs within the same repository/question are
rejected. Files are written with exclusive creation and will **never
overwrite** existing output. Separate labeled examples into train and holdout
files by *repository ID*, not randomly by question, and ensure both positive
and negative labels occur in the training set. The trainer rejects repositories
that appear in both sets. Training and holdout datasets also reject duplicate
evidence IDs within the same repository/question, missing evidence IDs, and
source-excerpt evidence. The same evidence ID may still appear under a different
question or in a different repository. These checks prevent accidental double
weighting and contradictory labels, but cannot verify labeling accuracy.

Generate templates for other tasks using `repository`, `architecture`,
`dependencies`, `history`, `onboarding`, `module`, `hotspot`,
or `question`; the last three take a `SUBJECT` argument after the output
path. Example:

```sh
cargo run --locked -p repodna-ai --bin repodna-ranker-labels -- \
  prepare repodna.json my-repo-id question questions-unreviewed.json \
  "Where are the database dependencies?"
```

A reviewer must not label guessed functionality as factual evidence. More
examples are not a substitute for independently checked annotations, privacy
review, or a repository-disjoint evaluation set.

## Evaluate a frozen checkpoint on an untouched test split

After choosing training settings using separate **validation repositories**, evaluate
the saved checkpoint on an untouched, repository-disjoint final test set without
retraining or changing its weights:

```sh
cargo run --locked -p repodna-ai --bin evaluate-repodna-ranker -- \
  ranker-local.json train-reviewed.json test-reviewed.json
```

The command reads the saved checkpoint, original training labels, and test labels
**locally**, checks basic checkpoint provenance counts, validates that no canonical
repository ID occurs in both sets, and prints aggregate JSON diagnostics. This
includes classification, relevance-score diagnostics, retrieval metrics, original
evidence-order baseline and signed deltas, plus hypothetical answerability
thresholds. It never writes a new checkpoint or modifies the existing one.

Keep the **original training dataset** available for this check. New checkpoints
store `training_dataset_sha256` as a lowercase 64-character SHA-256 digest of
canonical typed records. The evaluator rejects mismatches even when repository
and sample counts still match; reordered JSON records and whitespace changes
do not change the digest. The report includes `trainingFingerprintVerified`.
Legacy checkpoints without this field still load, but evaluation explicitly
reports `false` because only basic training counts can be checked.

The digest binds the checkpoint's declared provenance metadata to the provided
labels. **It is not a digital signature and cannot prove honest training, model
weights, label quality, or dataset licensing.** Keep a separate experiment log
with training settings and trusted dataset hashes. Do not choose a threshold or
tune parameters using this final test result. A separate validation split is
required for those decisions.

The evaluator accepts at most 64 KiB of checkpoint JSON and 8 MiB per label file,
with a maximum of 100,000 examples in each input. Invalid checkpoints, duplicate
evidence IDs, source excerpts and overlapping repository identifiers are rejected.
The JSON output contains aggregate numbers, a verification flag and (from the
trainer) a dataset digest rather than raw paths or labels. A digest can reveal
whether two runs used identical labeled data, so review it before publishing.
The local input datasets may contain sensitive repository metadata. The results
measure labeled evidence relevance, **not** factual accuracy of natural-language
explanations.

## Reproduce a saved checkpoint from training labels

After training, you can verify whether a saved ranker agrees with a fresh,
deterministic fit from the **supplied** labeled training file. This operates
entirely offline and never modifies the checkpoint:

```sh
cargo run --locked -p repodna-ai --bin reproduce-repodna-ranker -- \
  ranker-local.json train-reviewed.json
```

The default **60 epochs** matches the current `train-repodna-ranker` CLI.
If a checkpoint was created using the library API with another epoch count,
pass it as a third argument (between 1 and 200). The command validates
checkpoint weights and training examples, checks the stored training counts,
fits the same CPU-only linear ranker again, and compares the learned weights
within a documented small floating-point tolerance. It exits with an error
and prints no JSON on a mismatch. It prints only aggregate metadata on success.

This is a **reproducibility check, not cryptographic dataset provenance**.
Different datasets can sometimes produce identical input features and weights;
a modified model could also be retrained or substituted deliberately.
The command cannot establish that the labels were originally used, are
correct, are properly licensed, or generalize to other repositories. Keep the
original training data and its independently recorded digest in your model
release records. Run frozen evaluation on a separate, untouched test split
to measure performance, not just reproducibility.

## Experimental pairwise evidence ranking

RepoDNA also offers an alternative **locally trained ranking objective** that
compares positive and negative evidence *for the same repository and question*.
This is a small CPU-only learning experiment using the existing 13 inspectable
features. It is **not** a generative AI model, and its scores are not calibrated
relevance probabilities or factual confidence.

```sh
cargo run --locked -p repodna-ai --bin train-repodna-pairwise -- \
  examples/native-ai/train.demo.json \
  examples/native-ai/holdout.demo.json \
  native-pairwise.json
```

The trainer creates a distinct `repodna-evidence-pairwise/1` checkpoint
format and reports holdout retrieval metrics alongside the original-order
baseline. It does **not** report classification accuracy or answerability
scores, since pairwise training does not calibrate absolute relevance scores.
The sample files are synthetic smoke tests, not model-quality evidence.

For production-oriented experimentation, split human-reviewed labels by
repository, use independent validation repositories to choose training
settings, and evaluate only once on an untouched final test. The separate
`select-repodna-ranker` command currently optimizes the original *pointwise*
training objective; it does not automatically compare pointwise and pairwise
training.

The pairwise trainer uses bounded, deterministic pairing with no cross-question
negatives, automatic downloads or external AI runtime. It rejects datasets
without any same-question positive/negative pair, prevents duplicate evidence
IDs and checks repository-disjoint holdouts. Saving a checkpoint does not
activate it: opt into local inference using `[ai] provider = "native"` and
`model = "/path/to/native-pairwise.json"` in your user config.

You can verify pairwise checkpoint weight reproducibility with the existing
`reproduce-repodna-ranker` CLI; specify the training epoch count (60 by
default). The checkpoint's format identifies the training objective.
Reproducing weights is not evidence of accuracy, labeling quality or
authentic data provenance.
