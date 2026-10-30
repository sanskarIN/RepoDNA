//! Stable, row-order-independent SHA-256 for human-reviewed ranker labels.
//!
//! This binds a checkpoint to a particular labeled dataset's *contents*.
//! It is not a signature, proof of labeling quality, or proof of honest training.

use sha2::{Digest, Sha256};

use super::{LabeledEvidence, validate_evidence_ids};

/// Fingerprint the complete typed training data independent of JSON formatting
/// and array row order. All evidence fields, questions, repository IDs, and
/// relevance labels are included exactly as provided.
///
/// Length prefixes avoid ambiguous record boundaries; a domain separator
/// prevents accidental reuse with a different hash purpose.
pub fn training_data_fingerprint(samples: &[LabeledEvidence]) -> Result<String, String> {
    if samples.is_empty() {
        return Err("training fingerprint requires nonempty labels".into());
    }
    validate_evidence_ids(samples)?;
    let mut rows: Vec<Vec<u8>> = samples
        .iter()
        .map(|sample| {
            serde_json::to_vec(sample)
                .map_err(|error| format!("unable to serialize training label: {error}"))
        })
        .collect::<Result<_, _>>()?;
    rows.sort_unstable();
    let mut digest = Sha256::new();
    digest.update(b"repodna-training-labels/v1\0");
    for row in rows {
        digest.update((row.len() as u64).to_le_bytes());
        digest.update(row);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EvidenceItem, EvidenceKind};

    fn labels() -> Vec<LabeledEvidence> {
        vec![
            LabeledEvidence {
                repository: "repo-one".into(),
                query: "What are the dependencies?".into(),
                evidence: EvidenceItem {
                    id: "E1".into(),
                    kind: EvidenceKind::Package,
                    reference: "Cargo.toml".into(),
                    detail: "A dependency was recorded".into(),
                },
                relevant: true,
            },
            LabeledEvidence {
                repository: "repo-one".into(),
                query: "What are the dependencies?".into(),
                evidence: EvidenceItem {
                    id: "E2".into(),
                    kind: EvidenceKind::Metric,
                    reference: "src".into(),
                    detail: "A line count was recorded".into(),
                },
                relevant: false,
            },
        ]
    }

    #[test]
    fn stable_across_input_row_order_and_json_whitespace() {
        let original = labels();
        let mut reordered = original.clone();
        reordered.reverse();
        assert_eq!(
            training_data_fingerprint(&original).unwrap(),
            training_data_fingerprint(&reordered).unwrap()
        );
        let pretty = serde_json::to_string_pretty(&original).unwrap();
        let parsed: Vec<LabeledEvidence> = serde_json::from_str(&pretty).unwrap();
        assert_eq!(
            training_data_fingerprint(&original).unwrap(),
            training_data_fingerprint(&parsed).unwrap()
        );
    }

    #[test]
    fn label_and_evidence_changes_invalidate_fingerprint() {
        let original = labels();
        let hash = training_data_fingerprint(&original).unwrap();
        assert_eq!(hash.len(), 64);
        assert!(hash.bytes().all(|c| c.is_ascii_hexdigit()));
        let mut changed_label = original.clone();
        changed_label[0].relevant = false;
        assert_ne!(hash, training_data_fingerprint(&changed_label).unwrap());
        let mut changed_evidence = original.clone();
        changed_evidence[0].evidence.detail.push('!');
        assert_ne!(hash, training_data_fingerprint(&changed_evidence).unwrap());
    }

    #[test]
    fn rejects_invalid_inputs_before_hashing() {
        assert!(training_data_fingerprint(&[]).is_err());
        let original = labels();
        let mut duplicated = original.clone();
        duplicated.push(original[0].clone());
        assert!(training_data_fingerprint(&duplicated).is_err());
        let mut excerpt = original;
        excerpt[0].evidence.kind = EvidenceKind::Excerpt;
        assert!(training_data_fingerprint(&excerpt).is_err());
    }
}
