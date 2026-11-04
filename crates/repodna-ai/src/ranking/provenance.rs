//! Domain-separated, content-based fingerprints for local benchmark provenance.
//!
//! These are SHA-256 identifiers, NOT signatures, lineage verification,
//! consent records, or independent evidence that a model was trained honestly.

use std::collections::HashSet;

use sha2::{Digest, Sha256};

use super::{MAX_FAMILY_MANIFEST_ENTRIES, RankerModel, RepositoryProvenance};

fn hex_digest(digest: Sha256) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in digest.finalize() {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

fn hash_rows(domain: &[u8], mut rows: Vec<Vec<u8>>) -> String {
    rows.sort_unstable();
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for row in rows {
        hasher.update((row.len() as u64).to_le_bytes());
        hasher.update(row);
    }
    hex_digest(hasher)
}

/// Fingerprint the complete typed checkpoint independently of JSON layout.
///
/// Includes every model weight, format and training metadata field. This
/// detects mismatches against a previously saved report, but does NOT
/// authenticate the checkpoint's author, training run, or training labels.
pub fn checkpoint_fingerprint(model: &RankerModel) -> Result<String, String> {
    model.validate()?;
    let bytes = serde_json::to_vec(model)
        .map_err(|error| format!("cannot fingerprint ranker checkpoint: {error}"))?;
    Ok(hash_rows(b"repodna-ranker-checkpoint/v1\0", vec![bytes]))
}

/// Stable fingerprint of a reviewer-curated provenance manifest.
///
/// Row order and cosmetic repository/family identifier case or surrounding
/// whitespace do not affect this fingerprint. Git revision hex casing is
/// canonicalized. License expressions are preserved as trimmed strings, since
/// their case and content may have distinct semantics. Invalid or duplicated
/// repository entries fail closed.
pub fn family_manifest_fingerprint(manifest: &[RepositoryProvenance]) -> Result<String, String> {
    if manifest.is_empty() || manifest.len() > MAX_FAMILY_MANIFEST_ENTRIES {
        return Err("invalid family manifest entry count for fingerprint".into());
    }
    let mut seen = HashSet::with_capacity(manifest.len());
    let mut rows = Vec::with_capacity(manifest.len());
    for item in manifest {
        if item.repository.trim().is_empty()
            || item.repository.len() > 200
            || item.family.trim().is_empty()
            || item.family.len() > 200
            || item.license.trim().is_empty()
            || item.license.len() > 128
            || !matches!(item.revision.len(), 40 | 64)
            || !item.revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("invalid family manifest entry for fingerprint".into());
        }
        let repository = item.repository.trim().to_lowercase();
        if !seen.insert(repository.clone()) {
            return Err("duplicate normalized repository ID in family fingerprint".into());
        }
        let family = item.family.trim().to_lowercase();
        let revision = item.revision.to_ascii_lowercase();
        let license = item.license.trim();
        let record = serde_json::to_vec(&(repository, family, revision, license))
            .map_err(|error| format!("cannot fingerprint family record: {error}"))?;
        rows.push(record);
    }
    Ok(hash_rows(b"repodna-family-manifest/v1\0", rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ranking::{FEATURE_COUNT, MODEL_FORMAT};

    fn family(repository: &str, source: &str) -> RepositoryProvenance {
        RepositoryProvenance {
            repository: repository.to_owned(),
            family: source.to_owned(),
            revision: "a".repeat(40),
            license: "MIT".into(),
        }
    }

    #[test]
    fn manifest_fingerprint_ignores_row_order_and_cosmetic_aliases() {
        let first = [family(" repo-a ", "Family-A"), family("repo-b", "family-b")];
        let second = [family("REPO-B", "FAMILY-B "), family("repo-a", "family-a")];
        assert_eq!(
            family_manifest_fingerprint(&first).expect("valid manifest"),
            family_manifest_fingerprint(&second).expect("same manifest")
        );
    }

    #[test]
    fn manifest_changes_and_duplicate_ids_invalidate_fingerprint() {
        let original = [family("repo-a", "family-a"), family("repo-b", "family-b")];
        let hash = family_manifest_fingerprint(&original).expect("valid manifest");
        assert_eq!(hash.len(), 64);

        let mut changed = original.clone();
        changed[0].family = "different-family".into();
        assert_ne!(
            hash,
            family_manifest_fingerprint(&changed).expect("changed family")
        );

        let mut changed = original.clone();
        changed[0].revision = "b".repeat(40);
        assert_ne!(
            hash,
            family_manifest_fingerprint(&changed).expect("changed revision")
        );

        let mut changed = original.clone();
        changed[0].license = "Apache-2.0".into();
        assert_ne!(
            hash,
            family_manifest_fingerprint(&changed).expect("changed license")
        );

        let duplicate = [family("repo-a", "x"), family(" REPO-A ", "y")];
        assert!(family_manifest_fingerprint(&duplicate).is_err());
        assert!(family_manifest_fingerprint(&[]).is_err());
    }

    #[test]
    fn checkpoint_fingerprint_changes_with_weights_and_metadata() {
        let original = RankerModel {
            format: MODEL_FORMAT.into(),
            weights: vec![0.0; FEATURE_COUNT],
            training_examples: 2,
            training_repositories: 1,
            training_dataset_sha256: None,
        };
        let hash = checkpoint_fingerprint(&original).expect("valid checkpoint");
        let mut changed = original.clone();
        changed.weights[0] = 0.1;
        assert_ne!(
            hash,
            checkpoint_fingerprint(&changed).expect("different checkpoint")
        );
        let mut changed = original.clone();
        changed.training_examples = 4;
        assert_ne!(
            hash,
            checkpoint_fingerprint(&changed).expect("different metadata")
        );
    }
}
