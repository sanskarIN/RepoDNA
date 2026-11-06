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

Both files must be regular files and are limited to 8 MiB and 100,000
records. The reader caps streamed input even if a file changes after its
initial size check. If only a subset was
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

## Audit double-reviewed subset coverage against a frozen dataset

When only part of a training, validation or final-test split received independent
second review, the two review files alone cannot establish which portion of the
**full, fully labeled split** they represent. Supply that split as an optional
`--source` file to verify exact evidence membership and calculate coverage:

```sh
cargo run --locked -p repodna-ai --bin audit-repodna-reviewers -- \
  REVIEW_A.json REVIEW_B.json --source FROZEN_SPLIT.json
```

The `FROZEN_SPLIT.json` input has the same complete JSON label-record schema as
the output of `repodna-ranker-labels finalize`. It must contain 1–100,000
non-excerpt annotations in a regular file no larger than 8 MiB. Its entries
must be complete and have unique evidence IDs within each normalized
repository/question. Each double-reviewed evidence record must be **exactly**
present in this source dataset (repository ID, question, evidence ID, kind,
reference and detail; review labels may differ). Changing a source record,
including its evidence metadata, fails the audit without any partial stdout.

The optional `sourceCoverage` report includes source and double-reviewed
annotation counts, annotation coverage rate, source and reviewed question and
repository counts, and the numbers of **fully**, **partially** and **not**
double-reviewed questions. It also includes the stable SHA-256 fingerprint
of the complete typed source labels. Do not publish this fingerprint if
linkage between private datasets would be sensitive. These counts establish
**structural coverage, not the independence, correctness or representative
sampling of the reviewers**. If reviewing only a subset, avoid claiming the
entire source dataset was double-reviewed.

To verify that the finalized reviewed subset's judgments actually match
the source labels, also supply the explicitly adjudicated records:

```sh
cargo run --locked -p repodna-ai --bin audit-repodna-reviewers -- \
  REVIEW_A.json REVIEW_B.json ADJUDICATED.json --source FROZEN_SPLIT.json
```

This enforces exact metadata pairing, adjudication consistency, membership
and label equality for every adjudicated record. The report field
`adjudicatedLabelsMatchSourceVerified` is true only in this mode; without the
third file the audit **cannot** establish label agreement with the source.
Neither mode verifies how final labels were adjudicated. The source may
include records outside the reviewed subset; the tool never modifies them.

The `--tasks TASKS.json` and `--source FROZEN_SPLIT.json` flags can be used
together in either order, with the optional `ADJUDICATED.json` before them.
Task manifest questions cover exactly the **double-reviewed subset**, not
the full source dataset. Run the audit separately for each split to avoid
mixing training, validation and final-test coverage, and preserve the
repository and clone-family split boundaries established in the
[dataset audit](ai-label-audit.md).

## Optional agreement by human-curated task type

Overall agreement can hide inconsistent relevance judgments for individual
tasks. To inspect agreement separately for architecture, dependencies, build
commands, security, onboarding, history, hotspots and other tasks, supply a
task manifest for the **exact independently reviewed subset**:

```sh
cargo run --locked -p repodna-ai --bin audit-repodna-reviewers -- \
  REVIEW_A.json REVIEW_B.json --tasks REVIEW_TASKS.json
```

The manifest is a JSON array with one record per unique repository/question,
using the same shape and category names as the
[task coverage audit](ai-task-coverage.md):

```json
[
  {
    "repository": "example-repository",
    "query": "Which dependencies are declared?",
    "task": "dependencies"
  }
]
```

This is a **schema example, not actual reviewed evidence**. The task category
must be chosen by a person; the CLI does not infer it. The task manifest must
cover every reviewed question and no others. Duplicate normalized repository/
question pairs, invalid task categories, missing/extra questions, malformed
JSON and nonregular or oversized files are rejected with no partial JSON output.
It is limited to 2 MiB and 100,000 question entries. Repository IDs are matched
case-insensitively after trimming outer whitespace; question text is matched
exactly. Review evidence identity is still matched exactly.

The result adds an optional `taskAgreement` object with a versioned schema,
the number of questions and a `summaries` map. Each present task has paired
sample counts, positive rates, raw agreement, disagreement directions and
Cohen's kappa. A category absent from the subset is omitted, **not** scored
zero. Kappa can be null when undefined. Task counts are not independent
samples when multiple evidence records share a question or repository, and
small groups have unstable metrics: avoid claiming model accuracy from these
descriptive agreement figures.

To combine with explicit adjudication, place the reviewed final labels before
the `--tasks` flag:

```sh
cargo run --locked -p repodna-ai --bin audit-repodna-reviewers -- \
  REVIEW_A.json REVIEW_B.json ADJUDICATED.json --tasks REVIEW_TASKS.json
```

The output includes aggregates only, but small group counts or reused manifests
can still be sensitive. Inspect reports before publishing and keep private
questions, repository metadata, reviewer identities and evidence local.

## Audit an explicitly adjudicated final dataset

After both reviewers have finished, a human adjudicator can inspect contested
examples using the original repository facts and the shared
[relevance rubric](ai-annotation-rubric.md). Save a **third**, fully labeled
`ADJUDICATED.json` file with exactly the same evidence metadata. A missing
final relevance judgment is **not** an implicit negative.

~~~sh
cargo run --locked -p repodna-ai --bin audit-repodna-reviewers -- \
  REVIEW_A.json REVIEW_B.json ADJUDICATED.json
~~~

The optional third argument turns on an additional fail-closed consistency
check. It verifies that all three files contain precisely the same evidence
records, accepts different JSON row ordering, refuses missing/duplicate
records and source excerpts, and **rejects a finalized label that silently
overrides both reviewers when they agreed**.

The report retains the original agreement metrics and adds an aggregate
`adjudication` object: count of agreed judgments preserved, disputes
assigned an explicit final label, number resolved in favor of reviewer A
or B, and final positive/negative label counts. The command prints
**nothing to stdout when validation fails**. It never prints names, paths,
questions, evidence details, or individual disputed judgments.

Since relevance is Boolean, a final decision on a disputed item necessarily
agrees with one of the two reviewers; **the command cannot distinguish a real
adjudication from copying one reviewer's answers**. Document how disputes
were settled privately. If two reviewers' shared judgment was truly wrong,
correct the underlying review records through a documented correction step,
then repeat agreement and adjudication audits. Do not silently override a
consensus in the final file.

The audit does not generate or export a training dataset. Keep
`ADJUDICATED.json` private, verify repository permissions, freeze its exact
contents and split membership, and then use independently validated records
in the regular label and model evaluation pipeline. For a subset double
review, audit that exact subset; do not misrepresent it as the complete
training set.

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
