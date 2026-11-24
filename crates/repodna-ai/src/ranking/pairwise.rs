//! Deterministic pairwise relevance training for the built-in CPU ranker.
//!
//! Each update compares two human-labeled evidence items for the same
//! repository and question. This optimizes relative ranking rather than an
//! item's standalone relevance probability. Scores are NOT calibrated
//! probabilities, and the model cannot generate language or prove claims.

use std::collections::{BTreeMap, HashSet};

use super::{
    FEATURE_COUNT, LabeledEvidence, MAX_EXAMPLES, RankerModel, features_with_terms,
    normalized_repository_id, sigmoid, terms, training_data_fingerprint, validate_evidence_ids,
};

/// Versioned checkpoint for relative-order-trained ranker weights.
pub const PAIRWISE_MODEL_FORMAT: &str = "repodna-evidence-pairwise/1";
const MAX_PAIRS_PER_GROUP: usize = 4096;
const LEARNING_RATE: f64 = 0.05;
const WEIGHT_DECAY: f64 = 0.001;

/// Mixed-label question group, preserving borrowed, checked annotations.
type PairGroup<'a> = (Vec<&'a LabeledEvidence>, Vec<&'a LabeledEvidence>);

// Preserve the v1 pair schedule exactly, including its per-group cap.
// No feature computation occurs inside the epoch loop.
fn prepare_pairs(
    query: &str,
    positives: &[&LabeledEvidence],
    negatives: &[&LabeledEvidence],
    output: &mut Vec<[f64; FEATURE_COUNT]>,
) {
    let cycle_length = positives.len().max(negatives.len());
    let pair_count = cycle_length.saturating_mul(2).min(MAX_PAIRS_PER_GROUP);
    let query_terms = terms(query);
    // Bound feature preparation even when a group exceeds the pair cap.
    // Every visited index is below pair_count, except the negative rotation
    // can reach one additional index on the second cycle.
    let positive_features = positives
        .iter()
        .take(pair_count)
        .map(|item| features_with_terms(&query_terms, &item.evidence))
        .collect::<Vec<_>>();
    let negative_features = negatives
        .iter()
        .take(pair_count.saturating_add(1))
        .map(|item| features_with_terms(&query_terms, &item.evidence))
        .collect::<Vec<_>>();
    for i in 0..pair_count {
        let p = positive_features[i % positives.len()];
        let n = negative_features[(i + i / cycle_length) % negatives.len()];
        let mut diff = [0.0; FEATURE_COUNT];
        for j in 0..FEATURE_COUNT {
            diff[j] = p[j] - n[j];
        }
        output.push(diff);
    }
}

impl RankerModel {
    /// Learn relative evidence order from within-question positive/negative pairs.
    ///
    /// Only evidence from the *same* repository and exact query is paired.
    /// A finite, deterministic subset bounds work to at most twice the input
    /// size per epoch. No cross-repository negatives, automatic labels, source
    /// execution, model downloads or external AI runtime are used.
    pub fn train_pairwise(samples: &[LabeledEvidence], epochs: u32) -> Result<Self, String> {
        if !(2..=MAX_EXAMPLES).contains(&samples.len()) {
            return Err("training set size is outside supported bounds".into());
        }
        if !(1..=200).contains(&epochs) {
            return Err("epochs must be 1..=200".into());
        }
        if samples.iter().any(|s| {
            s.repository.trim().is_empty()
                || s.query.trim().is_empty()
                || s.evidence.detail.trim().is_empty()
                || s.query.len() > 4_000
                || s.evidence.detail.len() > 8_000
        }) {
            return Err("invalid or oversized training example".into());
        }
        validate_evidence_ids(samples)?;

        let training_repositories = samples
            .iter()
            .map(|s| normalized_repository_id(&s.repository))
            .collect::<HashSet<_>>()
            .len();
        let mut groups: BTreeMap<(String, &str), PairGroup<'_>> = BTreeMap::new();
        for sample in samples {
            let group = groups
                .entry((normalized_repository_id(&sample.repository), &sample.query))
                .or_default();
            if sample.relevant {
                group.0.push(sample);
            } else {
                group.1.push(sample);
            }
        }
        for (positives, negatives) in groups.values_mut() {
            positives.sort_unstable_by(|a, b| a.evidence.id.cmp(&b.evidence.id));
            negatives.sort_unstable_by(|a, b| a.evidence.id.cmp(&b.evidence.id));
        }
        if !groups.values().any(|(p, n)| !p.is_empty() && !n.is_empty()) {
            return Err(
                "pairwise training requires a positive and negative in the same question".into(),
            );
        }

        // Pair selection and feature vectors are independent of the current
        // weights. Prepare them once, preserving the exact original update
        // order and floating-point arithmetic. At most 2 * samples.len()
        // differences are retained, each containing FEATURE_COUNT f64s.
        let mut prepared = Vec::new();
        for ((_, query), (positives, negatives)) in &groups {
            if positives.is_empty() || negatives.is_empty() {
                continue;
            }
            prepare_pairs(query, positives, negatives, &mut prepared);
        }

        let mut weights = vec![0.0_f64; FEATURE_COUNT];
        for _ in 0..epochs {
            for &diff in &prepared {
                let logit = weights.iter().zip(diff).map(|(w, d)| w * d).sum();
                let gradient = 1.0 - sigmoid(logit);
                for (weight, delta) in weights.iter_mut().zip(diff) {
                    *weight = (*weight
                        + LEARNING_RATE * (gradient * delta - WEIGHT_DECAY * *weight))
                        .clamp(-32.0, 32.0);
                }
            }
        }

        let model = Self {
            format: PAIRWISE_MODEL_FORMAT.to_owned(),
            weights,
            training_examples: samples.len(),
            training_repositories,
            training_dataset_sha256: Some(training_data_fingerprint(samples)?),
        };
        model.validate()?;
        Ok(model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EvidenceItem, EvidenceKind};

    fn sample(
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
                reference: "Cargo.toml".to_owned(),
                detail: "Recorded repository evidence".to_owned(),
            },
            relevant,
        }
    }

    fn training() -> Vec<LabeledEvidence> {
        vec![
            sample(
                "repo-a",
                "Which dependency?",
                "E1",
                EvidenceKind::Package,
                true,
            ),
            sample(
                "repo-a",
                "Which dependency?",
                "E2",
                EvidenceKind::Commit,
                false,
            ),
            sample(
                "repo-b",
                "Which package?",
                "E3",
                EvidenceKind::Package,
                true,
            ),
            sample(
                "repo-b",
                "Which package?",
                "E4",
                EvidenceKind::Metric,
                false,
            ),
        ]
    }

    #[test]
    fn learns_correct_relative_order_and_keeps_provenance() {
        let data = training();
        let model = RankerModel::train_pairwise(&data, 40).unwrap();
        assert_eq!(model.format, PAIRWISE_MODEL_FORMAT);
        assert_eq!(model.training_examples, 4);
        assert_eq!(model.training_repositories, 2);
        assert_eq!(
            model.training_dataset_sha256.as_deref(),
            Some(training_data_fingerprint(&data).unwrap().as_str())
        );
        assert!(model.weights.iter().all(|weight| weight.is_finite()));
        model.validate().unwrap();
        // Pairwise scores are relative ordering signals, never calibrated
        // probabilities or absolute answerability confidence.
        assert!(model.evaluate_relevance_scores(&data).is_err());
        assert!(model.evaluate_answerability(&data, 0.5).is_err());
        assert!(
            model
                .evaluate_answerability_sweep(&data, &[0.4, 0.6])
                .is_err()
        );
        let package = &data[0].evidence;
        let commit = &data[1].evidence;
        assert!(
            model.relevance("Which dependency?", package)
                > model.relevance("Which dependency?", commit)
        );
    }

    #[test]
    fn stable_under_reordering_and_normalized_repository_aliases() {
        let mut data = training();
        let first = RankerModel::train_pairwise(&data, 20).unwrap();
        data.reverse();
        data[0].repository = " REPO-B ".into();
        let second = RankerModel::train_pairwise(&data, 20).unwrap();
        assert_eq!(first.weights, second.weights);
        // The fingerprint intentionally covers the exact stored labels,
        // including original spelling of repository identifiers.
        assert_ne!(
            first.training_dataset_sha256,
            second.training_dataset_sha256
        );
    }

    #[test]
    fn rejects_unpaired_and_malformed_labels() {
        let mut data = training();
        for example in &mut data {
            example.relevant = true;
        }
        assert!(RankerModel::train_pairwise(&data, 20).is_err());
        let mut data = training();
        data.push(data[0].clone());
        assert!(RankerModel::train_pairwise(&data, 20).is_err());
        assert!(RankerModel::train_pairwise(&training(), 0).is_err());
        assert!(RankerModel::train_pairwise(&training(), 201).is_err());
    }

    // Independent implementation of the pre-optimization v1 update loop.
    // Keep repeated tokenization here as a regression oracle, not a benchmark.
    fn reference_weights(samples: &[LabeledEvidence], epochs: u32) -> Vec<f64> {
        let mut groups: BTreeMap<(String, &str), PairGroup<'_>> = BTreeMap::new();
        for item in samples {
            let (positives, negatives) = groups
                .entry((item.repository.trim().to_lowercase(), &item.query))
                .or_default();
            if item.relevant {
                positives.push(item);
            } else {
                negatives.push(item);
            }
        }
        for (positives, negatives) in groups.values_mut() {
            positives.sort_unstable_by(|a, b| a.evidence.id.cmp(&b.evidence.id));
            negatives.sort_unstable_by(|a, b| a.evidence.id.cmp(&b.evidence.id));
        }
        let mut weights = vec![0.0_f64; FEATURE_COUNT];
        for _ in 0..epochs {
            for ((_, query), (positives, negatives)) in &groups {
                if positives.is_empty() || negatives.is_empty() {
                    continue;
                }
                let cycle_length = positives.len().max(negatives.len());
                let pair_count = (cycle_length * 2).min(4096);
                for i in 0..pair_count {
                    let p = super::super::features(query, &positives[i % positives.len()].evidence);
                    let n = super::super::features(
                        query,
                        &negatives[(i + i / cycle_length) % negatives.len()].evidence,
                    );
                    let mut diff = [0.0; FEATURE_COUNT];
                    for j in 0..FEATURE_COUNT {
                        diff[j] = p[j] - n[j];
                    }
                    let logit = weights.iter().zip(diff).map(|(w, d)| w * d).sum();
                    let gradient = 1.0 - sigmoid(logit);
                    for (weight, delta) in weights.iter_mut().zip(diff) {
                        *weight = (*weight + 0.05 * (gradient * delta - 0.001 * *weight))
                            .clamp(-32.0, 32.0);
                    }
                }
            }
        }
        weights
    }

    fn assert_same_weights(samples: &[LabeledEvidence], epochs: u32) {
        let expected = reference_weights(samples, epochs);
        let actual = RankerModel::train_pairwise(samples, epochs).unwrap();
        for (left, right) in actual.weights.iter().zip(expected) {
            assert_eq!(left.to_bits(), right.to_bits());
        }
        assert_eq!(
            actual.training_dataset_sha256,
            Some(training_data_fingerprint(samples).unwrap())
        );
    }

    #[test]
    fn prepared_training_preserves_v1_weights_for_multiple_epochs() {
        let mut data = training();
        data.extend([
            sample(
                " REPO-A ",
                "Which dependency?",
                "E5",
                EvidenceKind::Fact,
                false,
            ),
            sample(
                "repo-a",
                "Which dependency?",
                "E6",
                EvidenceKind::File,
                true,
            ),
            sample(
                "repo-a",
                "Which dependency?",
                "E7",
                EvidenceKind::Module,
                true,
            ),
            sample(
                "repo-b",
                "Which package?",
                "E8",
                EvidenceKind::Command,
                false,
            ),
            sample("repo-c", "Single label", "E9", EvidenceKind::Package, true),
        ]);
        data[0].evidence.detail = "dependency dependency 中文".into();
        data[1].evidence.reference = "Which/Dependency".into();
        data.reverse();
        for epochs in [1, 2, 20, 60, 200] {
            assert_same_weights(&data, epochs);
        }
    }

    #[test]
    fn prepared_pair_schedule_matches_v1_for_asymmetric_and_capped_groups() {
        // Exercise both directions of imbalance, rotation at cycle boundaries,
        // and a group larger than the v1 cap.
        for (positive_count, negative_count) in [
            (1, 1),
            (1, 17),
            (19, 1),
            (7, 11),
            (11, 7),
            (1, 4100),
            (4100, 1),
        ] {
            let mut data = Vec::new();
            for index in 0..(positive_count + negative_count) {
                let kind = if index % 2 == 0 {
                    EvidenceKind::Package
                } else {
                    EvidenceKind::Metric
                };
                let mut row = sample(
                    "repo-a",
                    "Which dependency?",
                    &format!("E{index:05}"),
                    kind,
                    index < positive_count,
                );
                if index % 3 == 0 {
                    row.evidence.detail.push_str(" dependency");
                }
                data.push(row);
            }
            assert_same_weights(&data, 2);
        }
    }

}
