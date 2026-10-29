//! Per-question retrieval evaluation for native evidence ranking.
//!
//! Binary classification accuracy cannot tell whether useful facts were placed
//! first. These metrics evaluate question/repository groups and explicitly skip
//! groups without a positive label; they do not measure factual entailment.

use std::collections::{BTreeMap, HashSet};

use serde::Serialize;

use super::{LabeledEvidence, RankerModel};

/// Aggregate retrieval metrics over repository-and-question groups.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalEvaluation {
    /// Configured cutoff for ranking metrics.
    pub k: usize,
    /// Number of distinct repository/question groups.
    pub total_queries: usize,
    /// Number of questions with at least one known relevant evidence item.
    pub evaluated_queries: usize,
    /// Questions without a positive label, excluded from ranking denominators.
    pub unanswerable_queries: usize,
    /// Average reciprocal rank of the first relevant item over answerable queries.
    pub mean_reciprocal_rank: f64,
    /// Mean relevant items in top k, divided by k (even if fewer candidates exist).
    pub precision_at_k: f64,
    /// Mean relevant items in top k, divided by relevant candidate count.
    pub recall_at_k: f64,
    /// Normalized discounted cumulative gain, with binary relevance labels.
    pub ndcg_at_k: f64,
}

/// Query-level threshold behavior on labeled, repository-disjoint holdout data.
///
/// A score exceeding the threshold means the ranker predicts at least one
/// useful evidence item. This is a *hypothetical* abstention gate, not an
/// automatic runtime behavior or a calibrated truthfulness probability.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerabilityEvaluation {
    /// Fixed threshold applied to individual evidence relevance scores.
    pub threshold: f64,
    /// Total distinct repository/question groups.
    pub total_queries: usize,
    /// Groups with at least one human-labeled relevant item.
    pub answerable_queries: usize,
    /// Groups with no human-labeled relevant items.
    pub unanswerable_queries: usize,
    /// Answerable groups containing at least one predicted relevant item.
    pub true_positive_queries: usize,
    /// Answerable groups whose evidence all falls below the threshold.
    pub false_negative_queries: usize,
    /// Unanswerable groups with an evidence score above the threshold.
    pub false_positive_queries: usize,
    /// Unanswerable groups where every evidence score is below the threshold.
    pub true_negative_queries: usize,
}

impl AnswerabilityEvaluation {
    /// Fraction of answerable questions identified; zero for an empty denominator.
    pub fn answerable_detection_rate(self) -> f64 {
        self.true_positive_queries as f64 / self.answerable_queries.max(1) as f64
    }

    /// Fraction of unanswerable questions rejected; zero for an empty denominator.
    pub fn unanswerable_abstention_rate(self) -> f64 {
        self.true_negative_queries as f64 / self.unanswerable_queries.max(1) as f64
    }
}

/// Evaluate the original input order of selected evidence without trained weights.
/// This is a necessary baseline, not a quality target by itself.
pub fn evaluate_original_order(
    holdout: &[LabeledEvidence],
    k: usize,
) -> Result<RetrievalEvaluation, String> {
    evaluate_retrieval_inner(None, holdout, k)
}

/// Signed change in retrieval metrics relative to an original-order baseline.
/// A positive number indicates improvement, not guaranteed factual correctness.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalDelta {
    /// Mean reciprocal rank improvement.
    pub mean_reciprocal_rank: f64,
    /// Precision at k improvement.
    pub precision_at_k: f64,
    /// Recall at k improvement.
    pub recall_at_k: f64,
    /// Normalized discounted cumulative gain at k improvement.
    pub ndcg_at_k: f64,
}

impl RetrievalEvaluation {
    /// Compare measurements from the same holdout and cutoff.
    ///
    /// Callers must evaluate both orderings on the same labeled examples.
    /// This consistency check detects mismatched cutoffs and group counts.
    pub fn delta_from(self, baseline: Self) -> Result<RetrievalDelta, String> {
        if self.k != baseline.k
            || self.total_queries != baseline.total_queries
            || self.evaluated_queries != baseline.evaluated_queries
            || self.unanswerable_queries != baseline.unanswerable_queries
        {
            return Err(
                "retrieval comparisons require matching cutoffs and question groups".into(),
            );
        }
        Ok(RetrievalDelta {
            mean_reciprocal_rank: self.mean_reciprocal_rank - baseline.mean_reciprocal_rank,
            precision_at_k: self.precision_at_k - baseline.precision_at_k,
            recall_at_k: self.recall_at_k - baseline.recall_at_k,
            ndcg_at_k: self.ndcg_at_k - baseline.ndcg_at_k,
        })
    }
}

impl RankerModel {
    /// Evaluate ranking effectiveness on held-out, manually labeled evidence.
    ///
    /// Groups are keyed by both repository and exact question. Use a
    /// repository-disjoint holdout: this function cannot establish independently
    /// that the supplied labels were not used for training.
    ///
    /// Questions with no relevant candidates are counted but excluded from the
    /// ranking metric denominators. Model selection should additionally measure
    /// abstention on these questions, using other evaluation methods.
    pub fn evaluate_retrieval(
        &self,
        holdout: &[LabeledEvidence],
        k: usize,
    ) -> Result<RetrievalEvaluation, String> {
        self.validate()?;
        evaluate_retrieval_inner(Some(self), holdout, k)
    }

    /// Evaluate a hypothetical score-threshold abstention rule on held-out labels.
    ///
    /// An unanswerable query is one with no *labeled* relevant candidate. This
    /// cannot establish factual truth or whether the candidate pool is complete.
    pub fn evaluate_answerability(
        &self,
        holdout: &[LabeledEvidence],
        threshold: f64,
    ) -> Result<AnswerabilityEvaluation, String> {
        self.validate()?;
        if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
            return Err("answerability threshold must be finite and between 0 and 1".into());
        }
        let groups = group_holdout(holdout)?;
        let mut result = AnswerabilityEvaluation {
            threshold,
            total_queries: groups.len(),
            answerable_queries: 0,
            unanswerable_queries: 0,
            true_positive_queries: 0,
            false_negative_queries: 0,
            false_positive_queries: 0,
            true_negative_queries: 0,
        };
        for ((_, query), candidates) in groups {
            let labeled_answerable = candidates.iter().any(|item| item.relevant);
            let predicted_answerable = candidates
                .iter()
                .any(|item| self.relevance(query, &item.evidence) >= threshold);
            match (labeled_answerable, predicted_answerable) {
                (true, true) => {
                    result.answerable_queries += 1;
                    result.true_positive_queries += 1;
                }
                (true, false) => {
                    result.answerable_queries += 1;
                    result.false_negative_queries += 1;
                }
                (false, true) => {
                    result.unanswerable_queries += 1;
                    result.false_positive_queries += 1;
                }
                (false, false) => {
                    result.unanswerable_queries += 1;
                    result.true_negative_queries += 1;
                }
            }
        }
        Ok(result)
    }
}

fn group_holdout(
    holdout: &[LabeledEvidence],
) -> Result<BTreeMap<(&str, &str), Vec<&LabeledEvidence>>, String> {
    if holdout.is_empty() {
        return Err("retrieval evaluation needs labeled holdout examples".to_owned());
    }
    let mut groups: BTreeMap<(&str, &str), Vec<&LabeledEvidence>> = BTreeMap::new();
    let mut ids = HashSet::new();
    for item in holdout {
        if item.repository.trim().is_empty()
            || item.query.trim().is_empty()
            || item.evidence.id.trim().is_empty()
            || item.evidence.detail.trim().is_empty()
        {
            return Err(
                "holdout examples require repository, query, evidence ID and detail".to_owned(),
            );
        }
        let identity = (
            item.repository.as_str(),
            item.query.as_str(),
            item.evidence.id.as_str(),
        );
        if !ids.insert(identity) {
            return Err("duplicate evidence ID within repository/question group".to_owned());
        }
        groups
            .entry((&item.repository, &item.query))
            .or_default()
            .push(item);
    }
    Ok(groups)
}

fn evaluate_retrieval_inner(
    model: Option<&RankerModel>,
    holdout: &[LabeledEvidence],
    k: usize,
) -> Result<RetrievalEvaluation, String> {
    if !(1..=100).contains(&k) {
        return Err("retrieval cutoff k must be between 1 and 100".to_owned());
    }
    let groups = group_holdout(holdout)?;
    let total_queries = groups.len();
    let mut evaluated_queries = 0_usize;
    let mut unanswerable_queries = 0_usize;
    let mut mrr = 0.0;
    let mut precision = 0.0;
    let mut recall = 0.0;
    let mut ndcg = 0.0;
    for ((_, query), mut group) in groups {
        let total_relevant = group.iter().filter(|item| item.relevant).count();
        if total_relevant == 0 {
            unanswerable_queries += 1;
            continue;
        }
        evaluated_queries += 1;
        // Stable sort preserves input order when scores tie.
        if let Some(model) = model {
            group.sort_by(|left, right| {
                model
                    .relevance(query, &right.evidence)
                    .total_cmp(&model.relevance(query, &left.evidence))
            });
        }
        if let Some(position) = group.iter().position(|item| item.relevant) {
            mrr += 1.0 / (position + 1) as f64;
        }
        let mut found = 0_usize;
        let mut dcg = 0.0;
        for (position, item) in group.iter().take(k).enumerate() {
            if item.relevant {
                found += 1;
                dcg += 1.0 / ((position + 2) as f64).log2();
            }
        }
        let ideal_dcg: f64 = (0..total_relevant.min(k))
            .map(|position| 1.0 / ((position + 2) as f64).log2())
            .sum();
        precision += found as f64 / k as f64;
        recall += found as f64 / total_relevant as f64;
        ndcg += dcg / ideal_dcg;
    }
    let divisor = evaluated_queries.max(1) as f64;
    Ok(RetrievalEvaluation {
        k,
        total_queries,
        evaluated_queries,
        unanswerable_queries,
        mean_reciprocal_rank: mrr / divisor,
        precision_at_k: precision / divisor,
        recall_at_k: recall / divisor,
        ndcg_at_k: ndcg / divisor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EvidenceItem, EvidenceKind};
    use crate::ranking::{FEATURE_COUNT, MODEL_FORMAT};

    fn labeled(
        repo: &str,
        query: &str,
        id: &str,
        kind: EvidenceKind,
        relevant: bool,
    ) -> LabeledEvidence {
        LabeledEvidence {
            repository: repo.to_owned(),
            query: query.to_owned(),
            evidence: EvidenceItem {
                id: id.to_owned(),
                kind,
                reference: "src/".to_owned(),
                detail: "A recorded fact.".to_owned(),
            },
            relevant,
        }
    }

    fn model() -> RankerModel {
        let mut weights = vec![0.0; FEATURE_COUNT];
        weights[7] = 5.0; // packages first
        weights[8] = -5.0; // commits last
        RankerModel {
            format: MODEL_FORMAT.to_owned(),
            weights,
            training_examples: 10,
            training_repositories: 3,
        }
    }

    #[test]
    fn scores_correct_order_and_excludes_unanswerable_queries() {
        let holdout = vec![
            labeled(
                "repo-one",
                "Which package?",
                "E1",
                EvidenceKind::Commit,
                false,
            ),
            labeled(
                "repo-one",
                "Which package?",
                "E2",
                EvidenceKind::Package,
                true,
            ),
            labeled(
                "repo-two",
                "Where is a package?",
                "E3",
                EvidenceKind::Package,
                true,
            ),
            labeled(
                "repo-two",
                "Where is a package?",
                "E4",
                EvidenceKind::Commit,
                false,
            ),
            labeled(
                "repo-three",
                "Unknown feature",
                "E5",
                EvidenceKind::Commit,
                false,
            ),
        ];
        let result = model().evaluate_retrieval(&holdout, 1).unwrap();
        assert_eq!(result.total_queries, 3);
        assert_eq!(result.evaluated_queries, 2);
        assert_eq!(result.unanswerable_queries, 1);
        assert_eq!(result.mean_reciprocal_rank, 1.0);
        assert_eq!(result.precision_at_k, 1.0);
        assert_eq!(result.recall_at_k, 1.0);
        assert_eq!(result.ndcg_at_k, 1.0);
    }

    #[test]
    fn penalizes_relevant_evidence_ranked_after_cutoff() {
        let sample = [
            labeled("r1", "Which package?", "E1", EvidenceKind::Commit, true),
            labeled("r1", "Which package?", "E2", EvidenceKind::Package, false),
        ];
        let result = model().evaluate_retrieval(&sample, 1).unwrap();
        assert_eq!(result.mean_reciprocal_rank, 0.5);
        assert_eq!(result.precision_at_k, 0.0);
        assert_eq!(result.recall_at_k, 0.0);
        assert_eq!(result.ndcg_at_k, 0.0);
    }

    #[test]
    fn rejects_invalid_cutoffs_and_duplicate_evidence_ids() {
        let samples = [
            labeled("r1", "query", "E1", EvidenceKind::Fact, true),
            labeled("r1", "query", "E1", EvidenceKind::File, false),
        ];
        assert!(model().evaluate_retrieval(&samples, 0).is_err());
        assert!(model().evaluate_retrieval(&samples, 101).is_err());
        assert!(model().evaluate_retrieval(&[], 1).is_err());
        assert!(model().evaluate_retrieval(&samples, 2).is_err());
    }

    #[test]
    fn original_order_baseline_exposes_ranking_improvement() {
        let sample = [
            labeled("repo", "Dependency?", "E1", EvidenceKind::Commit, false),
            labeled("repo", "Dependency?", "E2", EvidenceKind::Package, true),
        ];
        let baseline = evaluate_original_order(&sample, 1).unwrap();
        let learned = model().evaluate_retrieval(&sample, 1).unwrap();
        assert_eq!(baseline.mean_reciprocal_rank, 0.5);
        assert_eq!(baseline.recall_at_k, 0.0);
        assert_eq!(learned.mean_reciprocal_rank, 1.0);
        assert_eq!(learned.recall_at_k, 1.0);
    }

    #[test]
    fn signed_metric_deltas_report_improvement_and_reject_mismatched_groups() {
        let sample = [
            labeled("repo", "Dependency?", "E1", EvidenceKind::Commit, false),
            labeled("repo", "Dependency?", "E2", EvidenceKind::Package, true),
        ];
        let original = evaluate_original_order(&sample, 1).unwrap();
        let trained = model().evaluate_retrieval(&sample, 1).unwrap();
        let delta = trained.delta_from(original).unwrap();
        assert_eq!(delta.mean_reciprocal_rank, 0.5);
        assert_eq!(delta.precision_at_k, 1.0);
        assert_eq!(delta.recall_at_k, 1.0);
        assert_eq!(delta.ndcg_at_k, 1.0);
        assert_eq!(original.delta_from(trained).unwrap().ndcg_at_k, -1.0);

        let other_cutoff = evaluate_original_order(&sample, 2).unwrap();
        assert!(trained.delta_from(other_cutoff).is_err());
        let other_groups = evaluate_original_order(&sample[..1], 1).unwrap();
        assert!(trained.delta_from(other_groups).is_err());
    }

    #[test]
    fn reports_false_positives_and_missed_answerable_queries() {
        let samples = [
            labeled("r1", "query 1", "E1", EvidenceKind::Package, true),
            labeled("r2", "query 2", "E1", EvidenceKind::Commit, true),
            labeled("r3", "query 3", "E1", EvidenceKind::Commit, false),
            labeled("r4", "query 4", "E1", EvidenceKind::Package, false),
        ];
        let evaluation = model().evaluate_answerability(&samples, 0.5).unwrap();
        assert_eq!(evaluation.total_queries, 4);
        assert_eq!(evaluation.answerable_queries, 2);
        assert_eq!(evaluation.unanswerable_queries, 2);
        assert_eq!(evaluation.true_positive_queries, 1);
        assert_eq!(evaluation.false_negative_queries, 1);
        assert_eq!(evaluation.false_positive_queries, 1);
        assert_eq!(evaluation.true_negative_queries, 1);
        assert_eq!(evaluation.answerable_detection_rate(), 0.5);
        assert_eq!(evaluation.unanswerable_abstention_rate(), 0.5);
        let value = serde_json::to_value(evaluation).unwrap();
        assert_eq!(value["falsePositiveQueries"], 1);
        assert_eq!(value["threshold"], 0.5);
    }

    #[test]
    fn answerability_rejects_bad_thresholds_and_invalid_holdout() {
        let samples = [labeled("r1", "question", "E1", EvidenceKind::Fact, true)];
        assert!(model().evaluate_answerability(&samples, -0.1).is_err());
        assert!(model().evaluate_answerability(&samples, 1.01).is_err());
        assert!(model().evaluate_answerability(&samples, f64::NAN).is_err());
        assert!(
            model()
                .evaluate_answerability(&samples, f64::INFINITY)
                .is_err()
        );
        assert!(model().evaluate_answerability(&[], 0.5).is_err());
        assert!(
            model()
                .evaluate_answerability(&[samples[0].clone(), samples[0].clone()], 0.5)
                .is_err()
        );
        assert!(model().evaluate_answerability(&samples, 0.0).is_ok());
        assert!(model().evaluate_answerability(&samples, 1.0).is_ok());
    }

    #[test]
    fn unanswerable_only_groups_get_zero_scores() {
        let sample = [labeled(
            "repo-no-answer",
            "Where is absent symbol?",
            "E1",
            EvidenceKind::Fact,
            false,
        )];
        let result = model().evaluate_retrieval(&sample, 3).unwrap();
        assert_eq!(result.evaluated_queries, 0);
        assert_eq!(result.unanswerable_queries, 1);
        assert_eq!(result.mean_reciprocal_rank, 0.0);
        assert_eq!(result.ndcg_at_k, 0.0);
    }
}
