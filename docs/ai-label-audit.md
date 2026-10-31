# Offline evidence-label dataset audit

RepoDNA can train native pointwise and pairwise evidence-ranking models using
locally reviewed labels. Before training, **inspect** the quality and split
structure of those datasets. Training can still learn from poor, copied, or
misleading labels if you do not check them.

```sh
cargo run --locked -p repodna-ai --bin audit-repodna-labels -- \
  examples/native-ai/train.demo.json \
  examples/native-ai/holdout.demo.json \
  examples/native-ai/test.demo.json
```

Use these **synthetic fixtures** only to test the tooling, not to claim a model
has learned from real codebases.

The audit enforces three nonempty repository-disjoint splits (train,
validation, final test). It checks canonical repository aliases, duplicate
evidence IDs, source-excerpt exclusions, input sizes, required label fields,
mixed positive/negative training examples within at least one question, and
answerable validation/final-test questions.

Its aggregate JSON report includes sample counts, independent repository
counts, question groups, positive and negative judgments, answerable and
unanswerable question counts, same-question positive/negative pairing
coverage, distribution across evidence kinds, maximum share of examples
from a single repository, and a SHA-256 fingerprint of each split's labels.

## Possible copied evidence across splits

An additional check compares the **exact question, evidence kind,
reference and detail** while deliberately ignoring repository IDs and
evidence IDs. If the same question and evidence text occurs in different
splits, the command reports *aggregate matching-signature counts* and
warns about possible cloned-repository or annotation leakage.

A matching signature is **not proof of leakage**. Independent repositories
can share familiar file and dependency metadata. Conversely, a copied
repository with slightly changed text or questions can pass this check.
These are warnings requiring human review, not hard failures.

When a dataset has fewer than five training repository IDs, fewer than
three validation or test repository IDs, extreme repository concentration,
or exact repeated evidence signatures, the tool warns. These thresholds
are simple research heuristics, **not statistical power guarantees**.

## Privacy and interpretation

The audit never prints raw repository identifiers, evidence details,
source paths, questions, or labels. It prints aggregate statistics and
SHA-256 dataset fingerprints. The latter can still reveal whether
the same labeled dataset was reused, so review before publishing reports.

**Passing structural checks is not evidence that labels were reviewed
correctly, that the repositories are truly independent, that a dataset is
licensed for training, or that the ranker generalizes.** A reviewer must
verify those properties independently, document repository provenance,
and avoid repeatedly modifying experiments based on an already used
final-test set.

Run the audit before using `compare-repodna-rankers`; examine warnings
and any suspicious repeated content before drawing conclusions from
validation or test metrics. All operations stay local, with a maximum
of 8 MiB and 100,000 labels per JSON file.
