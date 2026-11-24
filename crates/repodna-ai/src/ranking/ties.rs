//! Tie-aware, offline retrieval diagnostics.
//!
//! Runtime ranking retains its stable original-order tie break. This report
//! describes how much top-k quality could depend on that ordering, without
//! changing ranking, fitting weights, selecting a model or exposing labels.

use serde::Serialize;

use super::evaluation::group_holdout;
use super::{LabeledEvidence, MAX_EXAMPLES, RankerModel, terms};

/// Mean metric under pessimistic, uniformly random and optimistic tie breaks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TieMetricRange {
    /// All irrelevant items precede relevant items within each equal-score group.
    pub worst: f64,
    /// Exact expectation over uniform permutations within equal-score groups.
    pub expected: f64,
    /// All relevant items precede irrelevant items within each equal-score group.
    pub best: f64,
}

impl TieMetricRange {
    fn add(&mut self, other: Self) {
        self.worst += other.worst;
        self.expected += other.expected;
        self.best += other.best;
    }

    fn divided(self, divisor: f64) -> Self {
        Self {
            worst: self.worst / divisor,
            expected: self.expected / divisor,
            best: self.best / divisor,
        }
    }
}

/// Aggregate tie sensitivity over answerable repository/question groups.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TieAwareEvaluation {
    /// Top-k cutoff, matching the ordinary retrieval evaluation.
    pub k: usize,
    /// All repository/question groups, including those without positive labels.
    pub total_queries: usize,
    /// Groups with at least one positive judgment.
    pub evaluated_queries: usize,
    /// Groups excluded from ranking metrics because there is no positive label.
    pub unanswerable_queries: usize,
    /// Answerable questions with a mixed-label score tie intersecting top k.
    pub mixed_tie_queries_at_k: usize,
    /// Answerable questions whose number of top-k positives depends on tie order.
    pub cutoff_sensitive_queries: usize,
    /// Precision divided by k, even when a question has fewer than k candidates.
    pub precision_at_k: Option<TieMetricRange>,
    /// Recall divided by the number of relevant candidates.
    pub recall_at_k: Option<TieMetricRange>,
    /// Binary normalized discounted cumulative gain.
    pub ndcg_at_k: Option<TieMetricRange>,
}

#[derive(Default)]
struct QueryMetrics {
    hits: TieMetricRange,
    dcg: TieMetricRange,
    mixed_tie: bool,
    cutoff_sensitive: bool,
}

fn discount(position: usize) -> f64 {
    1.0 / ((position + 2) as f64).log2()
}

// The input is sorted descending by the same finite, sigmoid-transformed
// score used in production. Equal means exact equality under total_cmp;
// near-equal scores are not arbitrarily grouped.
fn query_metrics(scored: &[(f64, bool)], k: usize) -> QueryMetrics {
    let mut result = QueryMetrics::default();
    let mut start = 0;
    while start < scored.len() && start < k {
        let mut end = start + 1;
        while end < scored.len() && scored[start].0.total_cmp(&scored[end].0).is_eq() {
            end += 1;
        }
        let size = end - start;
        let positive = scored[start..end]
            .iter()
            .filter(|(_, label)| *label)
            .count();
        let negative = size - positive;
        let visible = size.min(k - start);
        let worst_hits = visible.saturating_sub(negative);
        let best_hits = visible.min(positive);
        let probability = positive as f64 / size as f64;

        result.mixed_tie |= positive > 0 && negative > 0;
        result.cutoff_sensitive |= worst_hits != best_hits;
        result.hits.add(TieMetricRange {
            worst: worst_hits as f64,
            expected: visible as f64 * probability,
            best: best_hits as f64,
        });

        // Each position in a uniform permutation has the same probability
        // of holding a relevant item. Linearity of expectation gives exact
        // expected DCG without sampling or enumerating permutations.
        for offset in 0..visible {
            let weight = discount(start + offset);
            if offset >= negative {
                result.dcg.worst += weight;
            }
            result.dcg.expected += probability * weight;
            if offset < positive {
                result.dcg.best += weight;
            }
        }
        start = end;
    }
    result
}

/// Evaluate uncertainty from exact ties for a frozen checkpoint.
///
/// Results are question-weighted, like ordinary retrieval. A null metric
/// means the supplied holdout has no answerable question. These ranges are
/// deterministic mathematical bounds, not statistical confidence intervals.
/// They neither establish factual correctness nor justify tuning on test data.
pub fn evaluate_tie_aware(
    model: &RankerModel,
    holdout: &[LabeledEvidence],
    k: usize,
) -> Result<TieAwareEvaluation, String> {
    model.validate()?;
    if !(1..=100).contains(&k) {
        return Err("retrieval cutoff k must be between 1 and 100".into());
    }
    if holdout.len() > MAX_EXAMPLES {
        return Err("too many labeled holdout examples".into());
    }
    let groups = group_holdout(holdout)?;
    let total_queries = groups.len();
    let mut evaluated_queries = 0;
    let mut mixed_tie_queries_at_k = 0;
    let mut cutoff_sensitive_queries = 0;
    let mut precision = TieMetricRange::default();
    let mut recall = TieMetricRange::default();
    let mut ndcg = TieMetricRange::default();

    for ((_, query), candidates) in groups {
        let positive = candidates.iter().filter(|item| item.relevant).count();
        if positive == 0 {
            continue;
        }
        evaluated_queries += 1;
        let query_terms = terms(query);
        let mut scored = candidates
            .iter()
            .map(|item| {
                (
                    model.relevance_with_terms(&query_terms, &item.evidence),
                    item.relevant,
                )
            })
            .collect::<Vec<_>>();
        scored.sort_unstable_by(|left, right| right.0.total_cmp(&left.0));
        let metrics = query_metrics(&scored, k);
        mixed_tie_queries_at_k += usize::from(metrics.mixed_tie);
        cutoff_sensitive_queries += usize::from(metrics.cutoff_sensitive);
        precision.add(metrics.hits.divided(k as f64));
        recall.add(metrics.hits.divided(positive as f64));
        let ideal: f64 = (0..positive.min(k)).map(discount).sum();
        ndcg.add(metrics.dcg.divided(ideal));
    }

    let average = |metric: TieMetricRange| {
        (evaluated_queries > 0).then(|| metric.divided(evaluated_queries as f64))
    };
    Ok(TieAwareEvaluation {
        k,
        total_queries,
        evaluated_queries,
        unanswerable_queries: total_queries - evaluated_queries,
        mixed_tie_queries_at_k,
        cutoff_sensitive_queries,
        precision_at_k: average(precision),
        recall_at_k: average(recall),
        ndcg_at_k: average(ndcg),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EvidenceItem, EvidenceKind};
    use crate::ranking::{FEATURE_COUNT, MODEL_FORMAT, PAIRWISE_MODEL_FORMAT};

    fn model() -> RankerModel {
        RankerModel {
            format: MODEL_FORMAT.into(),
            weights: vec![0.0; FEATURE_COUNT],
            training_examples: 2,
            training_repositories: 1,
            training_dataset_sha256: None,
        }
    }

    fn label(repo: &str, query: &str, id: &str, relevant: bool) -> LabeledEvidence {
        LabeledEvidence {
            repository: repo.into(),
            query: query.into(),
            evidence: EvidenceItem {
                id: id.into(),
                kind: EvidenceKind::Package,
                reference: "Cargo.lock".into(),
                detail: "Recorded dependency".into(),
            },
            relevant,
        }
    }

    fn close(left: f64, right: f64) {
        assert!((left - right).abs() < 1e-12, "{left} != {right}");
    }

    fn range(actual: TieMetricRange, worst: f64, expected: f64, best: f64) {
        close(actual.worst, worst);
        close(actual.expected, expected);
        close(actual.best, best);
    }

    #[test]
    fn constant_ranker_with_mixed_labels_has_exact_tie_bounds() {
        let samples = [
            label("repo", "query", "E1", false),
            label("repo", "query", "E2", true),
            label("repo", "query", "E3", false),
            label("repo", "query", "E4", true),
        ];
        let result = evaluate_tie_aware(&model(), &samples, 2).unwrap();
        assert_eq!(result.evaluated_queries, 1);
        assert_eq!(result.mixed_tie_queries_at_k, 1);
        assert_eq!(result.cutoff_sensitive_queries, 1);
        range(result.precision_at_k.unwrap(), 0.0, 0.5, 1.0);
        range(result.recall_at_k.unwrap(), 0.0, 0.5, 1.0);
        range(result.ndcg_at_k.unwrap(), 0.0, 0.5, 1.0);
    }

    #[test]
    fn full_tie_inside_cutoff_changes_dcg_but_not_hit_count() {
        let scored = [(0.9, false), (0.9, true), (0.3, true)];
        let result = query_metrics(&scored, 3);
        assert!(result.mixed_tie);
        assert!(!result.cutoff_sensitive);
        range(result.hits, 2.0, 2.0, 2.0);
        range(
            result.dcg,
            discount(1) + discount(2),
            (discount(0) + discount(1)) / 2.0 + discount(2),
            discount(0) + discount(2),
        );
    }

    #[test]
    fn ties_below_cutoff_do_not_change_top_k() {
        let scored = [(0.9, true), (0.3, false), (0.3, true)];
        let result = query_metrics(&scored, 1);
        assert!(!result.mixed_tie);
        assert!(!result.cutoff_sensitive);
        range(result.hits, 1.0, 1.0, 1.0);
        range(result.dcg, 1.0, 1.0, 1.0);
    }

    #[test]
    fn distinct_scores_match_stable_runtime_retrieval() {
        let mut ranker = model();
        ranker.weights[7] = 1.0;
        let mut negative = label("repo", "query", "E1", false);
        negative.evidence.kind = EvidenceKind::Commit;
        let positive = label("repo", "query", "E2", true);
        let samples = [negative, positive];
        let actual = ranker.evaluate_retrieval(&samples, 5).unwrap();
        let diagnostic = evaluate_tie_aware(&ranker, &samples, 5).unwrap();
        assert_eq!(diagnostic.mixed_tie_queries_at_k, 0);
        range(diagnostic.precision_at_k.unwrap(), 0.2, 0.2, 0.2);
        range(diagnostic.recall_at_k.unwrap(), 1.0, 1.0, 1.0);
        range(
            diagnostic.ndcg_at_k.unwrap(),
            actual.ndcg_at_k,
            actual.ndcg_at_k,
            actual.ndcg_at_k,
        );
    }

    #[test]
    fn ties_are_invariant_to_record_order_and_accept_pairwise_weights() {
        let mut samples = vec![
            label("Repo", "query", "E1", true),
            label(" repo ", "query", "E2", false),
            label("other", "query", "E1", false),
        ];
        let mut ranker = model();
        ranker.format = PAIRWISE_MODEL_FORMAT.into();
        let first = evaluate_tie_aware(&ranker, &samples, 1).unwrap();
        samples.reverse();
        assert_eq!(first, evaluate_tie_aware(&ranker, &samples, 1).unwrap());
        assert_eq!(first.total_queries, 2);
        assert_eq!(first.unanswerable_queries, 1);
        assert_eq!(first.evaluated_queries, 1);
        range(first.ndcg_at_k.unwrap(), 0.0, 0.5, 1.0);
    }

    #[test]
    fn unanswerable_metrics_are_null_and_invalid_inputs_fail() {
        let sample = label("repo", "query", "E1", false);
        let result = evaluate_tie_aware(&model(), std::slice::from_ref(&sample), 5).unwrap();
        assert_eq!(result.evaluated_queries, 0);
        assert_eq!(result.precision_at_k, None);
        assert_eq!(result.recall_at_k, None);
        assert_eq!(result.ndcg_at_k, None);
        assert!(evaluate_tie_aware(&model(), &[], 5).is_err());
        assert!(evaluate_tie_aware(&model(), std::slice::from_ref(&sample), 0).is_err());
        assert!(evaluate_tie_aware(&model(), std::slice::from_ref(&sample), 101).is_err());
        assert!(evaluate_tie_aware(&model(), &[sample.clone(), sample], 5).is_err());
    }

    #[test]
    fn expectation_matches_exhaustive_small_tie_permutations() {
        // Enumerate every binary label ordering for 1..=7 equal-score items
        // at every cutoff. This independently checks the analytic formula.
        for size in 1_usize..=7 {
            for positives in 1..=size {
                for k in 1..=size {
                    let mut hits = Vec::new();
                    let mut gains = Vec::new();
                    for mask in 0_usize..(1 << size) {
                        if mask.count_ones() as usize != positives {
                            continue;
                        }
                        let mut hit = 0.0;
                        let mut gain = 0.0;
                        for index in 0..k {
                            if mask & (1 << index) != 0 {
                                hit += 1.0;
                                gain += discount(index);
                            }
                        }
                        hits.push(hit);
                        gains.push(gain);
                    }
                    let scored = (0..size)
                        .map(|index| (0.5, index < positives))
                        .collect::<Vec<_>>();
                    let actual = query_metrics(&scored, k);
                    for (metric, values) in [(actual.hits, hits), (actual.dcg, gains)] {
                        range(
                            metric,
                            values.iter().copied().reduce(f64::min).unwrap(),
                            values.iter().sum::<f64>() / values.len() as f64,
                            values.iter().copied().reduce(f64::max).unwrap(),
                        );
                    }
                }
            }
        }
    }
}
