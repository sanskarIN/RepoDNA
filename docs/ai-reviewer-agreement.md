# Offline annotation reviewer agreement

RepoDNA's local evidence ranker needs trustworthy human relevance judgments.
This offline tool measures consistency between two complete, independent reviews
of exactly the same evidence subset. It does not generate labels or prove the
reviewers' judgments are correct.

Follow the [human relevance annotation rubric](ai-annotation-rubric.md) consistently
before assigning labels. The rubric is a decision guide, not a substitute for
independent review or validation of source facts.

## Prepare paired reviews

1. Generate one annotation template with repodna-ranker-labels. Give the same
   selected repository/question/evidence records to two independent reviewers.
2. Each reviewer saves their own JSON copy and explicitly assigns
   "relevant": true or "relevant": false for every record. Null is never treated
   as a negative label.
3. Keep repository IDs, questions, evidence IDs, kinds, references and details
   identical. Row order may differ, but both files must cover exactly the same
   records. No duplicate IDs, missing labels or source excerpts are accepted.
4. Use a documented rubric, and keep private repository metadata, reviewer
   identities and source permissions out of public reports.

~~~sh
cargo run --locked -p repodna-ai --bin audit-repodna-reviewers -- \
  REVIEW_A.json REVIEW_B.json
~~~

Both files are limited to 8 MiB and 100,000 records. If only a subset was
double-reviewed, select the same fixed subset for both files before collecting
judgments. This command rejects mismatches and incomplete review files rather
than guessing missing judgments.

## Interpret the aggregate results

The versioned JSON report provides the number of paired records, agreements,
disagreements, both-relevant, both-irrelevant, and one-reviewer-relevant cases.
It also reports both anonymous reviewers' positive rates, raw observed
agreement, chance agreement estimated from marginal label rates and Cohen's
kappa.

When chance agreement is 100% (for example, when both reviewers mark every
record positive), kappa is mathematically undefined and reported as null.
Low-prevalence tasks and small review samples can make kappa unstable: report
the raw contingency counts and agreement rate alongside it.

Only aggregate statistics appear on stdout: no repository identifiers,
questions, evidence paths, source details or reviewer identities. Input roles
A and B are labels, not actual identities. Agreement does not prove
independent review, annotation quality, copyright permission or model accuracy.

## Follow up after disagreement

Investigate disagreements privately against the written relevance rubric and
source facts. An adjudicator may then prepare a separate, finalized training
dataset. Do not silently replace disagreement with a default negative label
or imply that identical labels are verified ground truth. Publish aggregate
adjudication counts and the review methodology without private evidence.

Continue with [dataset leakage audit](ai-label-audit.md) and
[objective selection](ai-objective-selection.md), keeping the untouched final
test separate by repository and manually curated clone family. Synthetic
examples in examples/native-ai test command mechanics only; they must not
be used to claim actual reviewer or model performance.
