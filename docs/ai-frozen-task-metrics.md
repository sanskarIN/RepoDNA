# Frozen model retrieval quality by task type

A native RepoDNA ranker can perform differently on build questions, dependency
questions, security findings and other task types. Overall NDCG@5 and MRR
alone may hide those differences. The **frozen checkpoint evaluator** supports
an optional, human-curated task manifest for the exact **final-test** split:

~~~sh
cargo run --locked -p repodna-ai --bin evaluate-repodna-ranker -- \
  MODEL.json TRAIN.json FINAL_TEST.json FINAL_TEST_TASKS.json
~~~

The first three arguments behave exactly as before. Omit the fourth argument
for the original report without task-stratified diagnostics.

## Strict manifest scope

The fourth argument is an array containing **one record for each unique
repository/question in FINAL_TEST.json**, with the exact same question and
a reviewer-supplied task category:

~~~json
[
  {
    "repository": "example-public-repo",
    "query": "Which build command is configured?",
    "task": "build_commands"
  }
]
~~~

This snippet is a **schema illustration**, not a labeled benchmark.
Categories are `architecture`, `dependencies`, `build_commands`,
`security`, `onboarding`, `history`, `hotspots` and `other`.
Question text is compared exactly; repository IDs are normalized by trimming
surrounding whitespace and lowercasing. The manifest is limited to 2 MiB
and 100,000 question records. Unknown categories, duplicates, missing or
unused questions fail without printing a partial report.

**Important:** The separate
[task-coverage audit](ai-task-coverage.md) can use a manifest for *all
three splits*. The frozen evaluator's optional argument instead uses the
**final-test-only subset** of that manifest; validation questions must not
appear in this final-test manifest.

## What the output means

The output gains `taskStratifiedRetrieval` with a versioned schema,
the count of reviewer-tagged final questions, and an entry for every task
category. Each task records total, answerable and unanswerable questions;
when it has one or more answerable questions it also reports
NDCG@5, MRR, precision@5 and recall@5 of the frozen model, the
same original-order baseline, model-minus-baseline deltas and
repository-balanced ordering diagnostics. All data stays on the local machine.

If no question in a task is answerable (or the task is absent),
its ranking metrics are **null**. Null means ranking quality is not
measurable for that category; it is not an observed zero score. The
unanswerable count is still reported.

The evaluator checks training counts, repository separation and the saved
training-data fingerprint before evaluating the final split, just as the
normal frozen evaluation does. Both pointwise and pairwise models can be
evaluated for **ordering quality**; pairwise scores must not be presented
as calibrated probabilities or automatic abstention confidence.

The report never prints individual repository IDs, source references,
questions, label records or evidence. Aggregate statistics can still
be sensitive for tiny private datasets and should be reviewed before
publishing.

## Model selection and limitations

**Do not optimize the model using these final-test metrics.** Train and
choose your ranker objective, epochs and configuration on training and
validation only, freeze them, and compute task-stratified final-test metrics
once. If the results expose a weakness that motivates further training,
reserve another untouched repository/clone-family-disjoint final test for
the next model family.

Task categories and positive/negative relevance judgments are supplied
by people, not inferred by this command. The tool cannot verify human
independence, dataset rights, reviewer competence, semantic truth, or
repo-family provenance. Use the
[annotation rubric](ai-annotation-rubric.md),
[reviewer agreement audit](ai-reviewer-agreement.md) and
[family split audit](ai-family-splits.md) first. Synthetic demo data
tests the CLI mechanics only; it cannot establish product model quality.
