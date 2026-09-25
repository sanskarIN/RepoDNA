//! The file pass: discovery, reading, lexical analysis, tokenization, and security scanning.
//!
//! Files are processed in path-ordered chunks on a thread pool. Results are collected in
//! path order, so the artifact is identical regardless of thread scheduling, and the token
//! budget for duplication detection is applied deterministically between chunks. File text
//! is dropped as soon as a file is processed, except for the small set of metadata files
//! that later stages read (manifests, CI configuration, README, license).
//!
//! With an [`AnalysisCache`], lexical analysis results are reused for files whose content,
//! language definition, and RepoDNA version are unchanged.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::Path;

use rayon::prelude::*;
use repodna_core::cancel::CancellationToken;
use repodna_core::config::{Config, DEFAULT_IGNORE_PATTERNS, Stage, StageSet};
use repodna_core::glob::GlobSet;
use repodna_core::hash::StableHasher;
use repodna_core::model::languages::{LanguageKind, ParserCapability};
use repodna_core::model::security::PatternCandidate;
use repodna_core::model::structure::{FileCategory, FileRecord, LineCounts, SkipReason};
use repodna_dependencies::is_dependency_file;
use repodna_discovery::{
    ClassificationOverrides, DiscoveredFile, DiscoveryOptions, ReadOutcome, classify, discover,
    read_file,
};
use repodna_parser::{
    ANALYZER_VERSION, FileAnalysis, LanguageRegistry, LanguageSpec, analyze_and_tokenize,
    count_lines, tokenize,
};
use repodna_project::wants_content;
use repodna_quality::TokenStream;
use repodna_security::{PendingSecret, find_secrets, scan_patterns};

use crate::{EngineError, Progress, ProgressEvent};

/// Tokens kept across all files for duplication and similarity detection.
pub const MAX_TOKENS: usize = 12_000_000;

/// Files processed per parallel chunk.
const CHUNK: usize = 512;

/// A content-addressed cache of per-file lexical analysis results.
///
/// Keys (see [`cache_key`]) cover the file content, the language definition, and the
/// RepoDNA and analyzer versions, so a cached result is only reused for identical input to
/// identical code. Implementations must tolerate concurrent `get` calls from worker threads.
pub trait AnalysisCache: Send + Sync + fmt::Debug {
    /// Looks up a cached analysis.
    fn get(&self, key: &str) -> Option<FileAnalysis>;

    /// Stores analyses computed during the file pass.
    fn put(&self, entries: &[(String, &FileAnalysis)]);

    /// Called once when the file pass is complete, for bookkeeping such as eviction.
    fn finish(&self) {}
}

/// A fingerprint of a language definition, part of every cache key for that language.
pub fn language_fingerprint(spec: &LanguageSpec) -> String {
    let mut hasher = StableHasher::new();
    hasher.str_field(&format!("{spec:?}"));
    hasher.finish_hex()
}

/// The cache key for the analysis of a file with the given content hash.
pub fn cache_key(content_sha256: &[u8; 32], language_fingerprint: &str) -> String {
    let mut hasher = StableHasher::new();
    hasher
        .str_field("repodna-file-analysis")
        .str_field(env!("CARGO_PKG_VERSION"))
        .str_field(&ANALYZER_VERSION.to_string())
        .str_field(language_fingerprint)
        .field(content_sha256);
    hasher.finish_hex()
}

/// A processed file.
#[derive(Debug, Clone)]
pub struct ScannedFile {
    /// The record stored in the artifact.
    pub record: FileRecord,
    /// Kind of the detected language.
    pub language_kind: Option<LanguageKind>,
    /// Lexical analysis, for languages with a lexical analyzer.
    pub analysis: Option<FileAnalysis>,
    /// Normalized tokens, for non-test first-party code when duplication or similarity runs.
    pub tokens: Option<TokenStream>,
    /// SHA-256 of the content, when the file was read.
    pub sha256: Option<[u8; 32]>,
    /// Unix permission bits.
    pub mode: Option<u32>,
    /// The file contains inline tests.
    pub inline_tests: bool,
}

impl ScannedFile {
    /// `true` for first-party source or test code.
    pub fn first_party_code(&self) -> bool {
        self.record.is_first_party_code()
    }
}

/// Everything the file pass produced.
#[derive(Debug, Default)]
pub struct ScanResult {
    /// Files in path order.
    pub files: Vec<ScannedFile>,
    /// Contents of metadata files, keyed by path.
    pub contents: BTreeMap<String, String>,
    /// Secret candidates awaiting their fingerprints.
    pub secrets: Vec<PendingSecret>,
    /// Risky pattern candidates.
    pub patterns: Vec<PatternCandidate>,
    /// Text files scanned for security signals.
    pub security_files_scanned: u64,
    /// Symbolic links encountered (never followed).
    pub symlinks: u64,
    /// Discovery stopped at its file limit.
    pub truncated: bool,
    /// Problems during discovery.
    pub errors: Vec<String>,
    /// Ignore patterns applied.
    pub ignore_patterns: Vec<String>,
    /// Some files were not tokenized because the token budget was spent.
    pub tokens_truncated: bool,
    /// Analyses reused from the cache.
    pub cache_hits: u64,
    /// Analyses computed and offered to the cache.
    pub cache_misses: u64,
}

/// What the file pass needs to know.
#[derive(Debug, Clone, Copy)]
pub struct ScanOptions<'a> {
    /// Effective configuration.
    pub config: &'a Config,
    /// Enabled stages.
    pub stages: StageSet,
    /// Language registry.
    pub registry: &'a LanguageRegistry,
    /// Progress sink.
    pub progress: &'a Progress,
    /// Cache of per-file analysis results, when caching is enabled.
    pub cache: Option<&'a dyn AnalysisCache>,
}

/// Per-pass state shared by the worker threads.
struct PassContext<'a> {
    options: &'a ScanOptions<'a>,
    overrides: &'a ClassificationOverrides,
    /// Language fingerprints by identifier, computed once per pass when caching.
    fingerprints: HashMap<&'a str, String>,
}

/// Builds classification overrides from configuration patterns.
pub fn classification_overrides(config: &Config) -> Result<ClassificationOverrides, EngineError> {
    let set = |patterns: &[String]| {
        GlobSet::new(patterns)
            .map_err(|error| EngineError::Config(format!("classification pattern: {error}")))
    };
    let c = &config.classification;
    Ok(ClassificationOverrides {
        generated: set(&c.generated)?,
        vendor: set(&c.vendor)?,
        tests: set(&c.tests)?,
        docs: set(&c.docs)?,
        source: set(&c.source)?,
    })
}

/// Counts lines of a file without a known language: blank lines and everything else.
fn plain_line_counts(text: &str) -> LineCounts {
    let mut counts = LineCounts::default();
    for line in text.lines() {
        counts.total += 1;
        if line.trim().is_empty() {
            counts.blank += 1;
        } else {
            counts.code += 1;
        }
    }
    counts
}

fn has_inline_tests(language: Option<&str>, text: &str) -> bool {
    match language {
        Some("rust") => text.contains("#[cfg(test)]") || text.contains("#[test]"),
        Some("javascript" | "typescript") => text.contains("import.meta.vitest"),
        _ => false,
    }
}

/// Output of processing one file.
struct Processed {
    file: ScannedFile,
    content: Option<String>,
    secrets: Vec<PendingSecret>,
    patterns: Vec<PatternCandidate>,
    security_scanned: bool,
    /// Key under which a freshly computed analysis should be cached.
    cache_key: Option<String>,
    cache_hit: bool,
}

fn process(discovered: &DiscoveredFile, context: &PassContext<'_>) -> Processed {
    let options = context.options;
    let overrides = context.overrides;
    let config = options.config;
    let stages = options.stages;
    let registry = options.registry;
    let mut classification = discovered.classification;
    let mut language = discovered.language.clone();
    let mut language_kind = discovered.language_kind;
    let mut record = FileRecord::new(
        discovered.path.clone(),
        classification.category,
        discovered.bytes,
    );
    let mut processed = Processed {
        file: ScannedFile {
            record: FileRecord::new(String::new(), FileCategory::Other, 0),
            language_kind,
            analysis: None,
            tokens: None,
            sha256: None,
            mode: discovered.mode,
            inline_tests: false,
        },
        content: None,
        secrets: Vec::new(),
        patterns: Vec::new(),
        security_scanned: false,
        cache_key: None,
        cache_hit: false,
    };

    // Images, fonts, and other binary formats are recognized by extension and not read.
    if matches!(
        classification.category,
        FileCategory::Asset | FileCategory::Binary
    ) {
        record.binary = true;
    } else {
        match read_file(&discovered.absolute, config.analysis.max_file_bytes) {
            ReadOutcome::TooLarge { .. } => record.skipped = Some(SkipReason::TooLarge),
            ReadOutcome::Unreadable(_) => record.skipped = Some(SkipReason::Unreadable),
            ReadOutcome::Content(content) => {
                record.hash = Some(content.short_hash());
                record.binary = content.binary;
                processed.file.sha256 = Some(content.sha256);
                if let Some(text) = content.text.as_deref() {
                    if language.is_none()
                        && let Some(first) = text.lines().next()
                        && let Some(spec) = registry.detect_shebang(first)
                    {
                        language = Some(spec.id.clone());
                        language_kind = Some(spec.kind);
                        classification = classify(&discovered.path, language_kind, overrides);
                        record.category = classification.category;
                    }
                    let spec = language.as_deref().and_then(|id| registry.get(id));
                    let lexical =
                        spec.is_some_and(|spec| spec.capability() == ParserCapability::Lexical);
                    let code = matches!(
                        classification.category,
                        FileCategory::Source | FileCategory::Test
                    ) && !classification.vendored
                        && !classification.generated;
                    match spec {
                        Some(spec) if lexical && stages.contains(Stage::Parsing) => {
                            let wants_tokens = code
                                && classification.category == FileCategory::Source
                                && (stages.contains(Stage::Duplication)
                                    || stages.contains(Stage::Similarity));
                            let key = options.cache.and_then(|_| {
                                context
                                    .fingerprints
                                    .get(spec.id.as_str())
                                    .map(|fingerprint| cache_key(&content.sha256, fingerprint))
                            });
                            let cached = options
                                .cache
                                .zip(key.as_deref())
                                .and_then(|(cache, key)| cache.get(key));
                            let (analysis, tokens) = match cached {
                                Some(analysis) => {
                                    processed.cache_hit = true;
                                    let tokens = wants_tokens.then(|| {
                                        tokenize(&repodna_parser::scan(text, &spec.syntax).lines)
                                    });
                                    (analysis, tokens)
                                }
                                None => {
                                    processed.cache_key = key;
                                    let (analysis, tokens) = analyze_and_tokenize(spec, text);
                                    (analysis, Some(tokens))
                                }
                            };
                            record.lines = Some(analysis.lines);
                            record.analysis = Some(analysis.summary());
                            if wants_tokens && let Some(tokens) = &tokens {
                                processed.file.tokens =
                                    Some(TokenStream::for_file(tokens, Some(&analysis)));
                            }
                            processed.file.analysis = Some(analysis);
                        }
                        Some(spec) => record.lines = Some(count_lines(spec, text)),
                        None => record.lines = Some(plain_line_counts(text)),
                    }
                    processed.file.inline_tests =
                        code && has_inline_tests(language.as_deref(), text);
                    if stages.contains(Stage::Security)
                        && classification.category != FileCategory::Lockfile
                    {
                        processed.security_scanned = true;
                        processed.secrets = find_secrets(&discovered.path, text, spec);
                        if !classification.vendored {
                            processed.patterns = scan_patterns(&discovered.path, text, spec);
                        }
                    }
                    if wants_content(&discovered.path) || is_dependency_file(&discovered.path) {
                        processed.content = Some(text.to_owned());
                    }
                }
            }
        }
    }
    record.language = language;
    record.generated = classification.generated;
    record.vendored = classification.vendored;
    processed.file.language_kind = language_kind;
    processed.file.record = record;
    processed
}

/// Runs discovery and the file pass.
pub fn scan_files(
    root: &Path,
    options: &ScanOptions<'_>,
    cancel: &CancellationToken,
) -> Result<ScanResult, EngineError> {
    let config = options.config;
    let overrides = classification_overrides(config)?;
    let mut ignore_patterns: Vec<String> = Vec::new();
    if config.ignore.use_default_patterns {
        ignore_patterns.extend(DEFAULT_IGNORE_PATTERNS.iter().map(|p| (*p).to_owned()));
    }
    ignore_patterns.extend(config.ignore.patterns.iter().cloned());
    let threads = config.performance.parallelism.threads();
    let discovery = discover(
        root,
        &DiscoveryOptions {
            respect_gitignore: config.analysis.respect_gitignore,
            ignore_patterns,
            overrides: overrides.clone(),
            threads,
            ..DiscoveryOptions::default()
        },
        options.registry,
        cancel,
    )
    .map_err(|error| match error {
        repodna_discovery::DiscoveryError::Cancelled => EngineError::Cancelled,
        other => EngineError::Discovery(other),
    })?;

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .ok();
    let mut result = ScanResult {
        symlinks: discovery.symlinks,
        truncated: discovery.truncated,
        errors: discovery.errors,
        ignore_patterns: discovery.ignore_patterns,
        ..ScanResult::default()
    };
    let total = discovery.files.len();
    let fingerprints: HashMap<&str, String> = if options.cache.is_some() {
        options
            .registry
            .languages()
            .iter()
            .map(|spec| (spec.id.as_str(), language_fingerprint(spec)))
            .collect()
    } else {
        HashMap::new()
    };
    let context = PassContext {
        options,
        overrides: &overrides,
        fingerprints,
    };
    let mut tokens_used = 0usize;
    for chunk in discovery.files.chunks(CHUNK) {
        cancel.check()?;
        let work = || -> Vec<Processed> {
            chunk
                .par_iter()
                .map(|file| process(file, &context))
                .collect()
        };
        let processed = match &pool {
            Some(pool) => pool.install(work),
            None => work(),
        };
        let mut misses: Vec<(usize, String)> = Vec::new();
        for mut item in processed {
            if item.cache_hit {
                result.cache_hits += 1;
            }
            if let Some(key) = item.cache_key.take() {
                misses.push((result.files.len(), key));
            }
            if let Some(tokens) = &item.file.tokens {
                if tokens_used + tokens.len() > MAX_TOKENS {
                    item.file.tokens = None;
                    result.tokens_truncated = true;
                } else {
                    tokens_used += tokens.len();
                }
            }
            if let Some(content) = item.content {
                result
                    .contents
                    .insert(item.file.record.path.clone(), content);
            }
            result.secrets.extend(item.secrets);
            result.patterns.extend(item.patterns);
            result.security_files_scanned += u64::from(item.security_scanned);
            result.files.push(item.file);
        }
        if let Some(cache) = options.cache {
            let entries: Vec<(String, &FileAnalysis)> = misses
                .into_iter()
                .filter_map(|(index, key)| {
                    result.files[index]
                        .analysis
                        .as_ref()
                        .map(|analysis| (key, analysis))
                })
                .collect();
            result.cache_misses += entries.len() as u64;
            if !entries.is_empty() {
                cache.put(&entries);
            }
        }
        options.progress.emit(ProgressEvent::Files {
            done: result.files.len(),
            total,
        });
    }
    if let Some(cache) = options.cache {
        cache.finish();
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_testkit::write_tree;

    fn scan(files: &[(&str, &str)], stages: StageSet) -> ScanResult {
        let dir = tempfile::tempdir().unwrap();
        write_tree(dir.path(), files);
        let config = Config::default();
        let options = ScanOptions {
            config: &config,
            stages,
            registry: LanguageRegistry::builtin(),
            progress: &Progress::default(),
            cache: None,
        };
        scan_files(dir.path(), &options, &CancellationToken::new()).unwrap()
    }

    fn all_stages() -> StageSet {
        StageSet::of(&Stage::ALL)
    }

    #[test]
    fn processes_files_in_path_order() {
        let key = [
            "-----BEGIN ",
            "PRIVATE KEY-----\nMIIE\n-----END ",
            "PRIVATE KEY-----\n",
        ]
        .concat();
        let result = scan(
            &[
                (
                    "src/lib.rs",
                    "//! Docs\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\n#[cfg(test)]\nmod tests {}\n",
                ),
                (
                    "src/app.py",
                    "import os\n\ndef main():\n    return os.sep\n",
                ),
                ("bin/tool", "#!/usr/bin/env python3\nprint('hi')\n"),
                ("Cargo.toml", "[package]\nname = \"demo\"\n"),
                ("logo.png", "\u{0}PNG"),
                ("notes.txt", "one\n\ntwo\n"),
                ("deploy/server.key", &key),
                ("app/settings.py", "DEBUG = True\n"),
            ],
            all_stages(),
        );
        let paths: Vec<&str> = result
            .files
            .iter()
            .map(|f| f.record.path.as_str())
            .collect();
        let mut sorted = paths.clone();
        sorted.sort_unstable();
        assert_eq!(paths, sorted);
        let find = |path: &str| result.files.iter().find(|f| f.record.path == path).unwrap();

        let lib = find("src/lib.rs");
        assert_eq!(lib.record.language.as_deref(), Some("rust"));
        assert_eq!(lib.record.lines.unwrap().code, 5);
        assert!(lib.analysis.is_some() && lib.tokens.is_some() && lib.inline_tests);
        assert!(lib.record.hash.is_some());

        let tool = find("bin/tool");
        assert_eq!(tool.record.language.as_deref(), Some("python"));

        let logo = find("logo.png");
        assert!(logo.record.binary && logo.sha256.is_none());

        let notes = find("notes.txt");
        assert_eq!(notes.record.lines.unwrap().blank, 1);

        assert!(result.contents.contains_key("Cargo.toml"));
        assert!(!result.contents.contains_key("src/lib.rs"));
        assert_eq!(result.secrets.len(), 1);
        assert!(result.patterns.iter().any(|p| p.rule == "debug-enabled"));
        assert!(result.security_files_scanned >= 6);
    }

    #[test]
    fn respects_disabled_stages() {
        let result = scan(
            &[("src/lib.rs", "pub fn a() {}\n")],
            StageSet::of(&[Stage::Discovery]),
        );
        let file = &result.files[0];
        assert!(file.analysis.is_none() && file.tokens.is_none());
        assert_eq!(file.record.lines.unwrap().total, 1);
        assert_eq!(result.security_files_scanned, 0);
    }

    #[test]
    fn reuses_cached_analyses_for_unchanged_content() {
        use std::sync::Mutex;

        #[derive(Debug, Default)]
        struct MemoryCache(Mutex<HashMap<String, FileAnalysis>>);
        impl AnalysisCache for MemoryCache {
            fn get(&self, key: &str) -> Option<FileAnalysis> {
                self.0.lock().unwrap().get(key).cloned()
            }
            fn put(&self, entries: &[(String, &FileAnalysis)]) {
                let mut map = self.0.lock().unwrap();
                for (key, analysis) in entries {
                    map.insert(key.clone(), (*analysis).clone());
                }
            }
        }

        let dir = tempfile::tempdir().unwrap();
        write_tree(
            dir.path(),
            &[
                ("src/a.rs", "use crate::b;\npub fn a() { if true {} }\n"),
                ("src/b.rs", "pub fn b() {}\n"),
                ("README.md", "# Readme\n"),
            ],
        );
        let config = Config::default();
        let cache = MemoryCache::default();
        let options = ScanOptions {
            config: &config,
            stages: StageSet::of(&Stage::ALL),
            registry: LanguageRegistry::builtin(),
            progress: &Progress::default(),
            cache: Some(&cache),
        };
        let cancel = CancellationToken::new();
        let first = scan_files(dir.path(), &options, &cancel).unwrap();
        assert_eq!((first.cache_hits, first.cache_misses), (0, 2));
        assert_eq!(cache.0.lock().unwrap().len(), 2);
        let second = scan_files(dir.path(), &options, &cancel).unwrap();
        assert_eq!((second.cache_hits, second.cache_misses), (2, 0));
        for (a, b) in first.files.iter().zip(&second.files) {
            assert_eq!(a.analysis, b.analysis);
            assert_eq!(
                a.tokens.as_ref().map(|t| t.len()),
                b.tokens.as_ref().map(|t| t.len())
            );
        }

        std::fs::write(dir.path().join("src/b.rs"), "pub fn b() { loop {} }\n").unwrap();
        let third = scan_files(dir.path(), &options, &cancel).unwrap();
        assert_eq!((third.cache_hits, third.cache_misses), (1, 1));
    }
}
