# Repository-balanced native ranker evaluation

RepoDNA's native evidence ranker is a small offline model, not a generative
language model. The frozen-checkpoint evaluator now reports both
**question-weighted** retrieval scores and **repository-balanced** retrieval
scores. Different repositories may contain very different numbers of labeled
questions. Query-weighted results can look good even when a model performs
poorly on smaller codebases.

## Run after freezing the checkpoint

Use independent, reviewed, repository-disjoint datasets:

~~~sh
cargo run --locked -p repodna-ai --bin evaluate-repodna-ranker -- \
  MODEL.json TRAIN.json TEST.json
~~~

The report's existing `retrieval` section averages MRR and NDCG@5 over
all answerable repository/question groups. The new
`repositoryBalancedRetrieval` section instead:

1. Normalizes repository identifiers by trimming surrounding whitespace
   and lowercasing (the same rule as repository leakage checks).
2. Computes each repository's retrieval scores over its own answerable
   questions, alongside original evidence-order baselines.
3. Gives every repository **with at least one answerable question** equal
   weight, regardless of how many labeled questions it contributes.
4. Separately counts repositories with only unanswerable questions. They
   are excluded from ranking metric denominators rather than scored zero.

Both pointwise and pairwise checkpoints support these ordering metrics.
The output includes repository count, contributing repository count,
unanswerable-only repository count, NDCG@5 and mean reciprocal rank,
their original-order baselines and signed differences. No individual
repository ID, question, evidence reference or judgment is printed.
Totals and performance summaries can still be sensitive: inspect before
publishing reports from private datasets.

## Interpret alongside existing results

If repository-weighted and question-weighted scores differ, investigate
dataset composition and whether many questions from one codebase dominate
the summary. Neither number verifies accuracy of explanations, faithfulness
to evidence, license rights, clone-family separation or model fairness.

For example, if repository A has ten answerable queries and repository B
has one, question-weighting gives A ten times the weight of B; repository
balancing gives each one half. If a repository has **zero** answerable
labeled queries, it cannot contribute an NDCG or reciprocal-rank score.
The separate count remains visible and should be discussed. Both means
are zero when no repository contributes, not evidence of model competence.

**Do not select or tune a model on final-test metrics.** Use validation
NDCG@5 to choose a model, freeze the decision, then evaluate the untouched
clone-family-disjoint final set once. Repository-balanced metrics are
diagnostic, not a new selection criterion. The synthetic demo JSON
files validate mechanics only and are not a real-world benchmark.
