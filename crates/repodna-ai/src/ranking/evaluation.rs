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

/// Evaluate the original input order of selected evidence without trained weights.
/// This is a necessary baseline, not a quality target by itself.
pub fn evaluate_original_order(
    holdout: &[LabeledEvidence],
    k: usize,
) -> Result<RetrievalEvaluation, String> {
    evaluate_retrieval_inner(None, holdout, k)
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
}

fn evaluate_retrieval_inner(
    model: Option<&RankerModel>,
    holdout: &[LabeledEvidence],
    k: usize,
) -> Result<RetrievalEvaluation, String> {
    if !(1..=100).contains(&k) {
        return Err("retrieval cutoff k must be between 1 and 100".to_owned());
    }
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
