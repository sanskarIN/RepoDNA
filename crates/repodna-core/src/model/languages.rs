//! Language composition and analysis capability per language.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::SectionStatus;
use crate::confidence::Confidence;
use crate::evidence::Evidence;

/// Broad family of a language, used to decide which languages count toward composition shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LanguageKind {
    /// General-purpose programming and scripting languages.
    Programming,
    /// Markup languages such as HTML.
    Markup,
    /// Stylesheet languages such as CSS.
    Stylesheet,
    /// Data and configuration formats such as JSON, YAML, and TOML.
    Data,
    /// Prose formats such as Markdown.
    Prose,
    /// Build-system languages such as Makefiles and Dockerfiles.
    Build,
}

impl LanguageKind {
    /// Returns `true` when code in this language counts toward the language-composition share.
    ///
    /// Data, prose, and build files are listed with their counts but excluded from shares,
    /// so a repository with large JSON fixtures is not reported as "mostly JSON".
    pub const fn counts_toward_share(self) -> bool {
        matches!(
            self,
            LanguageKind::Programming | LanguageKind::Markup | LanguageKind::Stylesheet
        )
    }
}

/// How deeply RepoDNA can analyze files of a language.
///
/// RepoDNA never claims more than it does: the built-in analyzers are *lexical* (they
/// understand comments, strings, imports, and block structure through tokenization and
/// patterns), not full parsers. `Syntactic` and `Semantic` are reserved for parser plugins.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum ParserCapability {
    /// The language is recognized by file name or extension only.
    Detection,
    /// Lines are classified as code, comment, or blank.
    LineCount,
    /// Comments, strings, imports, symbols, and complexity are analyzed lexically.
    Lexical,
    /// A real parser builds a syntax tree (plugin-provided).
    Syntactic,
    /// Names and types are resolved (plugin-provided).
    Semantic,
}

impl ParserCapability {
    /// Human-readable label.
    pub const fn label(self) -> &'static str {
        match self {
            ParserCapability::Detection => "Detection only",
            ParserCapability::LineCount => "Line counting",
            ParserCapability::Lexical => "Lexical analysis",
            ParserCapability::Syntactic => "Syntactic analysis",
            ParserCapability::Semantic => "Semantic analysis",
        }
    }

    /// Explanation of what the capability covers.
    pub const fn description(self) -> &'static str {
        match self {
            ParserCapability::Detection => {
                "Files are identified and sized; their contents are not analyzed."
            }
            ParserCapability::LineCount => {
                "Lines are classified as code, comment, or blank using the language's comment syntax."
            }
            ParserCapability::Lexical => {
                "A tokenizer that understands comments and strings extracts imports, symbols, markers, and approximate complexity. Results are heuristic, not compiler-accurate."
            }
            ParserCapability::Syntactic => {
                "A parser plugin builds a syntax tree for exact structural metrics."
            }
            ParserCapability::Semantic => "A parser plugin resolves names and types across files.",
        }
    }
}

/// Statistics for one language.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LanguageStat {
    /// Stable identifier, e.g. `typescript`.
    pub id: String,
    /// Display name, e.g. `TypeScript`.
    pub name: String,
    /// Language family.
    pub kind: LanguageKind,
    /// Number of files.
    pub files: u64,
    /// Total bytes.
    pub bytes: u64,
    /// Code lines.
    pub code_lines: u64,
    /// Comment-only lines.
    pub comment_lines: u64,
    /// Blank lines.
    pub blank_lines: u64,
    /// Share of code lines among languages that count toward composition (0–1).
    pub share: f64,
    /// Depth of analysis available for this language.
    pub capability: ParserCapability,
}

/// A detected relationship between two languages in the same repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LanguageInteraction {
    /// Language that depends on or calls into the other.
    pub from: String,
    /// Language being referenced.
    pub to: String,
    /// Mechanism, e.g. `import`, `file-reference`, `ffi`, or `tauri-command`.
    pub mechanism: String,
    /// Plain-language description.
    pub description: String,
    /// Number of occurrences observed.
    pub count: u32,
    /// Confidence of the detection.
    pub confidence: Confidence,
    /// Sample evidence (capped).
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// Language composition of the repository.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LanguageReport {
    /// Whether this section was analyzed.
    pub status: SectionStatus,
    /// Notes about limitations or partial results.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Languages sorted by code lines, descending.
    #[serde(default)]
    pub languages: Vec<LanguageStat>,
    /// Identifiers of the primary languages (up to three, by share).
    #[serde(default)]
    pub primary: Vec<String>,
    /// Cross-language relationships.
    #[serde(default)]
    pub interactions: Vec<LanguageInteraction>,
    /// Text files whose language was not recognized.
    pub unrecognized_files: u64,
}

impl LanguageReport {
    /// Looks up a language by identifier.
    pub fn get(&self, id: &str) -> Option<&LanguageStat> {
        self.languages.iter().find(|language| language.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_order_reflects_depth() {
        assert!(ParserCapability::Detection < ParserCapability::LineCount);
        assert!(ParserCapability::Lexical < ParserCapability::Syntactic);
        assert_eq!(
            serde_json::to_string(&ParserCapability::LineCount).unwrap(),
            "\"line-count\""
        );
    }

    #[test]
    fn data_languages_do_not_count_toward_share() {
        assert!(LanguageKind::Programming.counts_toward_share());
        assert!(LanguageKind::Markup.counts_toward_share());
        assert!(!LanguageKind::Data.counts_toward_share());
        assert!(!LanguageKind::Prose.counts_toward_share());
    }
}
