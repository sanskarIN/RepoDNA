//! Human-reviewable labels for training the native evidence ranker.
//!
//! Annotation templates retain RepoDNA's selected evidence IDs, never source
//! excerpts. Labels are explicitly unknown until a reviewer assigns each one.

use serde::{Deserialize, Serialize};

use crate::context::{Context, EvidenceItem, EvidenceKind};
use crate::ranking::LabeledEvidence;

/// A review record; `None` means the reviewer has not made a judgment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnnotationExample {
    /// Repository-level ID shared by all examples from one project.
    pub repository: String,
    /// The explanation task or user question.
    pub query: String,
    /// Already-selected repository analysis evidence.
    pub evidence: EvidenceItem,
    /// `null` until reviewed: must be set to `true` or `false` for training.
    pub relevant: Option<bool>,
}

/// Create a label template from an existing selection, without making labels up.
///
/// This deliberately omits all source excerpts. Review the resulting local JSON
/// before sharing: repository metadata can still contain private information.
pub fn annotation_template(
    repository: &str,
    query: &str,
    context: &Context,
) -> Result<Vec<AnnotationExample>, String> {
    let repository = repository.trim();
    let query = query.trim();
    if repository.is_empty() || repository.len() > 200 {
        return Err("repository ID must be 1..=200 UTF-8 bytes".to_owned());
    }
    if query.is_empty() || query.len() > 4_000 {
        return Err("question/task must be 1..=4000 UTF-8 bytes".to_owned());
    }
    let examples: Vec<_> = context
        .items
        .iter()
        .filter(|item| item.kind != EvidenceKind::Excerpt)
        .map(|item| AnnotationExample {
            repository: repository.to_owned(),
            query: query.to_owned(),
            evidence: item.clone(),
            relevant: None,
        })
        .collect();
    if examples.is_empty() {
        return Err("no non-excerpt evidence items available for annotation".to_owned());
    }
    Ok(examples)
}

/// Convert a fully reviewed template to trainer input.
///
/// Explicitly refuses missing labels, avoiding accidental negative examples.
/// This conversion does not constitute a model evaluation or license check.
pub fn finish_annotations(
    annotations: Vec<AnnotationExample>,
) -> Result<Vec<LabeledEvidence>, String> {
    if annotations.is_empty() {
        return Err("annotation batch must be nonempty".to_owned());
    }
    annotations
        .into_iter()
        .enumerate()
        .map(|(index, sample)| {
            let relevant = sample.relevant.ok_or_else(|| {
                format!("annotation {} has no relevance label (expected true or false)", index + 1)
            })?;
            if sample.repository.trim().is_empty()
                || sample.query.trim().is_empty()
                || sample.evidence.id.trim().is_empty()
                || sample.evidence.detail.trim().is_empty()
            {
                return Err(format!("annotation {} has missing required evidence fields", index + 1));
            }
            if sample.evidence.kind == EvidenceKind::Excerpt {
                return Err(format!("annotation {} contains a source excerpt", index + 1));
            }
            Ok(LabeledEvidence {
                repository: sample.repository,
                query: sample.query,
                evidence: sample.evidence,
                relevant,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::EvidenceKind;

    fn context() -> Context {
        Context {
            items: vec![
                EvidenceItem {
                    id: "E1".into(),
                    kind: EvidenceKind::Package,
                    reference: "Cargo.toml".into(),
                    detail: "Dependency metadata".into(),
                },
                EvidenceItem {
                    id: "E2".into(),
                    kind: EvidenceKind::Excerpt,
                    reference: "src/private.rs".into(),
                    detail: "private source".into(),
                },
            ],
            omitted: 0,
            estimated_tokens: 42,
            excerpts: true,
        }
    }

    #[test]
    fn requires_real_labels_and_skips_excerpts() {
        let mut template = annotation_template("repo-a", "Which dependencies?", &context()).unwrap();
        assert_eq!(template.len(), 1);
        assert_eq!(template[0].evidence.id, "E1");
        assert!(template[0].relevant.is_none());
        let raw = serde_json::to_string_pretty(&template).unwrap();
        assert!(raw.contains("\"relevant\": null"));
        assert!(!raw.contains("private source"));
        assert!(finish_annotations(template.clone()).is_err());
        template[0].relevant = Some(false);
        let finished = finish_annotations(template).unwrap();
        assert_eq!(finished.len(), 1);
        assert!(!finished[0].relevant);
    }

    #[test]
    fn never_accepts_unreviewed_or_excerpt_training_records() {
        assert!(annotation_template("", "question", &context()).is_err());
        assert!(annotation_template("repo-a", "", &context()).is_err());
        let mut template = annotation_template("repo-a", "question", &context()).unwrap();
        template[0].relevant = Some(true);
        template[0].evidence.kind = EvidenceKind::Excerpt;
        assert!(finish_annotations(template).is_err());
        assert!(finish_annotations(vec![]).is_err());
    }
}
