//! Repository structure: files, directories, symbols, and entrypoints.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{SectionStatus, is_false};
use crate::confidence::Confidence;
use crate::evidence::Evidence;

/// The primary role of a file in the repository.
///
/// When several categories apply, the most specific one wins; see the discovery crate for
/// the precedence rules. Independent flags on [`FileRecord`] (`generated`, `vendored`,
/// `binary`) record orthogonal properties.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum FileCategory {
    /// First-party source code.
    Source,
    /// Test code and test data.
    Test,
    /// Documentation and prose.
    Documentation,
    /// Configuration files.
    Configuration,
    /// Continuous integration and delivery definitions.
    CiCd,
    /// Build scripts and build-system metadata.
    Build,
    /// Dependency manifests such as `Cargo.toml` or `package.json`.
    Manifest,
    /// Dependency lockfiles such as `Cargo.lock`.
    Lockfile,
    /// Machine-generated code.
    Generated,
    /// Images, fonts, audio, and other assets.
    Asset,
    /// Binary files that are not recognized assets.
    Binary,
    /// Third-party code vendored into the repository.
    Vendor,
    /// Build output or caches that were committed.
    BuildOutput,
    /// Editor and IDE metadata.
    IdeMetadata,
    /// Container definitions such as Dockerfiles.
    Container,
    /// Infrastructure-as-code definitions.
    Infrastructure,
    /// Data files such as CSV or datasets.
    Data,
    /// License texts.
    License,
    /// Anything that does not match another category.
    Other,
}

impl FileCategory {
    /// Every category, in display order.
    pub const ALL: [FileCategory; 19] = [
        FileCategory::Source,
        FileCategory::Test,
        FileCategory::Documentation,
        FileCategory::Configuration,
        FileCategory::CiCd,
        FileCategory::Build,
        FileCategory::Manifest,
        FileCategory::Lockfile,
        FileCategory::Generated,
        FileCategory::Asset,
        FileCategory::Binary,
        FileCategory::Vendor,
        FileCategory::BuildOutput,
        FileCategory::IdeMetadata,
        FileCategory::Container,
        FileCategory::Infrastructure,
        FileCategory::Data,
        FileCategory::License,
        FileCategory::Other,
    ];

    /// Human-readable label.
    pub const fn label(self) -> &'static str {
        match self {
            FileCategory::Source => "Source code",
            FileCategory::Test => "Tests",
            FileCategory::Documentation => "Documentation",
            FileCategory::Configuration => "Configuration",
            FileCategory::CiCd => "CI/CD",
            FileCategory::Build => "Build metadata",
            FileCategory::Manifest => "Dependency manifests",
            FileCategory::Lockfile => "Lockfiles",
            FileCategory::Generated => "Generated code",
            FileCategory::Asset => "Assets",
            FileCategory::Binary => "Binary files",
            FileCategory::Vendor => "Vendored code",
            FileCategory::BuildOutput => "Build output / cache",
            FileCategory::IdeMetadata => "IDE metadata",
            FileCategory::Container => "Container files",
            FileCategory::Infrastructure => "Infrastructure-as-code",
            FileCategory::Data => "Data files",
            FileCategory::License => "License",
            FileCategory::Other => "Other",
        }
    }

    /// Returns `true` for categories whose code counts as first-party code for quality metrics.
    pub const fn is_first_party_code(self) -> bool {
        matches!(self, FileCategory::Source | FileCategory::Test)
    }
}

impl fmt::Display for FileCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Line counts for a text file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LineCounts {
    /// All lines.
    pub total: u64,
    /// Lines containing code (a line with code and a trailing comment counts as code).
    pub code: u64,
    /// Lines containing only comments.
    pub comment: u64,
    /// Lines containing only whitespace.
    pub blank: u64,
}

impl LineCounts {
    /// Adds another set of counts to this one.
    pub fn add(&mut self, other: &LineCounts) {
        self.total += other.total;
        self.code += other.code;
        self.comment += other.comment;
        self.blank += other.blank;
    }
}

/// Per-file results of lexical analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileAnalysisSummary {
    /// Import, include, or require statements detected.
    pub imports: u32,
    /// Symbols (functions, types, …) detected.
    pub symbols: u32,
    /// Functions and methods detected.
    pub functions: u32,
    /// Sum of the approximate cyclomatic complexity of all functions.
    pub cyclomatic_total: u32,
    /// Highest approximate cyclomatic complexity of any function.
    pub cyclomatic_max: u32,
    /// Deepest block nesting observed.
    pub max_nesting: u32,
    /// Length in lines of the longest function.
    pub longest_function: u32,
    /// TODO/FIXME-style markers found in comments.
    pub markers: u32,
}

/// Why a file was not analyzed beyond classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SkipReason {
    /// Larger than the configured maximum file size.
    TooLarge,
    /// Could not be read (permissions or I/O error).
    Unreadable,
}

/// A file discovered in the repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileRecord {
    /// Repository-relative path using forward slashes.
    pub path: String,
    /// Primary classification.
    pub category: FileCategory,
    /// Detected language identifier, e.g. `rust`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Size in bytes.
    pub bytes: u64,
    /// Line counts, absent for binary or skipped files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<LineCounts>,
    /// The file contains binary data.
    #[serde(default, skip_serializing_if = "is_false")]
    pub binary: bool,
    /// The file appears to be machine-generated.
    #[serde(default, skip_serializing_if = "is_false")]
    pub generated: bool,
    /// The file is third-party code vendored into the repository.
    #[serde(default, skip_serializing_if = "is_false")]
    pub vendored: bool,
    /// Short content hash (first 64 bits of SHA-256, hexadecimal).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// Identifier of the module this file belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    /// Lexical analysis summary, absent when the language has no lexical analyzer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<FileAnalysisSummary>,
    /// Set when content analysis was skipped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<SkipReason>,
}

impl FileRecord {
    /// Creates a record with only the path, category, and size set.
    pub fn new(path: impl Into<String>, category: FileCategory, bytes: u64) -> Self {
        Self {
            path: path.into(),
            category,
            language: None,
            bytes,
            lines: None,
            binary: false,
            generated: false,
            vendored: false,
            hash: None,
            module: None,
            analysis: None,
            skipped: None,
        }
    }

    /// Returns `true` if the file is first-party code: source or tests that are neither
    /// generated nor vendored.
    pub fn is_first_party_code(&self) -> bool {
        self.category.is_first_party_code() && !self.generated && !self.vendored && !self.binary
    }

    /// Number of code lines, or zero when lines were not counted.
    pub fn code_lines(&self) -> u64 {
        self.lines.map_or(0, |lines| lines.code)
    }
}

/// Aggregated statistics for a directory, including all nested files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryRecord {
    /// Repository-relative path of the directory (empty for the root).
    pub path: String,
    /// Number of path components (0 for the root).
    pub depth: u32,
    /// Files contained, recursively.
    pub files: u64,
    /// Bytes contained, recursively.
    pub bytes: u64,
    /// Code lines contained, recursively.
    pub code_lines: u64,
    /// Language with the most code lines in the directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_language: Option<String>,
}

/// The kind of a detected symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SymbolKind {
    /// A free function.
    Function,
    /// A function attached to a type.
    Method,
    /// A class.
    Class,
    /// A struct or record type.
    Struct,
    /// An enumeration.
    Enum,
    /// An interface or protocol.
    Interface,
    /// A trait.
    Trait,
    /// A module or namespace block.
    Module,
    /// A type alias or other named type.
    Type,
    /// A named constant.
    Constant,
    /// A macro.
    Macro,
    /// A database table, view, or other schema object.
    Schema,
}

/// A symbol detected by lexical analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SymbolRecord {
    /// Symbol name.
    pub name: String,
    /// Symbol kind.
    pub kind: SymbolKind,
    /// Repository-relative path of the defining file.
    pub path: String,
    /// Line of the definition (1-based).
    pub line: u32,
    /// Last line of the definition, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    /// Approximate cyclomatic complexity, for functions and methods.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complexity: Option<u32>,
}

/// The kind of an entrypoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EntrypointKind {
    /// The main function of an executable program.
    Binary,
    /// The root of a library or package.
    Library,
    /// A web application or page entry.
    Web,
    /// A command-line tool declared in a manifest.
    Cli,
    /// A server or service entry.
    Server,
    /// A script executed directly.
    Script,
}

/// A place where execution or consumption of the code starts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Entrypoint {
    /// Repository-relative path.
    pub path: String,
    /// Entrypoint kind.
    pub kind: EntrypointKind,
    /// Language of the entrypoint file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Why the file was identified as an entrypoint.
    pub reason: String,
    /// Confidence of the identification.
    pub confidence: Confidence,
    /// Supporting evidence.
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

/// File counts for one category.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CategoryCount {
    /// The category.
    pub category: FileCategory,
    /// Number of files.
    pub files: u64,
    /// Total bytes.
    pub bytes: u64,
    /// Total lines (text files only).
    pub lines: u64,
}

/// Repository size class by number of files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SizeClass {
    /// Fewer than 100 files.
    #[default]
    Tiny,
    /// 100 to 999 files.
    Small,
    /// 1,000 to 9,999 files.
    Medium,
    /// 10,000 to 99,999 files.
    Large,
    /// 100,000 files or more.
    VeryLarge,
}

impl SizeClass {
    /// Classifies a repository by its file count.
    pub const fn from_file_count(files: u64) -> Self {
        match files {
            0..=99 => SizeClass::Tiny,
            100..=999 => SizeClass::Small,
            1_000..=9_999 => SizeClass::Medium,
            10_000..=99_999 => SizeClass::Large,
            _ => SizeClass::VeryLarge,
        }
    }

    /// Human-readable label.
    pub const fn label(self) -> &'static str {
        match self {
            SizeClass::Tiny => "Tiny",
            SizeClass::Small => "Small",
            SizeClass::Medium => "Medium",
            SizeClass::Large => "Large",
            SizeClass::VeryLarge => "Very large",
        }
    }
}

/// Repository layout, file classification, and size.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StructureReport {
    /// Whether this section was analyzed.
    pub status: SectionStatus,
    /// Notes about limitations or partial results.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Number of files discovered (after ignore rules).
    pub total_files: u64,
    /// Total size of discovered files in bytes.
    pub total_bytes: u64,
    /// Total lines across text files.
    pub total_lines: u64,
    /// Code lines across text files.
    pub code_lines: u64,
    /// Comment-only lines across text files.
    pub comment_lines: u64,
    /// Blank lines across text files.
    pub blank_lines: u64,
    /// Size class by file count.
    pub size_class: SizeClass,
    /// File counts per category.
    #[serde(default)]
    pub categories: Vec<CategoryCount>,
    /// Every discovered file, sorted by path.
    #[serde(default)]
    pub files: Vec<FileRecord>,
    /// Aggregated directory statistics, sorted by path.
    #[serde(default)]
    pub directories: Vec<DirectoryRecord>,
    /// Detected symbols (capped by configuration; see `truncated`).
    #[serde(default)]
    pub symbols: Vec<SymbolRecord>,
    /// Detected entrypoints.
    #[serde(default)]
    pub entrypoints: Vec<Entrypoint>,
    /// Number of files flagged as generated.
    pub generated_files: u64,
    /// Number of files flagged as vendored.
    pub vendored_files: u64,
    /// Number of binary files.
    pub binary_files: u64,
    /// Number of symbolic links encountered (never followed).
    pub symlinks: u64,
    /// Number of files whose content analysis was skipped.
    pub skipped_files: u64,
    /// Ignore patterns applied in addition to `.gitignore` rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore_patterns: Vec<String>,
    /// `true` when lists in this section were capped to keep the artifact manageable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_classes_follow_documented_boundaries() {
        assert_eq!(SizeClass::from_file_count(0), SizeClass::Tiny);
        assert_eq!(SizeClass::from_file_count(99), SizeClass::Tiny);
        assert_eq!(SizeClass::from_file_count(100), SizeClass::Small);
        assert_eq!(SizeClass::from_file_count(9_999), SizeClass::Medium);
        assert_eq!(SizeClass::from_file_count(10_000), SizeClass::Large);
        assert_eq!(SizeClass::from_file_count(250_000), SizeClass::VeryLarge);
    }

    #[test]
    fn file_records_serialize_compactly() {
        let mut record = FileRecord::new("src/main.rs", FileCategory::Source, 120);
        record.language = Some("rust".into());
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["category"], "source");
        assert!(json.get("binary").is_none(), "false flags are omitted");
        assert!(json.get("lines").is_none());
        let back: FileRecord = serde_json::from_value(json).unwrap();
        assert_eq!(back, record);
        assert!(record.is_first_party_code());
    }

    #[test]
    fn generated_and_vendored_code_is_not_first_party() {
        let mut record = FileRecord::new("gen/api.rs", FileCategory::Source, 10);
        record.generated = true;
        assert!(!record.is_first_party_code());
        assert_eq!(FileCategory::CiCd.label(), "CI/CD");
        assert_eq!(
            serde_json::to_string(&FileCategory::CiCd).unwrap(),
            "\"ci-cd\""
        );
    }

    #[test]
    fn line_counts_accumulate() {
        let mut total = LineCounts::default();
        total.add(&LineCounts {
            total: 10,
            code: 6,
            comment: 2,
            blank: 2,
        });
        total.add(&LineCounts {
            total: 5,
            code: 5,
            comment: 0,
            blank: 0,
        });
        assert_eq!(total.code, 11);
        assert_eq!(total.total, 15);
    }
}
