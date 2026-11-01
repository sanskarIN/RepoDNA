# Native evidence relevance annotation rubric (draft v1)

This rubric is for **human reviewers** judging whether a recorded RepoDNA
analysis fact is *useful for answering a particular question*. It does not
establish factual truth, security impact or code execution behavior. The
benchmark described in [issue #173](https://github.com/sanskarIN/RepoDNA/issues/173)
still needs real licensed repositories and human judgments.

## Unit of review

Each JSON record contains a repository ID, question/task, one numbered
non-excerpt evidence item, and a nullable Boolean relevance label. A review
decision concerns that **specific question and item**, not the overall quality
of the repository. Two independent reviewers receive the same record set.

Mark **relevant = true** only when the existing recorded fact directly helps
answer an identifiable part of the question, without guessing undocumented
behavior or supplying missing context. Mark **relevant = false** when it does
not help answer the asked question, even if the fact is interesting or true.
Never convert an unreviewed null to false: finish the review or explicitly
exclude the record from *both* independent review subsets before evaluation.

An incorrect, outdated, contradictory, ambiguous or untrustworthy fact
requires investigation outside this relevance judgment. A high relevance
score does not make a claim factually correct.

## Task-specific decision rules

| Task | Relevant positive evidence | Typically irrelevant evidence |
| --- | --- | --- |
| Architecture | Recorded module boundaries, dependency edges and relationships that answer the target design question | Unrelated file counts or unrelated commit dates |
| Dependencies | Dependency names, declared versions, lockfile facts or selected relationships specific to the requested package | General repository identity without package information |
| Build commands | Explicit recorded build scripts, configuration and tool commands applicable to the question | Commands inferred only from a language or file extension |
| Security findings | A finding's recorded location, severity and detection basis when directly requested | Unrelated popularity metrics or unsupported exploit speculation |
| Onboarding | Recorded entry points, module inventory, documented commands and setup facts related to the stated first task | Tangential contributor history or arbitrary files |
| Change history | Commits, change-frequency indicators, churn and recorded historical trends relevant to the period/component asked about | Current static package counts without change evidence |
| Hotspots | Measured complexity/churn/maintainability information for the requested area | Unrelated modules with no evidence of risk or activity |

These are examples of *decision rules*, not pre-labeled training examples.
Record disagreements when the question itself is underspecified; do not
force a positive label merely because the evidence uses similar words.

## Unanswerable questions

Include genuine questions for which the **available selected evidence
contains no relevant items**. Mark each item false only after human inspection
of the question and *every supplied candidate*. This does not mean the fact
is absent from the source code; it means the selected facts cannot support
the answer. Distinguish unavailable evidence from a proven negative fact.
Do not manufacture positive samples to make retrieval scores computable.

## Independent review and adjudication

1. Record provenance of the repository snapshot, version, known clone/fork
   family, license/permissions and review rubric outside public labels.
2. Freeze a diverse review subset before assigning labels. Give two reviewers
   identical unlabeled metadata; hide each other's decisions until completion.
3. Require a decision on every item in both review copies; do not allow
   duplicate evidence IDs within a repository/question group or source
   excerpts in the dataset.
4. Run [review agreement auditing](ai-reviewer-agreement.md); inspect
   agreement, both types of disagreement, class prevalence and whether
   kappa is undefined.
5. Privately adjudicate contested cases against recorded facts and this
   rubric. Document why the accepted label follows the task question.
   Record aggregate counts of reviewed, contested, adjudicated and
   unresolved examples, without publishing confidential evidence.
6. Freeze adjudicated labels and exact split membership. Check
   [dataset leakage](ai-label-audit.md), including repository family
   separation, before training and validation selection.

An independent reviewer is not a model API or generated pseudo-labels. The
tool cannot verify that a person independently performed the review.

## Quality and reporting boundaries

Report dataset size, per-task label prevalence, unanswerable question count,
double-review fraction, raw and chance-corrected agreement, adjudication
rate, split counts, provenance version, baseline metrics and confidence
intervals where justified. Do not release source excerpts, secrets or
personally identifying reviewer records. Question strings and references
may also be sensitive; aggregate summaries should be inspected before
publication.

Use a genuinely untouched repository-family-disjoint final test set and
do not retune a model repeatedly on that same set. The repository's
synthetic demonstration labels validate software mechanics only; they
cannot support scientific or product accuracy claims.
