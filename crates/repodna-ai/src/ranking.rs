//! Deterministic, locally trainable evidence relevance ranker.
//!
//! It predicts which facts are useful for a task, not prose or program behavior.
//! No external model runtime, pretrained weights, network access, or GPU.

mod annotation;
mod evaluation;

pub use annotation::{AnnotationExample, annotation_template, finish_annotations};

pub use evaluation::{
    AnswerabilityEvaluation, RetrievalDelta, RetrievalEvaluation, evaluate_original_order,
};

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
    query.intersection(&terms(candidate)).count() as f64 / query.len() as f64
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

/// Fixed inspectable feature extraction; never executes repository text.
pub fn features(query: &str, item: &EvidenceItem) -> [f64; FEATURE_COUNT] {
    let query_terms = terms(query);
    let mut features = [0.0; FEATURE_COUNT];
    features[0] = 1.0;
    features[1] = overlap(&query_terms, &item.reference);
    features[2] = overlap(&query_terms, &item.detail);
    features[kind_index(item.kind)] = 1.0;
    features
}

fn sigmoid(v: f64) -> f64 {
    1.0 / (1.0 + (-v.clamp(-32.0, 32.0)).exp())
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
            sample.repository.as_str(),
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
    let repo_ids: HashSet<&str> = train.iter().map(|s| s.repository.as_str()).collect();
    if train.iter().chain(test.iter()).any(|s| {
        s.repository.trim().is_empty()
            || s.query.trim().is_empty()
            || s.evidence.detail.trim().is_empty()
    }) {
        return Err("every example needs a repository, query, and evidence detail".into());
    }
    if test
        .iter()
        .any(|s| repo_ids.contains(s.repository.as_str()))
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
        if self.format != MODEL_FORMAT {
            return Err("unsupported ranker checkpoint format".into());
        }
        if self.weights.len() != FEATURE_COUNT {
            return Err("incorrect number of ranker weights".into());
        }
        if self.training_examples == 0 || self.training_repositories == 0 {
            return Err("ranker has no training provenance".into());
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
            .map(|s| s.repository.as_str())
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
        let result = Self {
            format: MODEL_FORMAT.into(),
            weights,
            training_examples: samples.len(),
            training_repositories,
        };
        result.validate()?;
        Ok(result)
    }

    /// Ranking probability, not a calibrated semantic confidence score.
    pub fn relevance(&self, query: &str, item: &EvidenceItem) -> f64 {
        self.weights
            .iter()
            .zip(features(query, item))
            .map(|(w, x)| w * x)
            .sum::<f64>()
            .pipe_sigmoid()
    }

    /// Indices sorted by relevance; stable original order on ties.
    pub fn rank_indices(&self, query: &str, evidence: &[EvidenceItem]) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..evidence.len()).collect();
        indices.sort_by(|&a, &b| {
            self.relevance(query, &evidence[b])
                .total_cmp(&self.relevance(query, &evidence[a]))
                .then(a.cmp(&b))
        });
        indices
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
