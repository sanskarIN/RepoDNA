//! Offline threshold diagnostics for the native evidence ranker.
//!
//! These diagnostics use manually reviewed labels. They must not be used to
//! select a production threshold from the final held-out test set.

use super::{AnswerabilityEvaluation, LabeledEvidence, RankerModel};

/// Limit repeated full-dataset evaluations to a small, inspectable sweep.
pub const MAX_SWEEP_THRESHOLDS: usize = 32;

impl RankerModel {
    /// Evaluate increasingly strict hypothetical answerability thresholds.
    ///
    /// Thresholds must be finite, strictly increasing, and within [0, 1].
    /// This does not change inference or enable an automatic abstention gate.
    /// Choose operating points on a separate validation split, then evaluate
    /// once on an untouched repository-disjoint test split.
    pub fn evaluate_answerability_sweep(
        &self,
        holdout: &[LabeledEvidence],
        thresholds: &[f64],
    ) -> Result<Vec<AnswerabilityEvaluation>, String> {
        if thresholds.is_empty() || thresholds.len() > MAX_SWEEP_THRESHOLDS {
            return Err("answerability sweep requires 1..=32 thresholds".into());
        }
        if thresholds
            .iter()
            .any(|threshold| !threshold.is_finite() || !(0.0..=1.0).contains(threshold))
        {
            return Err("answerability thresholds must be finite and between 0 and 1".into());
        }
        if thresholds.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("answerability thresholds must be strictly increasing".into());
        }
        // The score of a candidate does not depend on the threshold. Validate
        // and group the labels once, then score each candidate once for the
        // entire sweep instead of repeating work up to 32 times.
        self.validate()?;
        if self.format != super::MODEL_FORMAT {
            return Err("pairwise ranker scores cannot be used as answerability thresholds".into());
        }
        let groups = super::evaluation::group_holdout(holdout)?;
        let total_queries = groups.len();
        let mut results = thresholds
            .iter()
            .map(|&threshold| AnswerabilityEvaluation {
                threshold,
                total_queries,
                answerable_queries: 0,
                unanswerable_queries: 0,
                true_positive_queries: 0,
                false_negative_queries: 0,
                false_positive_queries: 0,
                true_negative_queries: 0,
            })
            .collect::<Vec<_>>();

        for ((_, query), candidates) in groups {
            let answerable = candidates.iter().any(|item| item.relevant);
            let query_terms = super::terms(query);
            let max_score = candidates
                .iter()
                .map(|item| self.relevance_with_terms(&query_terms, &item.evidence))
                .fold(f64::NEG_INFINITY, f64::max);
            for result in &mut results {
                match (answerable, max_score >= result.threshold) {
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
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EvidenceItem, EvidenceKind};
    use crate::ranking::{FEATURE_COUNT, MODEL_FORMAT};

    fn model() -> RankerModel {
        RankerModel {
            format: MODEL_FORMAT.to_owned(),
            weights: vec![0.0; FEATURE_COUNT],
            training_examples: 2,
            training_repositories: 2,
            training_dataset_sha256: None,
        }
    }

    fn sample(repository: &str, relevant: bool) -> LabeledEvidence {
        LabeledEvidence {
            repository: repository.to_owned(),
            query: "Which dependency?".into(),
            evidence: EvidenceItem {
                id: "E1".into(),
                kind: EvidenceKind::Package,
                reference: "Cargo.lock".into(),
                detail: "A package was recorded.".into(),
            },
            relevant,
        }
    }

    #[test]
    fn sweep_reports_false_answers_and_missed_answers() {
        let holdout = [
            sample("repo-positive", true),
            sample("repo-negative", false),
        ];
        // Zero weights give each candidate a relevance score of exactly 0.5.
        let points = model()
            .evaluate_answerability_sweep(&holdout, &[0.25, 0.5, 0.75])
            .unwrap();
        assert_eq!(points.len(), 3);
        assert_eq!(points[0].threshold, 0.25);
        assert_eq!(points[1].threshold, 0.5);
        assert_eq!(points[2].threshold, 0.75);
        assert_eq!(points[0].true_positive_queries, 1);
        assert_eq!(points[0].false_positive_queries, 1);
        assert_eq!(points[0].false_negative_queries, 0);
        assert_eq!(points[2].true_negative_queries, 1);
        assert_eq!(points[2].false_positive_queries, 0);
        assert_eq!(points[2].false_negative_queries, 1);
        assert_eq!(points[0].answerable_detection_rate(), 1.0);
        assert_eq!(points[0].unanswerable_abstention_rate(), 0.0);
        assert_eq!(points[2].answerable_detection_rate(), 0.0);
        assert_eq!(points[2].unanswerable_abstention_rate(), 1.0);

        for pair in points.windows(2) {
            assert!(pair[0].true_positive_queries >= pair[1].true_positive_queries);
            assert!(pair[0].false_positive_queries >= pair[1].false_positive_queries);
            assert_eq!(pair[0].total_queries, pair[1].total_queries);
        }
    }

    #[test]
    fn rejects_invalid_or_duplicate_sweep_thresholds() {
        let holdout = [sample("repo-positive", true)];
        let ranker = model();
        for thresholds in [
            vec![],
            vec![f64::NAN],
            vec![f64::INFINITY],
            vec![-0.01],
            vec![1.01],
            vec![0.5, 0.5],
            vec![0.75, 0.25],
            vec![0.0; MAX_SWEEP_THRESHOLDS + 1],
        ] {
            assert!(
                ranker
                    .evaluate_answerability_sweep(&holdout, &thresholds)
                    .is_err()
            );
        }
        assert_eq!(
            ranker
                .evaluate_answerability_sweep(&holdout, &[0.0, 1.0])
                .unwrap()
                .len(),
            2
        );
        assert!(ranker.evaluate_answerability_sweep(&[], &[0.5]).is_err());
    }
}
