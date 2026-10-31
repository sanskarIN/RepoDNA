# Offline native ranker objective comparison

This research CLI compares RepoDNA's **pointwise** evidence relevance ranker
and experimental **pairwise** relevance ordering ranker. Both are small
locally trained CPU models, not language generators. The implementation
uses no pretrained models, hosted AI APIs, downloaded weights or external
inference runtimes.

## Prepare three independent repository sets

Create labeled JSON files of `LabeledEvidence`:

- `TRAIN.json`: reviewed positive and negative examples, including mixed
  positive/negative evidence for at least one repository/question.
- `VALIDATION.json`: independently reviewed judgments from different
  repositories, including questions with at least one relevant item.
- `TEST.json`: a separate, untouched final set of repositories with
  at least one answerable question.

Repositories must belong to **exactly one split**. Repository IDs are
canonicalized for overlap checks (trimmed and lowercased); duplicates
within a repository/question are rejected. Never place evidence from the
same repository in both training and validation or final test.

```sh
cargo run --locked -p repodna-ai --bin compare-repodna-rankers -- \
  examples/native-ai/train.demo.json \
  examples/native-ai/holdout.demo.json \
  examples/native-ai/test.demo.json \
  selected-evidence-ranker.json
```

These example files are **synthetic plumbing fixtures only**. Their scores
are not proof of model quality or generalization.

The selector fits 10 candidate models: pointwise and pairwise at 10, 30,
60, 100, and 150 epochs. It chooses the maximum **validation NDCG@5**,
preferring pointwise and then fewer epochs for exact ties. The CLI saves
the winning checkpoint with its format and training-data fingerprint,
and prints aggregate validation scores and per-query-group pair-ordering
agreement (correct/tied/incorrect relative order) for each candidate.
The extra pair-ordering statistics are **diagnostic only** and do not
change the configured selection criterion.

Only after selection is finalized does the tool measure the selected
checkpoint on the untouched final test, reporting retrieval metrics,
within-question pair-ordering agreement, and deltas from original evidence
order. Pair ordering is a micro-averaged metric: questions with many
positive-negative pairs contribute more weight; report pair counts and
evaluated question counts alongside it. The final test is **not**
used for objective, epoch, or threshold selection. To avoid selection
bias, do not use these results to retune and retest on the same
final-test repositories.

The selected file is written via exclusive creation and cannot
overwrite an existing checkpoint. Its training-data fingerprint binds
the included training examples but is not a digital signature or proof
of model quality.

You may audit deterministic training with:

```sh
cargo run --locked -p repodna-ai --bin reproduce-repodna-ranker -- \
  selected-evidence-ranker.json TRAIN.json SELECTED_EPOCHS
```

Use the actual `selectedEpochs` reported by the selector.

Pairwise scores are **relative ranking signals**, not calibrated
probabilities or claim-confidence values. Relevance ranking metrics
do not establish factual accuracy of explanations. Additional evaluation
is needed for citation faithfulness, real codebase generalization,
latency, memory usage, and privacy of reviewer-supplied labels.
All datasets stay local. The tool accepts at most 8 MiB and 100,000
labels per file. Review aggregate reports before sharing because even
dataset fingerprints may disclose repeated use of the same labels.
