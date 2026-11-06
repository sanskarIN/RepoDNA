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
    Ok(hex_digest(digest))
}

/// Preserve row order when identifying an evaluation split. The existing
/// canonical label fingerprint deliberately sorts rows for training identity,
/// but evaluation baselines and stable score ties may depend on candidate
/// order within a repository/question. This is separate from model weights.
pub fn evaluation_order_fingerprint(samples: &[LabeledEvidence]) -> Result<String, String> {
    if samples.is_empty() {
        return Err("evaluation order fingerprint requires nonempty labels".into());
    }
    validate_evidence_ids(samples)?;
    let mut digest = Sha256::new();
    digest.update(b"repodna-evaluation-label-order/v1\0");
    for sample in samples {
        let row = serde_json::to_vec(sample)
            .map_err(|error| format!("unable to serialize evaluation label: {error}"))?;
        digest.update((row.len() as u64).to_le_bytes());
        digest.update(&row);
    }
    Ok(hex_digest(digest))
}

fn hex_digest(digest: Sha256) -> String {
    // digest::Output implements byte iteration but not LowerHex in sha2 0.11.
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in digest.finalize() {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
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
    fn evaluation_order_identity_detects_reordering_but_not_json_layout() {
        let original = labels();
        let mut reordered = original.clone();
        reordered.reverse();
        let source_hash = training_data_fingerprint(&original).unwrap();
        assert_eq!(source_hash, training_data_fingerprint(&reordered).unwrap());

        let original_order = evaluation_order_fingerprint(&original).unwrap();
        assert_eq!(original_order.len(), 64);
        assert_ne!(original_order, source_hash);
        assert_ne!(
            original_order,
            evaluation_order_fingerprint(&reordered).unwrap()
        );
        let pretty = serde_json::to_string_pretty(&original).unwrap();
        let parsed: Vec<LabeledEvidence> = serde_json::from_str(&pretty).unwrap();
        assert_eq!(
            original_order,
            evaluation_order_fingerprint(&parsed).unwrap()
        );

        let mut changed = original;
        changed[0].relevant = !changed[0].relevant;
        assert_ne!(
            original_order,
            evaluation_order_fingerprint(&changed).unwrap()
        );
    }

    #[test]
    fn evaluation_order_identity_rejects_unreviewed_or_duplicate_inputs() {
        assert!(evaluation_order_fingerprint(&[]).is_err());
        let mut sample = labels();
        sample.push(sample[0].clone());
        assert!(evaluation_order_fingerprint(&sample).is_err());

        let mut sample = labels();
        sample[0].evidence.kind = EvidenceKind::Excerpt;
        assert!(evaluation_order_fingerprint(&sample).is_err());
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
