//! Deterministic, locally trainable evidence relevance ranker.
//!
//! It predicts which facts are useful for a task, not prose or program behavior.
//! No external model runtime, pretrained weights, network access, or GPU.

mod annotation;
mod evaluation;
mod families;
mod fingerprint;
mod pair_ordering;
mod pairwise;
mod provenance;
mod scores;
mod thresholds;

pub use annotation::{AnnotationExample, annotation_template, finish_annotations};

pub use evaluation::{
    AnswerabilityEvaluation, RepositoryBalancedEvaluation, RetrievalDelta, RetrievalEvaluation,
    evaluate_original_order, evaluate_repository_balanced, evaluate_repository_stability,
    RepositoryStabilityEvaluation,
};
pub use families::{
    FamilySplitAudit, MAX_FAMILY_MANIFEST_ENTRIES, RepositoryProvenance,
    validate_repository_families,
};
pub use fingerprint::training_data_fingerprint;
pub use pair_ordering::PairOrderingEvaluation;
pub use pairwise::PAIRWISE_MODEL_FORMAT;
pub use provenance::{checkpoint_fingerprint, family_manifest_fingerprint};
pub use scores::{RelevanceScoreBin, RelevanceScoreEvaluation, SCORE_BIN_COUNT};

use crate::context::{EvidenceItem, EvidenceKind};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Versioned JSON checkpoint identifier.
pub const MODEL_FORMAT: &str = "repodna-evidence-ranker/1";
/// Bias, two overlap features and ten one-hot evidence kinds.
pub const FEATURE_COUNT: usize = 13;
/// Maximum offline training set size.
pub const MAX_EXAMPLES: usize = 100_000;

/// Human-labeled relevance judgment; callers are responsible for dataset licensing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabeledEvidence {
    /// Source repository ID for leakage-safe holdout splits.
    pub repository: String,
    /// Question or task.
    pub query: String,
    /// A concrete analysis fact and its evidence ID.
    pub evidence: EvidenceItem,
    /// Whether this fact is relevant to the query.
    pub relevant: bool,
}

/// Small self-contained linear model; not a generative language model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RankerModel {
    /// Model schema version.
    pub format: String,
    /// Finite, bounded weights.
    pub weights: Vec<f64>,
    /// Count of labeled training samples.
    pub training_examples: usize,
    /// Count of distinct repositories in training.
    pub training_repositories: usize,
    /// Optional SHA-256 of canonical human-reviewed training records; absent in legacy checkpoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training_dataset_sha256: Option<String>,
}

/// Labeled holdout evaluation, not a measure of factual truth.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankerEvaluation {
    /// Count of holdout samples.
    pub samples: usize,
    /// Correct classifications at 0.5 threshold.
    pub correct: usize,
    /// Correct positive predictions.
    pub true_positives: usize,
    /// Incorrect positive predictions.
    pub false_positives: usize,
    /// Relevant examples not selected.
    pub false_negatives: usize,
}

impl RankerEvaluation {
    /// Fraction of correct classifications.
    pub fn accuracy(self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            self.correct as f64 / self.samples as f64
        }
    }
}

fn terms(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.chars().count() >= 2)
        .map(str::to_lowercase)
        .collect()
}

fn overlap(query: &HashSet<String>, candidate: &str) -> f64 {
    if query.is_empty() {
        return 0.0;
    }
    // Collect only *matching* unique candidate tokens rather than allocating
    // a set for every token in possibly long evidence descriptions. This must
    // retain the same case-folding and distinct-token semantics as terms().
    let mut matched = HashSet::new();
    for token in candidate
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| token.chars().count() >= 2)
    {
        let normalized = token.to_lowercase();
        if query.contains(&normalized) {
            matched.insert(normalized);
            if matched.len() == query.len() {
                // All query terms are accounted for; later tokens cannot
                // increase overlap or change the score.
                break;
            }
        }
    }
    matched.len() as f64 / query.len() as f64
}

fn kind_index(kind: EvidenceKind) -> usize {
    match kind {
        EvidenceKind::Fact => 3,
        EvidenceKind::File => 4,
        EvidenceKind::Module => 5,
        EvidenceKind::Edge => 6,
        EvidenceKind::Package => 7,
        EvidenceKind::Commit => 8,
        EvidenceKind::Metric => 9,
        EvidenceKind::Finding => 10,
        EvidenceKind::Command => 11,
        EvidenceKind::Excerpt => 12,
    }
}

/// Identical feature vector for a previously tokenized task query. This
/// avoids splitting and allocating the same query tokens for every candidate.
fn features_with_terms(query_terms: &HashSet<String>, item: &EvidenceItem) -> [f64; FEATURE_COUNT] {
    let mut features = [0.0; FEATURE_COUNT];
    features[0] = 1.0;
    features[1] = overlap(query_terms, &item.reference);
    features[2] = overlap(query_terms, &item.detail);
    features[kind_index(item.kind)] = 1.0;
    features
}

/// Fixed inspectable feature extraction; never executes repository text.
pub fn features(query: &str, item: &EvidenceItem) -> [f64; FEATURE_COUNT] {
    features_with_terms(&terms(query), item)
}

fn sigmoid(v: f64) -> f64 {
    1.0 / (1.0 + (-v.clamp(-32.0, 32.0)).exp())
}

/// Normalize user-supplied repository identifiers before applying leakage checks.
/// Whitespace and simple casing differences must not create fake holdout repos.
fn normalized_repository_id(id: &str) -> String {
    id.trim().to_lowercase()
}

/// Refuse repeated or missing evidence IDs within one repository/question.
/// Duplicates can bias training, and conflicting labels should never be silently accepted.
fn validate_evidence_ids(samples: &[LabeledEvidence]) -> Result<(), String> {
    let mut seen = HashSet::with_capacity(samples.len());
    for sample in samples {
        if sample.evidence.id.trim().is_empty() {
            return Err("ranker dataset evidence IDs must not be empty".into());
        }
        if sample.evidence.kind == EvidenceKind::Excerpt {
            return Err("ranker datasets must not contain source excerpts".into());
        }
        if !seen.insert((
            normalized_repository_id(&sample.repository),
            sample.query.as_str(),
            sample.evidence.id.as_str(),
        )) {
            return Err("duplicate evidence ID within repository/question group".into());
        }
    }
    Ok(())
}

/// Reject samples from the same repository in training and holdout datasets.
pub fn validate_repository_split(
    train: &[LabeledEvidence],
    test: &[LabeledEvidence],
) -> Result<(), String> {
    if train.is_empty() || test.is_empty() {
        return Err("training and holdout must both be nonempty".into());
    }
    let repo_ids: HashSet<String> = train
        .iter()
        .map(|sample| normalized_repository_id(&sample.repository))
        .collect();
    if train.iter().chain(test.iter()).any(|s| {
        s.repository.trim().is_empty()
            || s.query.trim().is_empty()
            || s.evidence.detail.trim().is_empty()
    }) {
        return Err("every example needs a repository, query, and evidence detail".into());
    }
    if test
        .iter()
        .any(|sample| repo_ids.contains(&normalized_repository_id(&sample.repository)))
    {
        return Err("repository leakage: training and test repository IDs overlap".into());
    }
    validate_evidence_ids(train)?;
    validate_evidence_ids(test)?;
    Ok(())
}

impl RankerModel {
    /// Strictly validate checkpoint metadata and numeric weights.
    pub fn validate(&self) -> Result<(), String> {
        if self.format != MODEL_FORMAT && self.format != PAIRWISE_MODEL_FORMAT {
            return Err("unsupported ranker checkpoint format".into());
        }
        if self.weights.len() != FEATURE_COUNT {
            return Err("incorrect number of ranker weights".into());
        }
        if self.training_examples == 0 || self.training_repositories == 0 {
            return Err("ranker has no training provenance".into());
        }
        if self
            .training_dataset_sha256
            .as_ref()
            .is_some_and(|fingerprint| {
                fingerprint.len() != 64
                    || !fingerprint
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
        {
            return Err(
                "training dataset fingerprint must be a lowercase SHA-256 hex digest".into(),
            );
        }
        if self
            .weights
            .iter()
            .any(|w| !w.is_finite() || w.abs() > 32.0)
        {
            return Err("ranker weights must be finite and bounded".into());
        }
        Ok(())
    }

    /// Fit logistic relevance weights using deterministic bounded SGD.
    pub fn train(samples: &[LabeledEvidence], epochs: u32) -> Result<Self, String> {
        if !(2..=MAX_EXAMPLES).contains(&samples.len()) {
            return Err("training set size is outside supported bounds".into());
        }
        if !(1..=200).contains(&epochs) {
            return Err("epochs must be 1..=200".into());
        }
        if !samples.iter().any(|s| s.relevant) || !samples.iter().any(|s| !s.relevant) {
            return Err("training requires positive and negative labels".into());
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
            .map(|sample| normalized_repository_id(&sample.repository))
            .collect::<HashSet<_>>()
            .len();
        // SGD is sensitive to record order. Sort a borrowed view of the
        // validated labels so JSON row ordering cannot change exported weights.
        let mut ordered: Vec<&LabeledEvidence> = samples.iter().collect();
        ordered.sort_unstable_by(|left, right| {
            left.repository
                .cmp(&right.repository)
                .then(left.query.cmp(&right.query))
                .then(left.evidence.id.cmp(&right.evidence.id))
        });
        // Feature extraction depends on frozen labels and the task, not on
        // SGD weights. Build each vector once instead of repeating Unicode
        // tokenization and overlap calculation every training epoch.
        // Keep the original deterministic label order and floating-point
        // operation order unchanged to preserve exact checkpoint weights.
        let prepared = ordered
            .iter()
            .map(|sample| (features(&sample.query, &sample.evidence), sample.relevant))
            .collect::<Vec<_>>();
        let mut weights = vec![0.0_f64; FEATURE_COUNT];
        for _ in 0..epochs {
            for &(x, relevant) in &prepared {
                let logit = weights.iter().zip(x).map(|(w, x)| w * x).sum::<f64>();
                let target = if relevant { 1.0 } else { 0.0 };
                let error = target - sigmoid(logit);
                for (w, feature) in weights.iter_mut().zip(x) {
                    *w = (*w + 0.05 * (error * feature - 0.001 * *w)).clamp(-32.0, 32.0);
                }
            }
        }
        let result = Self {
            format: MODEL_FORMAT.into(),
            weights,
            training_examples: samples.len(),
            training_repositories,
            training_dataset_sha256: Some(training_data_fingerprint(samples)?),
        };
        result.validate()?;
        Ok(result)
    }

    /// Reuse the exact query-token set for repeated candidate scoring.
    fn relevance_with_terms(&self, query_terms: &HashSet<String>, item: &EvidenceItem) -> f64 {
        self.weights
            .iter()
            .zip(features_with_terms(query_terms, item))
            .map(|(w, x)| w * x)
            .sum::<f64>()
            .pipe_sigmoid()
    }

    /// Sigmoid-transformed ranking score; pairwise values are not calibrated probabilities.
    pub fn relevance(&self, query: &str, item: &EvidenceItem) -> f64 {
        self.relevance_with_terms(&terms(query), item)
    }

    /// Indices sorted by relevance; stable original order on ties.
    ///
    /// Score each candidate once before sorting. Re-scoring inside the
    /// comparator recomputes token overlap O(n log n) times for a single
    /// explanation; cached scores make comparisons cheap and deterministic.
    pub fn rank_indices(&self, query: &str, evidence: &[EvidenceItem]) -> Vec<usize> {
        let query_terms = terms(query);
        let mut scored = evidence
            .iter()
            .enumerate()
            .map(|(index, item)| (index, self.relevance_with_terms(&query_terms, item)))
            .collect::<Vec<_>>();
        scored.sort_by(|(left_index, left_score), (right_index, right_score)| {
            right_score
                .total_cmp(left_score)
                .then(left_index.cmp(right_index))
        });
        scored.into_iter().map(|(index, _)| index).collect()
    }

    /// Score labels not used during training without changing weights.
    pub fn evaluate(&self, holdout: &[LabeledEvidence]) -> RankerEvaluation {
        let mut result = RankerEvaluation {
            samples: holdout.len(),
            correct: 0,
            true_positives: 0,
            false_positives: 0,
            false_negatives: 0,
        };
        for sample in holdout {
            let yes = self.relevance(&sample.query, &sample.evidence) >= 0.5;
            if yes == sample.relevant {
                result.correct += 1;
            }
            match (yes, sample.relevant) {
                (true, true) => result.true_positives += 1,
                (true, false) => result.false_positives += 1,
                (false, true) => result.false_negatives += 1,
                (false, false) => {}
            }
        }
        result
    }
}

/// Private helper to keep numeric computation readable.
trait SigmoidExt {
    fn pipe_sigmoid(self) -> f64;
}
impl SigmoidExt for f64 {
    fn pipe_sigmoid(self) -> f64 {
        sigmoid(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_only_overlap_matches_full_candidate_set_reference() {
        // Keep the previous full-set algorithm as an independent oracle.
        fn reference(query: &HashSet<String>, candidate: &str) -> f64 {
            if query.is_empty() {
                return 0.0;
            }
            query.intersection(&terms(candidate)).count() as f64 / query.len() as f64
        }
        let cases = [
            ("", "build dependency"),
            ("build dependencies", "BUILD build b b dependencies"),
            ("build dependency", "unrelated tokens only"),
            ("C++ C#", "C c++ c#"),
            ("中文 构建 build", "中文 中文 构建 build 工具"),
            ("İstanbul STRASSE", "İSTANBUL strasse İstanbul"),
            ("a b one two", "a b a b one one TWO"),
            ("build module", "build-module,build module"),
        ];
        for (query, candidate) in cases {
            let query_terms = terms(query);
            assert_eq!(
                overlap(&query_terms, candidate).to_bits(),
                reference(&query_terms, candidate).to_bits(),
                "query={query:?}, candidate={candidate:?}"
            );
        }
        // A long evidence detail with many distinct nonmatching words and
        // duplicates must not inflate relevance or change the score.
        let query = terms("build package");
        let candidate = format!(
            "{} build build",
            (0..2_000)
                .map(|index| format!("unrelated{index}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        assert_eq!(overlap(&query, &candidate), 0.5);
        assert_eq!(overlap(&query, &candidate), reference(&query, &candidate));
        let all_matched = "package build ".repeat(1_000);
        assert_eq!(overlap(&query, &all_matched), 1.0);
        assert_eq!(
            overlap(&query, &all_matched),
            reference(&query, &all_matched)
        );
    }

    fn sample(
        repo: &str,
        query: &str,
        kind: EvidenceKind,
        detail: &str,
        relevant: bool,
    ) -> LabeledEvidence {
        LabeledEvidence {
            repository: repo.into(),
            query: query.into(),
            evidence: EvidenceItem {
                id: format!("E{}", kind_index(kind)),
                kind,
                reference: "src/main.rs".into(),
                detail: detail.into(),
            },
            relevant,
        }
    }
    fn training() -> Vec<LabeledEvidence> {
        vec![
            sample(
                "r1",
                "dependency package versions",
                EvidenceKind::Package,
                "Dependency package versions in lockfile",
                true,
            ),
            sample(
                "r1",
                "dependency package versions",
                EvidenceKind::Commit,
                "Contributor activity over time",
                false,
            ),
            sample(
                "r1",
                "build commands",
                EvidenceKind::Command,
                "Build commands detected in package manifest",
                true,
            ),
            sample(
                "r1",
                "build commands",
                EvidenceKind::Finding,
                "Code complexity finding",
                false,
            ),
            sample(
                "r2",
                "security finding",
                EvidenceKind::Finding,
                "Security finding in configuration",
                true,
            ),
            sample(
                "r2",
                "security finding",
                EvidenceKind::Metric,
                "Code lines by language",
                false,
            ),
        ]
    }
    #[test]
    fn learns_relevance_deterministically() {
        let data = training();
        let m = RankerModel::train(&data, 60).unwrap();
        assert_eq!(m, RankerModel::train(&data, 60).unwrap());
        assert_eq!(m.training_repositories, 2);
        assert!(
            m.relevance(&data[0].query, &data[0].evidence)
                > m.relevance(&data[0].query, &data[1].evidence)
        );
        let candidates = vec![data[1].evidence.clone(), data[0].evidence.clone()];
        assert_eq!(m.rank_indices(&data[0].query, &candidates), vec![1, 0]);
        let serialized = serde_json::to_string(&m).unwrap();
        let decoded: RankerModel = serde_json::from_str(&serialized).unwrap();
        assert_eq!(decoded, m);
    }
    #[test]
    fn cached_inference_scores_preserve_legacy_comparator_order_and_ties() {
        let model = RankerModel::train(&training(), 60).unwrap();
        let kinds = [
            EvidenceKind::Fact,
            EvidenceKind::Module,
            EvidenceKind::Package,
            EvidenceKind::Commit,
            EvidenceKind::Finding,
            EvidenceKind::Command,
            EvidenceKind::Metric,
        ];
        let evidence = (0..175)
            .map(|index| EvidenceItem {
                id: format!("E{}", index + 1),
                kind: kinds[index % kinds.len()],
                reference: match index % 3 {
                    0 => "Cargo.lock".into(),
                    1 => "src/main.rs".into(),
                    _ => "README".into(),
                },
                detail: match index % 4 {
                    0 => "Dependency package versions are locked".into(),
                    1 => "Recorded build commands and targets".into(),
                    2 => "Measured historical changes".into(),
                    _ => "Measured historical changes".into(),
                },
            })
            .collect::<Vec<_>>();

        for query in [
            "",
            "dependency package versions",
            "build commands",
            "changes in README",
            "unseen token",
            "非英语任务 build",
        ] {
            // Reference comparator from the previous implementation, retained
            // in this test to detect altered ordering semantics.
            let mut expected: Vec<usize> = (0..evidence.len()).collect();
            expected.sort_by(|&left, &right| {
                model
                    .relevance(query, &evidence[right])
                    .total_cmp(&model.relevance(query, &evidence[left]))
                    .then(left.cmp(&right))
            });

            assert_eq!(model.rank_indices(query, &evidence), expected);
            assert_eq!(model.rank_indices(query, &evidence), expected);
        }
        // Equal candidate scores must keep input-order index ties, even
        // when the score is generated by the same model feature vector.
        let repeats = vec![
            evidence[0].clone(),
            evidence[0].clone(),
            evidence[0].clone(),
        ];
        assert_eq!(model.rank_indices("anything", &repeats), vec![0, 1, 2]);
        assert!(model.rank_indices("anything", &[]).is_empty());
        assert_eq!(model.rank_indices("anything", &evidence[..1]), vec![0]);
    }

    #[test]
    fn cached_query_tokens_match_public_features_and_scoring_bit_for_bit() {
        let model = RankerModel::train(&training(), 60).unwrap();
        let inputs = training()
            .into_iter()
            .map(|label| label.evidence)
            .collect::<Vec<_>>();
        for query in [
            "",
            "dependencies and build commands",
            "Cargo.lock cargo.lock CARGO.LOCK",
            "C++ C# build!",
            "中英文 mixed identifiers",
            "a b c",
            "spaced words\nwith delimiters",
        ] {
            let query_terms = terms(query);
            for item in &inputs {
                assert_eq!(
                    features(query, item),
                    features_with_terms(&query_terms, item)
                );
                assert_eq!(
                    model.relevance(query, item).to_bits(),
                    model.relevance_with_terms(&query_terms, item).to_bits()
                );
            }
        }
        // Input order and the sigmoid score must not vary with repeated
        // calls or duplicate relevant-candidate feature vectors.
        let query = "build commands";
        let items = vec![
            inputs[0].clone(),
            inputs[0].clone(),
            inputs[1].clone(),
            inputs[0].clone(),
        ];
        let mut legacy = (0..items.len()).collect::<Vec<_>>();
        legacy.sort_by(|&left, &right| {
            model
                .relevance(query, &items[right])
                .total_cmp(&model.relevance(query, &items[left]))
                .then(left.cmp(&right))
        });
        assert_eq!(model.rank_indices(query, &items), legacy);
    }

    #[test]
    fn cached_training_features_match_original_sgd_weights_exactly() {
        // Preserve this pre-optimization training loop as a reference,
        // including the order of all floating-point operations.
        fn legacy_weights(samples: &[LabeledEvidence], epochs: u32) -> Vec<f64> {
            let mut ordered: Vec<&LabeledEvidence> = samples.iter().collect();
            ordered.sort_unstable_by(|left, right| {
                left.repository
                    .cmp(&right.repository)
                    .then(left.query.cmp(&right.query))
                    .then(left.evidence.id.cmp(&right.evidence.id))
            });
            let mut weights = vec![0.0_f64; FEATURE_COUNT];
            for _ in 0..epochs {
                for sample in &ordered {
                    let x = features(&sample.query, &sample.evidence);
                    let logit = weights.iter().zip(x).map(|(w, x)| w * x).sum::<f64>();
                    let target = if sample.relevant { 1.0 } else { 0.0 };
                    let error = target - sigmoid(logit);
                    for (w, feature) in weights.iter_mut().zip(x) {
                        *w = (*w + 0.05 * (error * feature - 0.001 * *w)).clamp(-32.0, 32.0);
                    }
                }
            }
            weights
        }

        let mut cases = vec![training()];
        let mut shuffled = training();
        shuffled.reverse();
        shuffled[0].query = "中文 dependency build // test".into();
        cases.push(shuffled);
        for samples in &cases {
            for epochs in [1, 2, 7, 60, 200] {
                let learned = RankerModel::train(samples, epochs).unwrap();
                let original = legacy_weights(samples, epochs);
                let actual_bits = learned
                    .weights
                    .iter()
                    .map(|w| w.to_bits())
                    .collect::<Vec<_>>();
                let expected_bits = original.iter().map(|w| w.to_bits()).collect::<Vec<_>>();
                assert_eq!(
                    actual_bits, expected_bits,
                    "checkpoint drift at epoch {epochs}"
                );
                assert_eq!(learned.training_examples, samples.len());
                assert_eq!(
                    learned.training_dataset_sha256,
                    Some(training_data_fingerprint(samples).unwrap())
                );
            }
        }
    }

    #[test]
    fn training_is_invariant_to_json_row_order() {
        let originals = training();
        let mut reordered = originals.clone();
        reordered.reverse();
        reordered.rotate_left(2);
        let first = RankerModel::train(&originals, 60).unwrap();
        let second = RankerModel::train(&reordered, 60).unwrap();
        assert_eq!(first, second);
        assert_eq!(originals, training());
    }

    #[test]
    fn prevents_leakage_and_invalid_checkpoints() {
        let data = training();
        let holdout = vec![sample(
            "r3",
            "build commands",
            EvidenceKind::Command,
            "Build commands",
            true,
        )];
        validate_repository_split(&data, &holdout).unwrap();
        assert!(validate_repository_split(&data, &data).is_err());
        let mut m = RankerModel::train(&data, 10).unwrap();
        assert_eq!(m.evaluate(&holdout).samples, 1);
        m.weights[0] = f64::NAN;
        assert!(m.validate().is_err());
        assert!(RankerModel::train(&data, 0).is_err());
        assert!(RankerModel::train(&data[..1], 10).is_err());
    }
    #[test]
    fn rejects_duplicate_training_or_holdout_labels() {
        let training = training();
        let mut duplicate_train = training.clone();
        let mut repeated = training[0].clone();
        repeated.relevant = false;
        duplicate_train.push(repeated);
        assert!(
            RankerModel::train(&duplicate_train, 10)
                .unwrap_err()
                .contains("duplicate evidence ID")
        );

        let holdout = sample(
            "r3",
            "build commands",
            EvidenceKind::Command,
            "Build command from manifest",
            true,
        );
        let duplicate_holdout = vec![holdout.clone(), holdout.clone()];
        assert!(
            validate_repository_split(&training, &duplicate_holdout)
                .unwrap_err()
                .contains("duplicate evidence ID")
        );

        // Evidence IDs may be reused for other questions or repositories.
        let different_question = sample(
            "r3",
            "other question",
            EvidenceKind::Command,
            "Different selected fact",
            false,
        );
        validate_repository_split(&training, &[holdout, different_question]).unwrap();
    }

    #[test]
    fn rejects_missing_ids_and_source_excerpts_in_ranker_datasets() {
        let training = training();
        let mut missing_id = training.clone();
        missing_id[0].evidence.id.clear();
        assert!(RankerModel::train(&missing_id, 10).is_err());

        let mut excerpt = training.clone();
        excerpt[0].evidence.kind = EvidenceKind::Excerpt;
        assert!(RankerModel::train(&excerpt, 10).is_err());

        let valid_holdout = vec![sample(
            "r3",
            "build commands",
            EvidenceKind::Command,
            "Build commands",
            true,
        )];
        let mut excerpt_holdout = valid_holdout.clone();
        excerpt_holdout[0].evidence.kind = EvidenceKind::Excerpt;
        assert!(validate_repository_split(&training, &excerpt_holdout).is_err());
        validate_repository_split(&training, &valid_holdout).unwrap();
    }

    #[test]
    fn rejects_repository_aliases_across_train_and_holdout() {
        let training = training();
        let mut aliased_holdout = sample(
            " R1 ",
            "another query",
            EvidenceKind::Fact,
            "Different evidence about the same repository",
            true,
        );
        assert!(
            validate_repository_split(&training, &[aliased_holdout.clone()])
                .unwrap_err()
                .contains("repository leakage")
        );
        aliased_holdout.repository = "r1".into();
        assert!(validate_repository_split(&training, &[aliased_holdout]).is_err());

        let mut independent_holdout = sample(
            " r3 ",
            "another query",
            EvidenceKind::Fact,
            "Independent repository evidence",
            true,
        );
        validate_repository_split(&training, &[independent_holdout.clone()]).unwrap();
        independent_holdout.repository = "R3".into();
        validate_repository_split(&training, &[independent_holdout]).unwrap();
    }

    #[test]
    fn canonical_repository_ids_prevent_duplicate_labels_and_count_aliases_once() {
        let training = training();
        let mut conflicting = training.clone();
        let mut alias = training[0].clone();
        alias.repository = " R1 ".into();
        alias.relevant = !alias.relevant;
        conflicting.push(alias);
        assert!(
            RankerModel::train(&conflicting, 1)
                .unwrap_err()
                .contains("duplicate evidence ID")
        );

        let mut aliased = training;
        aliased[0].repository = " R1 ".into();
        aliased[1].repository = "R1".into();
        let model = RankerModel::train(&aliased, 1).unwrap();
        assert_eq!(model.training_repositories, 2);
    }

    #[test]
    fn checkpoint_fingerprint_roundtrips_and_rejects_invalid_metadata() {
        let data = training();
        let model = RankerModel::train(&data, 10).unwrap();
        let digest = model.training_dataset_sha256.as_ref().unwrap();
        assert_eq!(digest, &training_data_fingerprint(&data).unwrap());
        let restored: RankerModel =
            serde_json::from_str(&serde_json::to_string(&model).unwrap()).unwrap();
        assert_eq!(restored.format, model.format);
        assert_eq!(restored.training_examples, model.training_examples);
        assert_eq!(restored.training_repositories, model.training_repositories);
        assert_eq!(
            restored.training_dataset_sha256,
            model.training_dataset_sha256
        );
        assert_eq!(restored.weights.len(), model.weights.len());
        // serde_json can round the last bit of some floating-point values.
        // The serialized checkpoint must preserve usable weights, not their
        // exact in-memory bit pattern before serialization.
        assert!(
            restored
                .weights
                .iter()
                .zip(&model.weights)
                .all(|(read, saved)| (read - saved).abs() <= 1e-12)
        );

        let mut invalid = model.clone();
        invalid.training_dataset_sha256 = Some("not-a-sha256".into());
        assert!(invalid.validate().is_err());
        invalid.training_dataset_sha256 = Some("A".repeat(64));
        assert!(invalid.validate().is_err());

        // Old v1 JSON without the optional field remains readable but is
        // explicitly distinguished from fingerprints generated by new trainers.
        let mut old: serde_json::Value = serde_json::to_value(&model).unwrap();
        old.as_object_mut()
            .unwrap()
            .remove("training_dataset_sha256");
        let legacy: RankerModel = serde_json::from_value(old).unwrap();
        assert!(legacy.training_dataset_sha256.is_none());
        legacy.validate().unwrap();
    }

    #[test]
    fn handles_non_english_text_without_nan() {
        let sample = sample(
            "r1",
            "मॉड्यूल परीक्षण",
            EvidenceKind::Module,
            "मॉड्यूल परीक्षण",
            true,
        );
        let f = features(&sample.query, &sample.evidence);
        assert_eq!(f[0], 1.0);
        assert_eq!(f[5], 1.0);
        assert!(f[2] > 0.0);
        assert!(f.iter().all(|v| v.is_finite()));
    }
}
