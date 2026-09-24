//! Code-quality signals: complexity, size, duplication, markers, and dead-code candidates.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::SectionStatus;
use crate::confidence::Confidence;
use crate::evidence::Evidence;

/// A function-level measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FunctionSignal {
    /// Repository-relative path.
    pub path: String,
    /// Function name.
    pub name: String,
    /// First line (1-based).
    pub line: u32,
    /// Last line (1-based, inclusive).
    pub end_line: u32,
    /// Length in lines.
    pub lines: u32,
    /// Approximate cyclomatic complexity.
    pub cyclomatic: u32,
    /// Deepest block nesting inside the function.
    pub nesting: u32,
    /// Language identifier.
    pub language: String,
}

/// A file-level measurement compared with a threshold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileSignal {
    /// Repository-relative path.
    pub path: String,
    /// Measured value.
    pub value: u64,
    /// Threshold the value exceeded.
    pub threshold: u64,
    /// Unit of `value`, e.g. `lines`.
    pub unit: String,
    /// Language identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// One bucket of a distribution histogram.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DistributionBucket {
    /// Label such as `1–5` or `51+`.
    pub label: String,
    /// Inclusive lower bound.
    pub min: u32,
    /// Inclusive upper bound; absent for the open-ended last bucket.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u32>,
    /// Number of items in the bucket.
    pub count: u64,
}

/// Complexity statistics for one language.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LanguageComplexity {
    /// Language identifier.
    pub language: String,
    /// Functions analyzed.
    pub functions: u64,
    /// Mean approximate cyclomatic complexity.
    pub average: f64,
    /// Highest approximate cyclomatic complexity.
    pub max: u32,
}

/// Complexity signals across the repository.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ComplexityReport {
    /// Functions analyzed.
    pub functions_analyzed: u64,
    /// Mean approximate cyclomatic complexity.
    pub average_cyclomatic: f64,
    /// Median approximate cyclomatic complexity.
    pub median_cyclomatic: f64,
    /// Distribution of function complexity.
    #[serde(default)]
    pub distribution: Vec<DistributionBucket>,
    /// Most complex functions, descending.
    #[serde(default)]
    pub top_functions: Vec<FunctionSignal>,
    /// Statistics per language.
    #[serde(default)]
    pub by_language: Vec<LanguageComplexity>,
    /// How complexity was approximated.
    pub method: String,
}

/// A code location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodeLocation {
    /// Repository-relative path.
    pub path: String,
    /// First line (1-based).
    pub start_line: u32,
    /// Last line (1-based, inclusive).
    pub end_line: u32,
}

/// A set of code blocks with identical normalized token sequences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCluster {
    /// Stable identifier.
    pub id: String,
    /// Normalized tokens in the duplicated block.
    pub tokens: u32,
    /// Lines in the (first) duplicated block.
    pub lines: u32,
    /// Language identifier.
    pub language: String,
    /// Every occurrence of the block.
    pub occurrences: Vec<CodeLocation>,
}

/// Duplication analysis.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DuplicationReport {
    /// Whether duplication detection ran.
    pub status: SectionStatus,
    /// Duplicate clusters, largest first.
    #[serde(default)]
    pub clusters: Vec<DuplicateCluster>,
    /// Lines covered by at least one duplicated block (excluding the first occurrence).
    pub duplicated_lines: u64,
    /// Code lines considered for duplication.
    pub analyzed_lines: u64,
    /// `duplicatedLines / analyzedLines`.
    pub ratio: f64,
    /// Minimum block size in normalized tokens.
    pub min_tokens: u32,
    /// How duplication was detected.
    pub method: String,
}

/// Kind of a code marker comment.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum MarkerKind {
    /// `TODO`
    Todo,
    /// `FIXME`
    Fixme,
    /// `HACK`
    Hack,
    /// `XXX`
    Xxx,
    /// `BUG`
    Bug,
    /// `DEPRECATED`
    Deprecated,
}

impl MarkerKind {
    /// The marker keyword as written in code.
    pub const fn keyword(self) -> &'static str {
        match self {
            MarkerKind::Todo => "TODO",
            MarkerKind::Fixme => "FIXME",
            MarkerKind::Hack => "HACK",
            MarkerKind::Xxx => "XXX",
            MarkerKind::Bug => "BUG",
            MarkerKind::Deprecated => "DEPRECATED",
        }
    }
}

/// Number of markers of one kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarkerCount {
    /// Marker kind.
    pub kind: MarkerKind,
    /// Occurrences.
    pub count: u64,
}

/// One marker comment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarkerItem {
    /// Repository-relative path.
    pub path: String,
    /// Line (1-based).
    pub line: u32,
    /// Marker kind.
    pub kind: MarkerKind,
    /// Comment text, truncated and with secret-like values redacted.
    pub text: String,
}

/// Marker comments such as TODO and FIXME.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarkerReport {
    /// Counts per kind.
    #[serde(default)]
    pub counts: Vec<MarkerCount>,
    /// Individual markers (capped).
    #[serde(default)]
    pub items: Vec<MarkerItem>,
    /// Total markers found.
    pub total: u64,
}

/// Kind of a dead-code candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DeadCodeKind {
    /// A source file that no other analyzed file references.
    UnreferencedFile,
    /// A module that no other module depends on and that is not an entrypoint.
    UnreferencedModule,
    /// A directory whose name suggests legacy code and that has not changed recently.
    LegacyDirectory,
}

/// A conservative dead-code candidate. Never a definitive claim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeadCodeCandidate {
    /// Repository-relative path.
    pub path: String,
    /// Candidate kind.
    pub kind: DeadCodeKind,
    /// Label shown to users, e.g. `No references detected`.
    pub label: String,
    /// Why the path was flagged.
    pub reason: String,
    /// Confidence.
    pub confidence: Confidence,
    /// Supporting evidence.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// A maintainability indicator with its interpretation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MaintainabilitySignal {
    /// Identifier.
    pub id: String,
    /// Label.
    pub label: String,
    /// Measured value.
    pub value: f64,
    /// Unit of `value`.
    pub unit: String,
    /// Neutral description of what the value means.
    pub description: String,
}

/// Code-quality signals.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QualityReport {
    /// Whether this section was analyzed.
    pub status: SectionStatus,
    /// Notes about limitations or partial results.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Complexity signals.
    #[serde(default)]
    pub complexity: ComplexityReport,
    /// Files above the large-file threshold.
    #[serde(default)]
    pub large_files: Vec<FileSignal>,
    /// Functions above the large-function threshold.
    #[serde(default)]
    pub large_functions: Vec<FunctionSignal>,
    /// Functions with deep nesting.
    #[serde(default)]
    pub deep_nesting: Vec<FunctionSignal>,
    /// Duplication analysis.
    #[serde(default)]
    pub duplication: DuplicationReport,
    /// Marker comments.
    #[serde(default)]
    pub markers: MarkerReport,
    /// Dead-code candidates.
    #[serde(default)]
    pub dead_code_candidates: Vec<DeadCodeCandidate>,
    /// Maintainability indicators.
    #[serde(default)]
    pub maintainability: Vec<MaintainabilitySignal>,
}

/// A pair of files with similar content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SimilarFilePair {
    /// First file.
    pub a: String,
    /// Second file.
    pub b: String,
    /// Estimated Jaccard similarity of token shingles (0–1).
    pub similarity: f64,
}

/// Similarity between files inside the repository.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SimilarityReport {
    /// Whether similarity analysis ran.
    pub status: SectionStatus,
    /// Notes about limitations or partial results.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// File pairs above the similarity threshold, most similar first.
    #[serde(default)]
    pub similar_files: Vec<SimilarFilePair>,
    /// Threshold used (0–1).
    pub threshold: f64,
    /// How similarity was estimated.
    pub method: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_keywords_round_trip() {
        assert_eq!(MarkerKind::Fixme.keyword(), "FIXME");
        assert_eq!(serde_json::to_string(&MarkerKind::Xxx).unwrap(), "\"xxx\"");
    }

    #[test]
    fn default_quality_report_is_skipped() {
        let report = QualityReport::default();
        assert_eq!(report.status, SectionStatus::Skipped);
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["duplication"]["status"], "skipped");
    }
}
