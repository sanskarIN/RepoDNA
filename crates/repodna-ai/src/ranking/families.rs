//! Explicit, reviewer-supplied repository-family provenance for model evaluation.
//!
//! Distinct repository IDs may describe forks or copied projects. A curated
//! manifest records their *common family* and enforces disjoint families
//! across training, validation and final-test sets. Never infer families from
//! names automatically, and never treat the manifest as independently verified.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::{LabeledEvidence, normalized_repository_id, validate_repository_split};

/// Maximum number of distinct repositories in a provenance manifest.
pub const MAX_FAMILY_MANIFEST_ENTRIES: usize = 10_000;

/// One repository's manually researched provenance (not an attestation).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryProvenance {
    /// Repository ID matching labels, case- and whitespace-normalized.
    pub repository: String,
    /// Human-curated fork/clone family identifier.
    pub family: String,
    /// Exact 40- or 64-character hexadecimal Git commit ID.
    pub revision: String,
    /// Recorded SPDX expression or NOASSERTION; not checked against a license.
    pub license: String,
}

/// Aggregate family split counts; no repository IDs or paths are disclosed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilySplitAudit {
    /// Exact number of repository entries supplied.
    pub manifest_repositories: usize,
    /// Distinct families in the training set.
    pub training_families: usize,
    /// Distinct families in the validation set.
    pub validation_families: usize,
    /// Distinct families in the final-test set.
    pub final_test_families: usize,
}

/// Reject unknown or reused family assignments before training/evaluation.
///
/// A caller MUST prepare this manifest manually: it cannot independently
/// establish clone identity, reviewer expertise, license rights or provenance
/// truth. Fails closed for absent/unused entries and case-folding aliases.
pub fn validate_repository_families(
    training: &[LabeledEvidence],
    validation: &[LabeledEvidence],
    test: &[LabeledEvidence],
    manifest: &[RepositoryProvenance],
) -> Result<FamilySplitAudit, String> {
    validate_repository_split(training, validation)?;
    validate_repository_split(training, test)?;
    validate_repository_split(validation, test)?;
    if manifest.is_empty() || manifest.len() > MAX_FAMILY_MANIFEST_ENTRIES {
        return Err("family manifest must contain 1..=10000 repository entries".into());
    }

    let mut mapping = HashMap::with_capacity(manifest.len());
    for entry in manifest {
        if entry.repository.trim().is_empty()
            || entry.repository.len() > 200
            || entry.family.trim().is_empty()
            || entry.family.len() > 200
            || entry.license.trim().is_empty()
            || entry.license.len() > 128
            || !matches!(entry.revision.len(), 40 | 64)
            || !entry.revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(
                "invalid family manifest repository, family, revision or license field".into(),
            );
        }
        let key = normalized_repository_id(&entry.repository);
        let value = normalized_repository_id(&entry.family);
        if mapping.insert(key, value).is_some() {
            return Err("duplicate normalized repository ID in family manifest".into());
        }
    }

    let mut used_repositories = HashSet::new();
    let mut family_owner: HashMap<&str, usize> = HashMap::new();
    let mut family_counts = [HashSet::new(), HashSet::new(), HashSet::new()];
    for (split, samples) in [training, validation, test].into_iter().enumerate() {
        for sample in samples {
            let repo = normalized_repository_id(&sample.repository);
            let family = mapping
                .get(&repo)
                .ok_or("family manifest is missing a labeled repository")?;
            used_repositories.insert(repo);
            if family_owner
                .insert(family, split)
                .is_some_and(|previous| previous != split)
            {
                return Err(
                    "repository family leakage across training, validation and final test"
                        .into(),
                );
            }
            family_counts[split].insert(family);
        }
    }
    if used_repositories.len() != mapping.len() {
        return Err("family manifest includes repositories absent from all label splits".into());
    }

    Ok(FamilySplitAudit {
        manifest_repositories: mapping.len(),
        training_families: family_counts[0].len(),
        validation_families: family_counts[1].len(),
        final_test_families: family_counts[2].len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EvidenceItem, EvidenceKind};

    fn label(repo: &str) -> LabeledEvidence {
        LabeledEvidence {
            repository: repo.into(),
            query: "Which dependency?".into(),
            evidence: EvidenceItem {
                id: "E1".into(),
                kind: EvidenceKind::Package,
                reference: "Cargo.toml".into(),
                detail: "Dependency metadata".into(),
            },
            relevant: true,
        }
    }

    fn entry(repo: &str, family: &str) -> RepositoryProvenance {
        RepositoryProvenance {
            repository: repo.into(),
            family: family.into(),
            revision: "a".repeat(40),
            license: "NOASSERTION".into(),
        }
    }

    #[test]
    fn accepts_disjoint_curated_families_and_reports_only_counts() {
        let training = [label("source-a"), label("FORK-A")];
        let validation = [label("source-b")];
        let test = [label("source-c")];
        let manifest = [
            entry("source-a", "family-a"),
            entry("fork-a", "FAMILY-A "),
            entry("source-b", "family-b"),
            entry("source-c", "family-c"),
        ];
        let result =
            validate_repository_families(&training, &validation, &test, &manifest).unwrap();
        assert_eq!(result.manifest_repositories, 4);
        assert_eq!(result.training_families, 1);
        assert_eq!(result.validation_families, 1);
        assert_eq!(result.final_test_families, 1);
    }

    #[test]
    fn catches_fork_leakage_even_with_unrelated_repository_ids() {
        let training = [label("different-a")];
        let validation = [label("different-b")];
        let test = [label("different-c")];
        let manifest = [
            entry("different-a", "same-source-family"),
            entry("different-b", " SAME-SOURCE-FAMILY "),
            entry("different-c", "independent"),
        ];
        let error =
            validate_repository_families(&training, &validation, &test, &manifest).unwrap_err();
        assert!(error.contains("family leakage"));
    }

    #[test]
    fn fails_closed_for_missing_extra_and_duplicate_aliases() {
        let training = [label("first")];
        let validation = [label("second")];
        let test = [label("third")];
        let manifest = [entry("first", "a"), entry("second", "b")];
        assert!(
            validate_repository_families(&training, &validation, &test, &manifest)
                .unwrap_err()
                .contains("missing")
        );

        let extra = [
            entry("first", "a"),
            entry("second", "b"),
            entry("third", "c"),
            entry("unused", "d"),
        ];
        assert!(
            validate_repository_families(&training, &validation, &test, &extra)
                .unwrap_err()
                .contains("absent")
        );

        let aliases = [
            entry("first", "a"),
            entry(" FIRST ", "a"),
            entry("second", "b"),
            entry("third", "c"),
        ];
        assert!(
            validate_repository_families(&training, &validation, &test, &aliases)
                .unwrap_err()
                .contains("duplicate")
        );
    }

    #[test]
    fn rejects_untraceable_revisions_and_missing_license_records() {
        let training = [label("first")];
        let validation = [label("second")];
        let test = [label("third")];
        let mut manifest = [
            entry("first", "a"),
            entry("second", "b"),
            entry("third", "c"),
        ];
        manifest[0].revision = "HEAD".into();
        assert!(validate_repository_families(&training, &validation, &test, &manifest).is_err());
        manifest[0].revision = "a".repeat(40);
        manifest[0].license = " ".into();
        assert!(validate_repository_families(&training, &validation, &test, &manifest).is_err());
    }
}
