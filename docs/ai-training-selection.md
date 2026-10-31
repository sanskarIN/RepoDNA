# Offline Native Ranker: Three-Way Model Selection

Status: experimental research tooling. This command trains only the local,
CPU-compatible **evidence relevance ranker**, not a generative language model.

## Why separate three repository groups?

Training updates the model weights. Validation selects an epoch count. A final
test checks the *already selected* checkpoint on repositories that were not
used for either training or validation.

Randomly splitting evidence rows from the same repository would leak repository
patterns across splits. Every split must use non-overlapping repository IDs.
IDs are normalized by trimming whitespace and lowercasing when overlap is
checked. Keep human labels reviewed, appropriately licensed, and free of secrets.

## Command

Prepare three JSON arrays of `LabeledEvidence`, with independent repository IDs:

- Training: positive and negative labels, from training repositories.
- Validation: questions with at least one relevant candidate, from different repositories.
- Final test: independently labeled questions, from repositories absent from both other splits.

```sh
cargo run --locked -p repodna-ai --bin select-repodna-ranker -- \
  examples/native-ai/train.demo.json \
  examples/native-ai/holdout.demo.json \
  examples/native-ai/test.demo.json \
  selected-local-ranker.json
```

The examples are **synthetic smoke tests**; do not report their scores as model
quality. For real work, prepare reviewed labels with
`repodna-ranker-labels` and keep each repository entirely within one split.

Selection fits five candidate models using 10, 30, 60, 100, and 150 epochs.
All candidates use the **same training data**. The tool selects the epoch
count with the highest validation NDCG@5, preferring fewer epochs on ties.
No final-test scores are used for selection. Only the selected checkpoint
is evaluated on the final test split, once, after selection.

The tool prints validation candidate scores, a validation original-order
baseline, and final-test retrieval metrics, original-order baseline,
retrieval deltas, and relevance-score diagnostics. These are measurements
against labels, not proof of code understanding or factual correctness.
The checkpoint includes a training dataset fingerprint and is written with
exclusive creation: it will not overwrite an existing file.

**Do not repeatedly rerun this command on the same final-test repositories
to choose new settings.** Reserve a fresh untouched final test for future
model families. Validation score improvements are not a guarantee of
performance on other codebases.

## Limits and security

All processing is offline. Nothing downloads pretrained weights, invokes
remote AI services, or executes repository source. Each JSON file is limited
to 8 MiB and 100,000 labeled examples. Duplicate evidence identifiers
within a repository/question, overlapping repository IDs and source
excerpts are rejected. Datasets remain on local storage; the console
report includes only aggregate statistics and a training fingerprint.
Because a fingerprint can reveal repeated use of the same training set,
review model reports before publishing them.

To audit whether checkpoint weights reproduce from the saved labels,
use `reproduce-repodna-ranker`, passing the reported `selectedEpochs`
as its third argument. Weight reproducibility is separate from model
accuracy, dataset provenance, and label correctness.


## Pair-ordering diagnostics

The selector additionally reports `validationPairOrdering` and
`testPairOrdering`: exact within-question agreement between the ordering of
human-labeled relevant and irrelevant evidence. These metrics measure how
often a positive item scores above a negative one. Tied scores contribute
half a correct pair; incomparable question groups are counted separately.

The metric uses all pairs but avoids constructing a quadratic array of
comparisons, making it suitable for bounded local datasets. Results are
**micro-averaged across pairs**, so questions with many labels contribute more
weight. Always examine `totalPairs` and `evaluatedQueries`, and compare
against original-order baselines and NDCG rather than optimizing a single
number. This measures ranking only, never factual truth, natural-language
faithfulness, answerability, or the fairness of reviewer labels.

The metric supports both the original pointwise ranker and later pairwise
checkpoints. The three-way selector still chooses epochs using validation
NDCG@5 alone; it never uses final-test pair ordering for training choices.
