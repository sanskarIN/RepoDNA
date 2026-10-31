//! Exact within-question ordering quality for evidence relevance rankers.
//!
//! Counts every positive-negative judgment pair using sorted scores, rather
//! than materializing quadratic pairs. This is **ranking** quality, not a
//! calibrated probability, explanation fidelity, or factual entailment.

use std::collections::BTreeMap;

use serde::Serialize;

use super::evaluation::validate_holdout;
use super::{LabeledEvidence, MAX_EXAMPLES, RankerModel, normalized_repository_id};

/// Aggregate ordered-pair agreement against held-out human-reviewed labels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairOrderingEvaluation {
    /// Number of labeled evidence judgments inspected.
    pub samples: usize,
    /// Distinct normalized repository and exact-question groups.
    pub total_queries: usize,
    /// Groups that contain at least one positive and one negative example.
    pub evaluated_queries: usize,
    /// Groups without both positive and negative examples, excluded from pairs.
    pub noncomparable_queries: usize,
    /// Number of all within-question positive-negative candidate pairs.
    pub total_pairs: u64,
    /// Pairs where relevant evidence scores strictly higher than irrelevant.
    pub positive_above_negative: u64,
    /// Pairs where irrelevant evidence scores strictly higher.
    pub negative_above_positive: u64,
    /// Pairs with exactly tied relevance scores.
    pub tied_pairs: u64,
    /// Micro-averaged pair agreement; ties score 0.5, 0.0 when no pairs.
    pub agreement: f64,
}

impl RankerModel {
    /// Evaluate exact relative-order agreement without score calibration.
    ///
    /// Both pointwise and pairwise checkpoints are supported. Grouping uses
    /// canonical repository IDs and exact queries. No candidate crosses a
    /// question boundary. This metric weights questions by their number of
    /// positive-negative pairs, so also report the query coverage.
    pub fn evaluate_pair_ordering(
        &self,
        holdout: &[LabeledEvidence],
    ) -> Result<PairOrderingEvaluation, String> {
        self.validate()?;
        if holdout.len() > MAX_EXAMPLES {
            return Err("too many evidence judgments for pair-ordering evaluation".into());
        }
        validate_holdout(holdout)?;

        let mut groups: BTreeMap<(String, &str), Vec<&LabeledEvidence>> = BTreeMap::new();
        for item in holdout {
            groups
                .entry((normalized_repository_id(&item.repository), &item.query))
                .or_default()
                .push(item);
        }

        let mut result = PairOrderingEvaluation {
            samples: holdout.len(),
            total_queries: groups.len(),
            evaluated_queries: 0,
            noncomparable_queries: 0,
            total_pairs: 0,
            positive_above_negative: 0,
            negative_above_positive: 0,
            tied_pairs: 0,
            agreement: 0.0,
        };

        for ((_, query), group) in groups {
            let mut positives = Vec::new();
            let mut negatives = Vec::new();
            for example in group {
                let score = self.relevance(query, &example.evidence);
                if example.relevant {
                    positives.push(score);
                } else {
                    negatives.push(score);
                }
            }
            if positives.is_empty() || negatives.is_empty() {
                result.noncomparable_queries += 1;
                continue;
            }
            result.evaluated_queries += 1;
            let pair_count = positives.len() as u64 * negatives.len() as u64;
            result.total_pairs += pair_count;
            negatives.sort_by(f64::total_cmp);

            // Binary search all negatives for each positive. This counts every
            // possible pair in O(n log n) rather than O(n²), including ties.
            for score in positives {
                let below = negatives.partition_point(|negative| *negative < score) as u64;
                let at_most = negatives.partition_point(|negative| *negative <= score) as u64;
                result.positive_above_negative += below;
                result.tied_pairs += at_most - below;
            }
        }
        result.negative_above_positive = result.total_pairs
            - result.positive_above_negative
            - result.tied_pairs;
        if result.total_pairs > 0 {
            result.agreement = (result.positive_above_negative as f64
                + 0.5 * result.tied_pairs as f64)
                / result.total_pairs as f64;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EvidenceItem, EvidenceKind};
    use crate::ranking::{FEATURE_COUNT, MODEL_FORMAT};

    fn judgment(repo: &str, question: &str, id: &str, kind: EvidenceKind, yes: bool) -> LabeledEvidence {
        LabeledEvidence {
            repository: repo.to_owned(),
            query: question.to_owned(),
            evidence: EvidenceItem {
                id: id.to_owned(),
                kind,
                reference: "src".to_owned(),
                detail: "Recorded fact".to_owned(),
            },
            relevant: yes,
        }
    }

    fn model() -> RankerModel {
        let mut weights = vec![0.0; FEATURE_COUNT];
        weights[7] = 5.0;
        weights[8] = -5.0;
        RankerModel {
            format: MODEL_FORMAT.to_owned(),
            weights,
            training_examples: 10,
            training_repositories: 2,
            training_dataset_sha256: None,
        }
    }

    #[test]
    fn correct_wrong_and_tied_pairs_are_counted_exactly() {
        let samples = [
            judgment("repo", "Q1", "E1", EvidenceKind::Package, true),
            judgment("repo", "Q1", "E2", EvidenceKind::Commit, false),
            judgment("repo", "Q1", "E3", EvidenceKind::Package, false),
            judgment("repo", "Q2", "E1", EvidenceKind::Package, false),
            judgment("repo", "Q2", "E2", EvidenceKind::Commit, true),
            judgment("repo", "unanswerable", "E1", EvidenceKind::Commit, false),
        ];
        let result = model().evaluate_pair_ordering(&samples).unwrap();
        assert_eq!(result.samples, 6);
        assert_eq!(result.total_queries, 3);
        assert_eq!(result.evaluated_queries, 2);
        assert_eq!(result.noncomparable_queries, 1);
        assert_eq!(result.total_pairs, 3);
        assert_eq!(result.positive_above_negative, 1);
        assert_eq!(result.negative_above_positive, 1);
        assert_eq!(result.tied_pairs, 1);
        assert_eq!(result.agreement, 0.5);
    }

    #[test]
    fn repository_aliases_cannot_split_question_groups() {
        let labels = [
            judgment(" Repo-A ", "Query", "E1", EvidenceKind::Package, true),
            judgment("repo-a", "Query", "E2", EvidenceKind::Commit, false),
        ];
        let result = model().evaluate_pair_ordering(&labels).unwrap();
        assert_eq!(result.total_queries, 1);
        assert_eq!(result.total_pairs, 1);
        assert_eq!(result.agreement, 1.0);

        let mut duplicates = labels.to_vec();
        duplicates[1].evidence.id = "E1".into();
        assert!(model().evaluate_pair_ordering(&duplicates).is_err());
    }

    #[test]
    fn unpaired_questions_are_reported_without_misleading_score() {
        let labels = [
            judgment("repo", "Q", "E1", EvidenceKind::Package, true),
            judgment("repo", "Q", "E2", EvidenceKind::Package, true),
        ];
        let result = model().evaluate_pair_ordering(&labels).unwrap();
        assert_eq!(result.total_pairs, 0);
        assert_eq!(result.evaluated_queries, 0);
        assert_eq!(result.noncomparable_queries, 1);
        assert_eq!(result.agreement, 0.0);
    }

    #[test]
    fn rejects_empty_holdouts() {
        assert!(model().evaluate_pair_ordering(&[]).is_err());
    }
}
