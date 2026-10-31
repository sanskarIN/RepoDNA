//! Offline diagnostics for the relevance scores of the native evidence ranker.
//!
//! These measurements compare scores with human-labeled evidence relevance.
//! They are not a measure of factual entailment or an endorsement of using
//! ranker scores as confidence in generated answers.

use serde::Serialize;

use super::evaluation::validate_holdout;
use super::{LabeledEvidence, RankerModel};

/// Fixed, interpretable score bands across [0, 1].
pub const SCORE_BIN_COUNT: usize = 10;

/// Nonempty score band, with its observed positive-label frequency.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelevanceScoreBin {
    /// Inclusive lower boundary; the last bin includes its upper boundary.
    pub lower_bound: f64,
    /// Exclusive upper boundary except for the last bin.
    pub upper_bound: f64,
    /// Number of evidence judgments in this band.
    pub samples: usize,
    /// Mean ranker score, not verified factual confidence.
    pub mean_score: f64,
    /// Fraction with a human-reviewed positive relevance label.
    pub observed_relevance_rate: f64,
}

/// Sample-weighted diagnostics for offline, labeled holdout evidence.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelevanceScoreEvaluation {
    /// Total labeled evidence judgments.
    pub samples: usize,
    /// Number labeled relevant.
    pub positive_labels: usize,
    /// Average squared error against binary relevance labels (smaller is better).
    pub brier_score: f64,
    /// Average binary cross entropy against labels (smaller is better).
    pub log_loss: f64,
    /// Weighted absolute gap between mean scores and label frequencies.
    pub expected_calibration_error: f64,
    /// Only nonempty bands; empty bands contain no empirical information.
    pub bins: Vec<RelevanceScoreBin>,
}

impl RankerModel {
    /// Measure score errors on independently reviewed, repository-disjoint data.
    ///
    /// Never select hyperparameters or thresholds using the final test split;
    /// use a separate validation split. With few examples, these values and
    /// band frequencies are noisy and do not establish model calibration.
    pub fn evaluate_relevance_scores(
        &self,
        holdout: &[LabeledEvidence],
    ) -> Result<RelevanceScoreEvaluation, String> {
        self.validate()?;
        validate_holdout(holdout)?;

        let mut counts = [0_usize; SCORE_BIN_COUNT];
        let mut score_sums = [0.0_f64; SCORE_BIN_COUNT];
        let mut positive_counts = [0_usize; SCORE_BIN_COUNT];
        let mut brier_sum = 0.0;
        let mut log_loss_sum = 0.0;
        let mut positive_labels = 0_usize;

        for example in holdout {
            let score = self.relevance(&example.query, &example.evidence);
            let target = if example.relevant { 1.0 } else { 0.0 };
            let error = score - target;
            brier_sum += error * error;
            // Clamp only for the logarithm; do not change reported model scores.
            let bounded = score.clamp(1e-12, 1.0 - 1e-12);
            log_loss_sum -= target * bounded.ln() + (1.0 - target) * (1.0 - bounded).ln();

            let index = ((score * SCORE_BIN_COUNT as f64) as usize).min(SCORE_BIN_COUNT - 1);
            counts[index] += 1;
            score_sums[index] += score;
            if example.relevant {
                positive_counts[index] += 1;
                positive_labels += 1;
            }
        }

        let samples = holdout.len();
        let mut bins = Vec::new();
        let mut expected_calibration_error = 0.0;
        for (index, &count) in counts.iter().enumerate() {
            if count == 0 {
                continue;
            }
            let mean_score = score_sums[index] / count as f64;
            let observed_relevance_rate = positive_counts[index] as f64 / count as f64;
            expected_calibration_error +=
                (mean_score - observed_relevance_rate).abs() * count as f64 / samples as f64;
            bins.push(RelevanceScoreBin {
                lower_bound: index as f64 / SCORE_BIN_COUNT as f64,
                upper_bound: (index + 1) as f64 / SCORE_BIN_COUNT as f64,
                samples: count,
                mean_score,
                observed_relevance_rate,
            });
        }

        Ok(RelevanceScoreEvaluation {
            samples,
            positive_labels,
            brier_score: brier_sum / samples as f64,
            log_loss: log_loss_sum / samples as f64,
            expected_calibration_error,
            bins,
        })
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
            training_examples: 4,
            training_repositories: 2,
            training_dataset_sha256: None,
        }
    }

    fn labeled(repo: &str, id: &str, relevant: bool) -> LabeledEvidence {
        LabeledEvidence {
            repository: repo.to_owned(),
            query: "Which package?".to_owned(),
            evidence: EvidenceItem {
                id: id.to_owned(),
                kind: EvidenceKind::Package,
                reference: "Cargo.lock".to_owned(),
                detail: "A recorded dependency.".to_owned(),
            },
            relevant,
        }
    }

    #[test]
    fn balanced_constant_scores_have_predictable_error_metrics() {
        let samples = [
            labeled("repo-a", "E1", true),
            labeled("repo-b", "E2", false),
        ];
        let scores = model().evaluate_relevance_scores(&samples).unwrap();
        assert_eq!(scores.samples, 2);
        assert_eq!(scores.positive_labels, 1);
        assert_eq!(scores.brier_score, 0.25);
        assert!((scores.log_loss - std::f64::consts::LN_2).abs() < 1e-12);
        assert_eq!(scores.expected_calibration_error, 0.0);
        assert_eq!(scores.bins.len(), 1);
        assert_eq!(scores.bins[0].samples, 2);
        assert_eq!(scores.bins[0].lower_bound, 0.5);
        assert_eq!(scores.bins[0].upper_bound, 0.6);
        assert_eq!(scores.bins[0].mean_score, 0.5);
        assert_eq!(scores.bins[0].observed_relevance_rate, 0.5);
    }

    #[test]
    fn extreme_weights_remain_finite_and_improve_correct_scores() {
        let samples = [
            labeled("repo-a", "E1", true),
            labeled("repo-b", "E2", false),
        ];
        let constant = model().evaluate_relevance_scores(&samples).unwrap();
        let mut discriminative = model();
        // A positive bias and negative package weight separate these labels
        // only if their input features differ, so use different evidence kinds.
        discriminative.weights[7] = 12.0;
        discriminative.weights[8] = -12.0;
        let mut samples = samples;
        samples[1].evidence.kind = EvidenceKind::Commit;
        let measured = discriminative.evaluate_relevance_scores(&samples).unwrap();
        assert!(measured.brier_score < constant.brier_score);
        assert!(measured.log_loss < constant.log_loss);
        assert!(measured.expected_calibration_error.is_finite());
        assert_eq!(
            measured.bins.iter().map(|bin| bin.samples).sum::<usize>(),
            2
        );
        assert!(
            measured.bins.iter().all(|bin| {
                bin.mean_score.is_finite() && bin.observed_relevance_rate.is_finite()
            })
        );
    }

    #[test]
    fn rejects_invalid_or_duplicate_holdouts_and_checkpoints() {
        let sample = labeled("Repo-A", "E1", true);
        let mut alias = sample.clone();
        alias.repository = " repo-a ".to_owned();
        let ranker = model();
        assert!(ranker.evaluate_relevance_scores(&[]).is_err());
        assert!(ranker.evaluate_relevance_scores(&[sample, alias]).is_err());
        let excerpt = LabeledEvidence {
            evidence: EvidenceItem {
                kind: EvidenceKind::Excerpt,
                ..labeled("repo-b", "E2", true).evidence
            },
            ..labeled("repo-b", "E2", true)
        };
        assert!(ranker.evaluate_relevance_scores(&[excerpt]).is_err());
        let mut invalid_model = ranker;
        invalid_model.weights[0] = f64::NAN;
        assert!(
            invalid_model
                .evaluate_relevance_scores(&[labeled("repo-b", "E2", true)])
                .is_err()
        );
    }
}
