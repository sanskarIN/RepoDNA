//! Evidence about one module, directory, or file, and redacted source excerpts.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use repodna_core::model::artifact::RepositoryDna;
use repodna_core::paths;
use repodna_core::redact::redact_secrets;
use repodna_core::text::count;

use super::general::{edges, findings, hotspots, is_within, module_path, modules};
use super::{Candidates, EXCERPT_BYTES, EXCERPT_FILES, EXCERPT_LINES, EvidenceKind};
use crate::error::AiError;

/// Files in `dir`, largest first.
pub(super) fn module_files(dna: &RepositoryDna, c: &mut Candidates, dir: &str, limit: usize) {
    let mut files: Vec<_> = dna
        .structure
        .files
        .iter()
        .filter(|file| is_within(&file.path, dir))
        .collect();
    files.sort_by_key(|file| {
        (
            std::cmp::Reverse(file.lines.as_ref().map_or(0, |lines| lines.code)),
            file.path.clone(),
        )
    });
    for file in files.into_iter().take(limit) {
        let lines = file.lines.as_ref().map_or(0, |lines| lines.code);
        let language = file.language.as_deref().unwrap_or("unknown language");
        let mut detail = format!(
            "{}, {language}, {lines} lines of code",
            file.category.label()
        );
        if let Some(analysis) = &file.analysis {
            detail.push_str(&format!(
                ", {} functions, highest complexity {}",
                analysis.functions, analysis.cyclomatic_max
            ));
        }
        detail.push('.');
        c.push(EvidenceKind::File, &file.path, detail);
    }
}

pub(super) fn file_history(dna: &RepositoryDna, c: &mut Candidates, dir: &str, limit: usize) {
    let mut records: Vec<_> = dna
        .git
        .file_history
        .iter()
        .filter(|record| is_within(&record.path, dir))
        .collect();
    records.sort_by_key(|record| (std::cmp::Reverse(record.commits), record.path.clone()));
    for record in records.into_iter().take(limit) {
        c.push(
            EvidenceKind::File,
            &record.path,
            format!(
                "{} ({} recent) by {}; +{} −{}; first seen {}, last changed {}.",
                count(u64::from(record.commits), "commit", "commits"),
                record.recent_commits,
                count(u64::from(record.authors), "author", "authors"),
                record.insertions,
                record.deletions,
                record.first_seen.date_string(),
                record.last_changed.date_string()
            ),
        );
    }
}

pub(super) fn complex_functions(dna: &RepositoryDna, c: &mut Candidates, dir: &str, limit: usize) {
    for function in dna
        .code_quality
        .complexity
        .top_functions
        .iter()
        .filter(|function| is_within(&function.path, dir))
        .take(limit)
    {
        c.push(
            EvidenceKind::File,
            format!("{}:{}", function.path, function.line),
            format!(
                "Function {}: {} lines, cyclomatic complexity {}, nesting depth {}.",
                function.name, function.lines, function.cyclomatic, function.nesting
            ),
        );
    }
}

/// Reads up to [`EXCERPT_LINES`] redacted lines from a repository file.
pub(super) fn read_excerpt(root: &Path, path: &str) -> Option<String> {
    let relative = paths::normalize_relative(path)?;
    if relative.is_empty() {
        return None;
    }
    let full = root.join(&relative);
    let metadata = std::fs::symlink_metadata(&full).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    File::open(&full)
        .ok()?
        .take(EXCERPT_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.contains(&0) {
        return None;
    }
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<String> = text
        .lines()
        .take(EXCERPT_LINES)
        .enumerate()
        .map(|(index, line)| {
            let line: String = line.chars().take(200).collect();
            format!("{:>4}| {}", index + 1, redact_secrets(&line))
        })
        .collect();
    (!lines.is_empty()).then(|| crate::text::sanitize(&lines.join("\n"), 12_000))
}

pub(super) fn excerpts(root: &Path, c: &mut Candidates, candidates: &[String]) {
    for path in candidates.iter().take(EXCERPT_FILES) {
        if let Some(text) = read_excerpt(root, path) {
            c.push_excerpt(format!("{path} (first {EXCERPT_LINES} lines)"), text);
        }
    }
}

/// Finds the module or directory a subject names.
pub(super) fn resolve_directory(dna: &RepositoryDna, subject: &str) -> Result<String, AiError> {
    if let Some(module) = dna
        .architecture
        .modules
        .iter()
        .find(|module| module.path == subject || module.name == subject || module.id == subject)
    {
        return Ok(module.path.clone());
    }
    if dna
        .structure
        .files
        .iter()
        .any(|file| paths::is_within(&file.path, subject))
    {
        return Ok(subject.to_owned());
    }
    Err(AiError::UnknownSubject(format!(
        "no module or directory `{subject}` in this analysis; `repodna architecture` lists the modules"
    )))
}

pub(super) fn module_task(
    dna: &RepositoryDna,
    c: &mut Candidates,
    subject: &str,
    root: Option<&Path>,
) -> Result<(), AiError> {
    let dir = resolve_directory(dna, subject)?;
    let within = |path: &str| is_within(path, &dir);
    modules(dna, c, 3, |path| path == dir);
    edges(dna, c, 16, |path| path == dir);
    let files = dna
        .structure
        .files
        .iter()
        .filter(|file| within(&file.path))
        .count();
    c.push(
        EvidenceKind::Module,
        if dir.is_empty() {
            "(repository root)"
        } else {
            &dir
        },
        format!(
            "Contains {} in this analysis.",
            count(files as u64, "file", "files")
        ),
    );
    hotspots(dna, c, 5, within);
    findings(dna, c, 10, |finding| {
        finding.paths.iter().any(|path| within(path))
    });
    module_files(dna, c, &dir, 12);
    complex_functions(dna, c, &dir, 6);
    file_history(dna, c, &dir, 6);
    if let Some(root) = root {
        let mut candidates: Vec<String> = dna
            .structure
            .entrypoints
            .iter()
            .map(|entry| entry.path.clone())
            .filter(|path| within(path))
            .collect();
        for file in &dna.insights.important_files {
            if within(&file.path) && !candidates.contains(&file.path) {
                candidates.push(file.path.clone());
            }
        }
        excerpts(root, c, &candidates);
    }
    Ok(())
}

pub(super) fn hotspot_task(
    dna: &RepositoryDna,
    c: &mut Candidates,
    subject: &str,
    root: Option<&Path>,
) -> Result<(), AiError> {
    let known = dna.structure.files.iter().any(|file| file.path == subject)
        || dna
            .git
            .file_history
            .iter()
            .any(|record| record.path == subject);
    if !known {
        return Err(AiError::UnknownSubject(format!(
            "no file `{subject}` in this analysis; `repodna hotspots` lists the ranked files"
        )));
    }
    let ranked = dna.git.hot_spots.len();
    if dna.git.hot_spots.iter().any(|h| h.path == subject) {
        hotspots(dna, c, 1, |path| path == subject);
    } else {
        c.push(
            EvidenceKind::Fact,
            subject,
            format!("The file is not among the {ranked} files ranked as hotspots."),
        );
    }
    file_history(dna, c, subject, 1);
    module_files(dna, c, subject, 1);
    complex_functions(dna, c, subject, 5);
    findings(dna, c, 8, |finding| {
        finding.paths.iter().any(|path| path == subject)
    });
    if let Some(module) = dna
        .structure
        .files
        .iter()
        .find(|file| file.path == subject)
        .and_then(|file| file.module.as_deref())
    {
        let module = module_path(dna, module);
        modules(dna, c, 1, |path| path == module);
        edges(dna, c, 8, |path| path == module);
    }
    hotspots(dna, c, 5, |path| path != subject);
    if let Some(root) = root {
        excerpts(root, c, &[subject.to_owned()]);
    }
    Ok(())
}
