//! The Project DNA fingerprint: a characterization of a repository, not a quality score.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::confidence::Confidence;

/// One dimension of the DNA fingerprint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DnaDimension {
    /// Identifier, e.g. `language-diversity`.
    pub id: String,
    /// Label, e.g. `Language diversity`.
    pub label: String,
    /// Normalized value (0–1). Higher is not "better"; it only positions the repository.
    pub value: f64,
    /// Raw measurement before normalization.
    pub raw: f64,
    /// Unit of `raw`.
    pub unit: String,
    /// What the dimension describes and how it was normalized.
    pub description: String,
    /// Confidence (`unavailable` when the underlying analysis did not run).
    pub confidence: Confidence,
}

/// Compact identity and characterization of a repository snapshot.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DnaFingerprint {
    /// Deterministic snapshot identifier: `rdna1-` followed by 32 hexadecimal characters.
    /// It identifies content; it is not a security primitive.
    pub dna_hash: String,
    /// Characterization dimensions.
    #[serde(default)]
    pub dimensions: Vec<DnaDimension>,
    /// How the hash and dimensions were computed.
    pub method: String,
}

impl DnaFingerprint {
    /// Looks up a dimension by identifier.
    pub fn dimension(&self, id: &str) -> Option<&DnaDimension> {
        self.dimensions.iter().find(|dimension| dimension.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_dimensions() {
        let fingerprint = DnaFingerprint {
            dna_hash: "rdna1-00".into(),
            dimensions: vec![DnaDimension {
                id: "size".into(),
                label: "Size".into(),
                value: 0.5,
                raw: 1000.0,
                unit: "code lines".into(),
                description: "log scale".into(),
                confidence: Confidence::High,
            }],
            method: "test".into(),
        };
        assert!(fingerprint.dimension("size").is_some());
        assert!(fingerprint.dimension("age").is_none());
    }
}
