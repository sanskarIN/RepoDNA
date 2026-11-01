# Curated task coverage for the native AI benchmark

RepoDNA's local ranker evaluates **evidence relevance**, not generated prose.
A model can appear to improve on an overall score while rarely seeing certain
question types. The offline `audit-repodna-tasks` tool reports each split's
coverage from a **human-curated question category manifest**. It never guesses
task categories from the wording and never generates a label.

## Prepare the dataset and task manifest

Create three distinct labeled repository splits: training, validation, final
test. Review the source rights and provenance separately. Use the same
question text that occurs in the label files and a stable repository ID,
with one entry per unique repository/question pair:

~~~json
[
  {
    "repository": "example-public-repo",
    "query": "Which direct dependencies are declared?",
    "task": "dependencies"
  }
]
~~~

The example above illustrates the **shape only**; do not claim it is a real
benchmark observation. Each actual task entry requires a reviewer to choose
exactly one of:

- `architecture`
- `dependencies`
- `build_commands`
- `security`
- `onboarding`
- `history`
- `hotspots`
- `other`

Use `other` for a genuinely outside-scope question, not to hide uncertain
judgments. Record task categorization decisions and reviewer method
privately. Do not put source excerpts, secrets or repository metadata from
private projects into public JSON.

Run the local audit:

~~~sh
cargo run --locked -p repodna-ai --bin audit-repodna-tasks -- \
  TRAIN.json VALIDATION.json TEST.json TASKS.json
~~~

The manifest may be at most 2 MiB and contains at most 100,000 entries.
Each split is at most 8 MiB with at most 100,000 labels.
Questions are matched **exactly**. Repository IDs are matched after
trimming surrounding whitespace and lowercasing, as in the other native
ranking leakage checks. Duplicate normalized entries, missing/extra
questions, unrecognized task values, split overlap and repeated evidence IDs
are rejected before any JSON report is printed.

## Interpret the privacy-safe report

For every category in each split, the output reports the count of unique
questions, questions with at least one relevant item (answerable questions),
questions without a relevant candidate (unanswerable questions), total
labeled evidence, and positive/negative label counts. Missing core categories
produce aggregate warnings, not made-up examples or forced failures.

The CLI prints neither repository names nor question/evidence text. Aggregate
counts can still disclose information about small private benchmarks; review
the report before sharing it.

This is a **task coverage audit**, not proof of reviewer independence,
repository-family separation, dataset permissions or model accuracy.
The tool still checks repository-ID split separation, but a
[manually reviewed fork-family manifest](ai-family-splits.md) is necessary
to detect related repositories with different IDs. It does not compare
the model's per-task NDCG or MRR; reporting a task has examples does not
establish that the model performs well on that task.

## Suggested benchmark workflow

1. Select and freeze independent repository families and revisions.
2. Write and independently review the source labels, using the
   [annotation rubric](ai-annotation-rubric.md).
3. Double-review a fixed subset, run the
   [reviewer agreement and adjudication checks](ai-reviewer-agreement.md),
   and document aggregate disputes.
4. Assign task categories manually, then run this task audit alongside
   [label leakage checks](ai-label-audit.md).
5. Select and train only with validation data. Freeze the model before
   examining final-test metrics, and publish limitations and exact
   [benchmark fingerprints](ai-benchmark-provenance.md).

RepoDNA's synthetic demonstration datasets can test this CLI but cannot
support claims about the native AI model's quality on real repositories.
