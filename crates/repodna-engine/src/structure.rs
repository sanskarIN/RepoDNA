//! Structure and language sections, built from the file pass.

use std::collections::BTreeMap;

use repodna_core::confidence::Confidence;
use repodna_core::config::Thresholds;
use repodna_core::evidence::Evidence;
use repodna_core::finding::{Finding, FindingCategory};
use repodna_core::metric::round4;
use repodna_core::model::SectionStatus;
use repodna_core::model::languages::{
    LanguageKind, LanguageReport, LanguageStat, ParserCapability,
};
use repodna_core::model::structure::{
    CategoryCount, DirectoryRecord, FileCategory, LineCounts, SizeClass, StructureReport,
    SymbolRecord,
};
use repodna_core::paths;
use repodna_core::severity::Severity;
use repodna_parser::LanguageRegistry;

use crate::scan::ScanResult;

/// Maximum directory records kept (shallowest first).
pub const MAX_DIRECTORIES: usize = 5_000;

/// Large binary files reported as findings (largest first).
const MAX_BINARY_FINDINGS: usize = 20;

/// A language counts as primary at this share of first-party code.
const PRIMARY_SHARE: f64 = 0.1;

/// Maximum primary languages.
const MAX_PRIMARY: usize = 3;

#[derive(Default)]
struct DirectoryAccumulator {
    files: u64,
    bytes: u64,
    code_lines: u64,
    languages: BTreeMap<String, u64>,
}

fn dominant(languages: &BTreeMap<String, u64>) -> Option<String> {
    languages
        .iter()
        .filter(|(_, lines)| **lines > 0)
        .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
        .map(|(language, _)| language.clone())
}

/// Builds the structure section. Symbols are capped at `max_symbols`.
pub fn structure_report(scan: &ScanResult, max_symbols: usize) -> StructureReport {
    let mut report = StructureReport {
        status: SectionStatus::Analyzed,
        ignore_patterns: scan.ignore_patterns.clone(),
        symlinks: scan.symlinks,
        ..StructureReport::default()
    };
    if scan.truncated {
        report.truncated = true;
        report
            .notes
            .push("Discovery stopped at its file limit; the file list is incomplete.".to_owned());
    }
    report.notes.extend(scan.errors.iter().take(20).cloned());

    let mut categories: BTreeMap<u8, CategoryCount> = BTreeMap::new();
    let mut directories: BTreeMap<String, DirectoryAccumulator> = BTreeMap::new();
    let mut lines = LineCounts::default();
    for file in &scan.files {
        let record = &file.record;
        report.total_files += 1;
        report.total_bytes += record.bytes;
        if let Some(counts) = &record.lines {
            lines.add(counts);
        }
        report.generated_files += u64::from(record.generated);
        report.vendored_files += u64::from(record.vendored);
        report.binary_files += u64::from(record.binary);
        report.skipped_files += u64::from(record.skipped.is_some());
        let rank = FileCategory::ALL
            .iter()
            .position(|category| *category == record.category)
            .unwrap_or(FileCategory::ALL.len()) as u8;
        let entry = categories.entry(rank).or_insert(CategoryCount {
            category: record.category,
            files: 0,
            bytes: 0,
            lines: 0,
        });
        entry.files += 1;
        entry.bytes += record.bytes;
        entry.lines += record.lines.map_or(0, |l| l.total);

        let first_party = !record.vendored && !record.generated;
        let code = record.code_lines();
        let mut dirs = paths::ancestors(&record.path);
        dirs.insert(0, "");
        for dir in dirs {
            let entry = directories.entry(dir.to_owned()).or_default();
            entry.files += 1;
            entry.bytes += record.bytes;
            entry.code_lines += code;
            if first_party && let Some(language) = &record.language {
                *entry.languages.entry(language.clone()).or_default() += code;
            }
        }
    }
    report.total_lines = lines.total;
    report.code_lines = lines.code;
    report.comment_lines = lines.comment;
    report.blank_lines = lines.blank;
    report.size_class = SizeClass::from_file_count(report.total_files);
    report.categories = categories.into_values().collect();

    let mut directory_records: Vec<DirectoryRecord> = directories
        .into_iter()
        .map(|(path, accumulator)| DirectoryRecord {
            depth: u32::try_from(paths::depth(&path)).unwrap_or(u32::MAX),
            primary_language: dominant(&accumulator.languages),
            path,
            files: accumulator.files,
            bytes: accumulator.bytes,
            code_lines: accumulator.code_lines,
        })
        .collect();
    if directory_records.len() > MAX_DIRECTORIES {
        directory_records.sort_by(|a, b| a.depth.cmp(&b.depth).then_with(|| a.path.cmp(&b.path)));
        directory_records.truncate(MAX_DIRECTORIES);
        directory_records.sort_by(|a, b| a.path.cmp(&b.path));
        report.truncated = true;
        report.notes.push(format!(
            "Directory statistics are limited to the {MAX_DIRECTORIES} shallowest directories."
        ));
    }
    report.directories = directory_records;

    let mut symbols = Vec::new();
    'files: for file in scan.files.iter().filter(|file| file.first_party_code()) {
        let Some(analysis) = &file.analysis else {
            continue;
        };
        for symbol in &analysis.symbols {
            if symbols.len() >= max_symbols {
                report.truncated = true;
                report.notes.push(format!(
                    "Symbols are limited to the first {max_symbols} in path order."
                ));
                break 'files;
            }
            symbols.push(SymbolRecord {
                name: symbol.name.clone(),
                kind: symbol.kind,
                path: file.record.path.clone(),
                line: symbol.line,
                end_line: Some(symbol.end_line),
                complexity: symbol.complexity,
            });
        }
    }
    report.symbols = symbols;
    report.files = scan.files.iter().map(|file| file.record.clone()).collect();
    report
}

/// Builds the language section. Shares count first-party code in languages that count
/// toward composition (not data, prose, or build files).
pub fn language_report(scan: &ScanResult, registry: &LanguageRegistry) -> LanguageReport {
    struct Stat {
        files: u64,
        bytes: u64,
        lines: LineCounts,
        first_party_code: u64,
    }
    let mut stats: BTreeMap<String, Stat> = BTreeMap::new();
    let mut unrecognized = 0u64;
    for file in &scan.files {
        let record = &file.record;
        let Some(language) = &record.language else {
            if !record.binary && record.lines.is_some() {
                unrecognized += 1;
            }
            continue;
        };
        let stat = stats.entry(language.clone()).or_insert(Stat {
            files: 0,
            bytes: 0,
            lines: LineCounts::default(),
            first_party_code: 0,
        });
        stat.files += 1;
        stat.bytes += record.bytes;
        if let Some(lines) = &record.lines {
            stat.lines.add(lines);
            if !record.vendored && !record.generated {
                stat.first_party_code += lines.code;
            }
        }
    }
    let kind_of = |id: &str| {
        registry
            .get(id)
            .map_or(LanguageKind::Programming, |spec| spec.kind)
    };
    let share_total: u64 = stats
        .iter()
        .filter(|(id, _)| kind_of(id).counts_toward_share())
        .map(|(_, stat)| stat.first_party_code)
        .sum();
    let mut languages: Vec<LanguageStat> = stats
        .into_iter()
        .map(|(id, stat)| {
            let spec = registry.get(&id);
            let kind = kind_of(&id);
            LanguageStat {
                name: spec.map_or_else(|| id.clone(), |spec| spec.name.clone()),
                kind,
                files: stat.files,
                bytes: stat.bytes,
                code_lines: stat.lines.code,
                comment_lines: stat.lines.comment,
                blank_lines: stat.lines.blank,
                share: if kind.counts_toward_share() && share_total > 0 {
                    round4(stat.first_party_code as f64 / share_total as f64)
                } else {
                    0.0
                },
                capability: spec.map_or(ParserCapability::Detection, |spec| spec.capability()),
                id,
            }
        })
        .collect();
    languages.sort_by(|a, b| {
        b.code_lines
            .cmp(&a.code_lines)
            .then_with(|| b.files.cmp(&a.files))
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut by_share: Vec<&LanguageStat> = languages.iter().filter(|l| l.share > 0.0).collect();
    by_share.sort_by(|a, b| b.share.total_cmp(&a.share).then_with(|| a.id.cmp(&b.id)));
    let mut primary: Vec<String> = by_share
        .iter()
        .filter(|l| l.share >= PRIMARY_SHARE)
        .take(MAX_PRIMARY)
        .map(|l| l.id.clone())
        .collect();
    if primary.is_empty()
        && let Some(top) = by_share.first()
    {
        primary.push(top.id.clone());
    }
    LanguageReport {
        status: SectionStatus::Analyzed,
        notes: Vec::new(),
        languages,
        primary,
        interactions: Vec::new(),
        unrecognized_files: unrecognized,
    }
}

/// Formats a byte count with a binary unit, e.g. `12.5 MiB`.
fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} bytes");
    }
    let mut value = bytes as f64;
    let mut unit = "";
    for candidate in UNITS {
        value /= 1024.0;
        unit = candidate;
        if value < 1024.0 {
            break;
        }
    }
    format!("{value:.1} {unit}")
}

/// Derives structure findings: committed binary files above the size threshold.
pub fn structure_findings(report: &StructureReport, thresholds: &Thresholds) -> Vec<Finding> {
    let limit = thresholds.large_binary_bytes;
    let mut large: Vec<_> = report
        .files
        .iter()
        .filter(|file| file.binary && file.bytes >= limit)
        .collect();
    large.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.path.cmp(&b.path)));
    large
        .into_iter()
        .take(MAX_BINARY_FINDINGS)
        .map(|file| {
            Finding::new("structure.large-binary", &file.path, FindingCategory::Structure, Severity::Info, Confidence::High, format!("{} is a {} binary file", file.path, format_bytes(file.bytes)))
                .summary(format!("The file is {} ({} bytes); the threshold is {}.", format_bytes(file.bytes), file.bytes, format_bytes(limit)))
                .rationale("Every clone downloads every committed version of a binary file, so large binaries keep the repository heavy even after they are replaced or deleted.")
                .method("File size compared with the large-binary threshold; binary files are recognized by extension or by NUL bytes in their first 8,000 bytes.")
                .evidence(Evidence::file(&file.path))
                .evidence(Evidence::metric_with_threshold("structure.file.bytes", file.bytes as f64, limit as f64, "bytes"))
                .limitation("Only the current version is measured; earlier versions in the history can be larger or more numerous.")
                .next_step("Consider Git LFS, release assets, or fetching the file during the build.")
                .path(file.path.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::ScannedFile;
    use repodna_core::model::structure::FileRecord;

    fn file(path: &str, category: FileCategory, language: Option<&str>, code: u64) -> ScannedFile {
        let mut record = FileRecord::new(path, category, code * 10);
        record.language = language.map(str::to_owned);
        record.lines = Some(LineCounts {
            total: code + 2,
            code,
            comment: 1,
            blank: 1,
        });
        ScannedFile {
            record,
            language_kind: None,
            analysis: None,
            tokens: None,
            sha256: None,
            mode: None,
            inline_tests: false,
        }
    }

    fn scan_of(files: Vec<ScannedFile>) -> ScanResult {
        ScanResult {
            files,
            ..ScanResult::default()
        }
    }

    #[test]
    fn aggregates_structure() {
        let mut vendored = file(
            "vendor/lib.js",
            FileCategory::Vendor,
            Some("javascript"),
            500,
        );
        vendored.record.vendored = true;
        let scan = scan_of(vec![
            file("src/main.rs", FileCategory::Source, Some("rust"), 100),
            file("src/util/mod.rs", FileCategory::Source, Some("rust"), 50),
            file(
                "README.md",
                FileCategory::Documentation,
                Some("markdown"),
                20,
            ),
            vendored,
        ]);
        let report = structure_report(&scan, 10);
        assert_eq!(report.total_files, 4);
        assert_eq!(report.code_lines, 670);
        assert_eq!(report.vendored_files, 1);
        assert_eq!(report.size_class, SizeClass::Tiny);
        let dirs: Vec<(&str, u64, Option<&str>)> = report
            .directories
            .iter()
            .map(|d| (d.path.as_str(), d.files, d.primary_language.as_deref()))
            .collect();
        assert_eq!(
            dirs,
            vec![
                ("", 4, Some("rust")),
                ("src", 2, Some("rust")),
                ("src/util", 1, Some("rust")),
                ("vendor", 1, None),
            ]
        );
        assert_eq!(report.categories[0].category, FileCategory::Source);
        assert_eq!(report.files.len(), 4);
    }

    #[test]
    fn computes_language_shares_from_first_party_code() {
        let mut vendored = file(
            "vendor/lib.js",
            FileCategory::Vendor,
            Some("javascript"),
            900,
        );
        vendored.record.vendored = true;
        let scan = scan_of(vec![
            file("src/main.rs", FileCategory::Source, Some("rust"), 300),
            file("web/app.ts", FileCategory::Source, Some("typescript"), 100),
            file("data.json", FileCategory::Data, Some("json"), 5_000),
            file("notes", FileCategory::Other, None, 3),
            vendored,
        ]);
        let report = language_report(&scan, LanguageRegistry::builtin());
        let shares: Vec<(&str, f64)> = report
            .languages
            .iter()
            .map(|l| (l.id.as_str(), l.share))
            .collect();
        assert_eq!(
            shares,
            vec![
                ("json", 0.0),
                ("javascript", 0.0),
                ("rust", 0.75),
                ("typescript", 0.25)
            ]
        );
        assert_eq!(report.primary, vec!["rust", "typescript"]);
        assert_eq!(report.unrecognized_files, 1);
        assert_eq!(report.languages[2].name, "Rust");
    }

    #[test]
    fn reports_large_binary_files() {
        let mut video = file("media/intro.mp4", FileCategory::Asset, None, 0);
        video.record.binary = true;
        video.record.bytes = 12 * 1024 * 1024;
        let mut small = file("media/icon.png", FileCategory::Asset, None, 0);
        small.record.binary = true;
        small.record.bytes = 2048;
        let mut text = file("data/big.csv", FileCategory::Data, None, 0);
        text.record.bytes = 50 * 1024 * 1024;
        let report = structure_report(&scan_of(vec![video, small, text]), 10);
        let findings = structure_findings(&report, &Thresholds::default());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "structure.large-binary");
        assert_eq!(
            findings[0].title,
            "media/intro.mp4 is a 12.0 MiB binary file"
        );
        assert_eq!(format_bytes(512), "512 bytes");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(3 * 1024 * 1024 * 1024), "3.0 GiB");
    }
}
