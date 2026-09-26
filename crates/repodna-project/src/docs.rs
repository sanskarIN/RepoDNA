//! Documentation: README structure, license identification, and a documentation checklist.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;
use repodna_core::confidence::Confidence;
use repodna_core::evidence::Evidence;
use repodna_core::model::SectionStatus;
use repodna_core::model::project::{DocCheck, DocCheckStatus, DocsReport, LicenseInfo, ReadmeInfo};
use repodna_core::model::structure::FileCategory;
use repodna_core::paths;

use crate::{ProjectFile, is_license, is_readme};

/// Maximum README headings recorded.
const MAX_HEADINGS: usize = 50;

/// Maximum characters of a quoted description.
const MAX_DESCRIPTION: usize = 300;

static MARKDOWN_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[^!])\[[^\]]*\]\([^)\s]+")
        .unwrap_or_else(|error| panic!("invalid link pattern: {error}"))
});
static MARKDOWN_IMAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"!\[[^\]]*\]\([^)\s]+")
        .unwrap_or_else(|error| panic!("invalid image pattern: {error}"))
});
static MARKDOWN_ANY_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"!?\[[^\]]*\]\([^)]*\)")
        .unwrap_or_else(|error| panic!("invalid link pattern: {error}"))
});

static MANIFEST_LICENSE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*"?license"?\s*[:=]\s*(?:\{\s*text\s*=\s*)?"([^"]+)""#)
        .unwrap_or_else(|error| panic!("invalid manifest license pattern: {error}"))
});

const INSTALL_WORDS: &[&str] = &[
    "install",
    "setup",
    "set up",
    "getting started",
    "quick start",
    "quickstart",
    "build",
    "requirements",
    "prerequisites",
    "download",
];
const USAGE_WORDS: &[&str] = &[
    "usage",
    "example",
    "getting started",
    "quick start",
    "quickstart",
    "how to",
    "tutorial",
    "cli",
    "commands",
    "running",
    "run ",
];

fn heading_mentions(headings: &[String], words: &[&str]) -> bool {
    headings.iter().any(|heading| {
        let lower = format!("{} ", heading.to_ascii_lowercase());
        words.iter().any(|word| lower.contains(word))
    })
}

/// Parses the structure of a README (Markdown, reStructuredText, AsciiDoc, or plain text).
pub fn parse_readme(path: &str, text: &str) -> ReadmeInfo {
    let extension = paths::extension(path).unwrap_or_default();
    let mut headings = Vec::new();
    let mut code_blocks = 0u32;
    let mut in_fence = false;
    let lines: Vec<&str> = text.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            if !in_fence {
                code_blocks += 1;
            }
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let heading = match extension.as_str() {
            "rst" => {
                let underline = lines.get(index + 1).map(|l| l.trim()).unwrap_or_default();
                (!trimmed.is_empty()
                    && underline.len() >= trimmed.len().min(3)
                    && underline
                        .chars()
                        .all(|c| matches!(c, '=' | '-' | '~' | '^' | '*' | '#'))
                    && !underline.is_empty())
                .then_some(trimmed)
            }
            "adoc" | "asciidoc" => trimmed
                .strip_prefix('=')
                .map(|rest| rest.trim_start_matches('=').trim()),
            _ => trimmed
                .strip_prefix('#')
                .filter(|rest| rest.starts_with([' ', '#']))
                .map(|rest| rest.trim_start_matches('#').trim()),
        };
        if extension == "rst" && trimmed.starts_with(".. code") {
            code_blocks += 1;
        }
        if let Some(heading) = heading.filter(|h| !h.is_empty())
            && headings.len() < MAX_HEADINGS
        {
            headings.push(heading.chars().take(100).collect());
        }
    }
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    let links = count(
        text.lines()
            .map(|line| MARKDOWN_LINK.find_iter(line).count())
            .sum(),
    );
    let images = count(MARKDOWN_IMAGE.find_iter(text).count() + text.matches("<img ").count());
    ReadmeInfo {
        path: path.to_owned(),
        lines: lines.len() as u64,
        words: text.split_whitespace().count() as u64,
        has_installation: heading_mentions(&headings, INSTALL_WORDS),
        has_usage: heading_mentions(&headings, USAGE_WORDS),
        headings,
        code_blocks,
        links,
        images,
    }
}

/// Identifies a license text. Returns the SPDX identifier and the confidence.
pub fn detect_license(text: &str) -> Option<(&'static str, Confidence)> {
    let normalized = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let has = |needle: &str| normalized.contains(needle);
    let high = Confidence::High;
    let found = if has("gnu affero general public license") {
        ("AGPL-3.0", high)
    } else if has("gnu lesser general public license") {
        if has("version 2.1") {
            ("LGPL-2.1", high)
        } else {
            ("LGPL-3.0", high)
        }
    } else if has("gnu general public license") {
        if has("version 3") {
            ("GPL-3.0", high)
        } else {
            ("GPL-2.0", Confidence::Medium)
        }
    } else if has("apache license") && has("version 2.0") {
        ("Apache-2.0", high)
    } else if has("mozilla public license") && has("2.0") {
        ("MPL-2.0", high)
    } else if has("eclipse public license") {
        if has("2.0") {
            ("EPL-2.0", high)
        } else {
            ("EPL-1.0", Confidence::Medium)
        }
    } else if has("boost software license") {
        ("BSL-1.0", high)
    } else if has("this is free and unencumbered software released into the public domain") {
        ("Unlicense", high)
    } else if has("cc0 1.0 universal") {
        ("CC0-1.0", high)
    } else if has("permission is hereby granted, free of charge") {
        ("MIT", high)
    } else if has(
        "permission to use, copy, modify, and/or distribute this software for any purpose",
    ) {
        ("ISC", high)
    } else if has("redistribution and use in source and binary forms") {
        if has("neither the name") || has("may be used to endorse or promote") {
            ("BSD-3-Clause", high)
        } else {
            ("BSD-2-Clause", Confidence::Medium)
        }
    } else if has("altered source versions must be plainly marked") {
        ("Zlib", high)
    } else if has("mit license") {
        ("MIT", Confidence::Medium)
    } else {
        return None;
    };
    Some(found)
}

fn check(
    id: &str,
    label: &str,
    optional: bool,
    status: DocCheckStatus,
    evidence: Vec<Evidence>,
) -> DocCheck {
    DocCheck {
        id: id.to_owned(),
        label: label.to_owned(),
        status,
        optional,
        evidence,
    }
}

/// Finds a file at the root, in `.github/`, or in `docs/` whose upper-cased stem is one
/// of `stems`.
fn community_file<'a>(files: &[ProjectFile<'a>], stems: &[&str]) -> Option<&'a str> {
    files
        .iter()
        // A script named security.py or history.js is not a security policy or changelog.
        .filter(|file| !matches!(file.category, FileCategory::Source | FileCategory::Test))
        .map(|file| file.path)
        .filter(|path| {
            let parent = paths::parent(path);
            matches!(parent, "" | ".github" | "docs" | ".gitlab")
                && stems.contains(&paths::file_stem(path).to_ascii_uppercase().as_str())
        })
        .min_by_key(|path| (paths::depth(path), *path))
}

/// Assembles the documentation section.
pub fn docs_report(
    files: &[ProjectFile<'_>],
    contents: &BTreeMap<String, String>,
    descriptions: &[(String, String)],
) -> DocsReport {
    let mut report = DocsReport {
        status: SectionStatus::Analyzed,
        ..DocsReport::default()
    };

    // README: prefer the root, Markdown first.
    let mut readmes: Vec<&str> = files
        .iter()
        .map(|file| file.path)
        .filter(|path| is_readme(path) && paths::depth(path) <= 2)
        .collect();
    readmes.sort_by_key(|path| {
        (
            paths::depth(path),
            !path.to_ascii_lowercase().ends_with(".md"),
            *path,
        )
    });
    let readme = readmes
        .first()
        .and_then(|path| contents.get(*path).map(|text| parse_readme(path, text)));
    let readme_status = match &readme {
        Some(info) if paths::depth(&info.path) == 1 && info.words >= 30 => DocCheckStatus::Present,
        Some(_) => DocCheckStatus::Partial,
        None => DocCheckStatus::NotDetected,
    };
    let readme_evidence: Vec<Evidence> = readme
        .iter()
        .map(|info| Evidence::file(&info.path).with_note(format!("{} words", info.words)))
        .collect();
    let headings: Vec<String> = readme
        .iter()
        .flat_map(|info| info.headings.clone())
        .collect();

    // License files, possibly several (dual licensing).
    let mut license_files: Vec<&str> = files
        .iter()
        .map(|file| file.path)
        .filter(|path| paths::depth(path) == 1 && is_license(path))
        .collect();
    license_files.sort_unstable();
    let recognized: Vec<(&str, &str, Confidence)> = license_files
        .iter()
        .filter_map(|path| {
            let (spdx, confidence) = detect_license(contents.get(*path)?)?;
            Some((*path, spdx, confidence))
        })
        .collect();
    let mut license_status = DocCheckStatus::NotDetected;
    let mut license_evidence = Vec::new();
    if let Some(first) = license_files.first() {
        license_status = DocCheckStatus::Present;
        license_evidence.push(Evidence::file(*first));
        let spdx: BTreeSet<&str> = recognized.iter().map(|(_, spdx, _)| *spdx).collect();
        report.license = Some(LicenseInfo {
            path: (*first).to_owned(),
            spdx: (!spdx.is_empty()).then(|| spdx.into_iter().collect::<Vec<_>>().join(" OR ")),
            confidence: match recognized.len() {
                0 => Confidence::Low,
                1 => recognized[0].2,
                _ => Confidence::Medium,
            },
        });
    } else {
        let declared = [
            "Cargo.toml",
            "package.json",
            "pyproject.toml",
            "composer.json",
        ]
        .iter()
        .filter_map(|manifest| {
            let text = contents.get(*manifest)?;
            let captures = MANIFEST_LICENSE.captures(text)?;
            Some((*manifest, captures[1].to_owned()))
        })
        .next();
        if let Some((manifest, spdx)) = declared {
            license_status = DocCheckStatus::Partial;
            license_evidence.push(
                Evidence::file(manifest)
                    .with_note(format!("declares license {spdx}; no license file")),
            );
            report.license = Some(LicenseInfo {
                path: manifest.to_owned(),
                spdx: Some(spdx),
                confidence: Confidence::Medium,
            });
        }
    }

    let with_heading =
        |stems: &[&str], heading_words: &[&str]| -> (DocCheckStatus, Vec<Evidence>) {
            if let Some(path) = community_file(files, stems) {
                (DocCheckStatus::Present, vec![Evidence::file(path)])
            } else if !heading_words.is_empty() && heading_mentions(&headings, heading_words) {
                let path = readme.as_ref().map_or("README", |info| info.path.as_str());
                (
                    DocCheckStatus::Partial,
                    vec![Evidence::file(path).with_note("README section")],
                )
            } else {
                (DocCheckStatus::NotDetected, Vec::new())
            }
        };
    let dir_with = |names: &[&str]| -> Option<String> {
        files
            .iter()
            .filter_map(|file| {
                let parts: Vec<&str> = file.path.split('/').collect();
                let position = parts[..parts.len() - 1]
                    .iter()
                    .position(|part| names.contains(&part.to_ascii_lowercase().as_str()))?;
                Some(parts[..=position].join("/"))
            })
            .min_by_key(|dir| (paths::depth(dir), dir.clone()))
    };
    let github_file = |matches: &dyn Fn(&str) -> bool| -> Option<&str> {
        files
            .iter()
            .map(|file| file.path)
            .find(|path| matches(&path.to_ascii_lowercase()))
    };

    let mut checks = vec![
        check(
            "readme",
            "README",
            false,
            readme_status,
            readme_evidence.clone(),
        ),
        check(
            "license",
            "License",
            false,
            license_status,
            license_evidence,
        ),
    ];
    let readme_section = |present: bool, label: &str| {
        let status = if present {
            DocCheckStatus::Present
        } else {
            DocCheckStatus::NotDetected
        };
        let evidence = if present {
            readme_evidence
                .iter()
                .cloned()
                .map(|e| e.with_note(format!("{label} section")))
                .collect()
        } else {
            Vec::new()
        };
        (status, evidence)
    };
    let (status, evidence) = readme_section(
        readme.as_ref().is_some_and(|r| r.has_installation),
        "installation",
    );
    checks.push(check(
        "installation",
        "Installation instructions",
        false,
        status,
        evidence,
    ));
    let (status, evidence) = readme_section(readme.as_ref().is_some_and(|r| r.has_usage), "usage");
    checks.push(check(
        "usage",
        "Usage instructions",
        false,
        status,
        evidence,
    ));
    for (id, label, stems, words) in [
        (
            "contributing",
            "Contributing guide",
            &["CONTRIBUTING"][..],
            &["contribut"][..],
        ),
        (
            "code-of-conduct",
            "Code of conduct",
            &["CODE_OF_CONDUCT", "CODE-OF-CONDUCT"][..],
            &["code of conduct"][..],
        ),
        (
            "security-policy",
            "Security policy",
            &["SECURITY"][..],
            &["security"][..],
        ),
        (
            "changelog",
            "Changelog",
            &["CHANGELOG", "CHANGES", "HISTORY", "NEWS", "RELEASES"][..],
            &[][..],
        ),
        (
            "support",
            "Support information",
            &["SUPPORT"][..],
            &["support"][..],
        ),
        ("codeowners", "Code owners", &["CODEOWNERS"][..], &[][..]),
        ("citation", "Citation file", &["CITATION"][..], &[][..]),
    ] {
        let (status, evidence) = with_heading(stems, words);
        checks.push(check(id, label, true, status, evidence));
    }
    let doc_dir = dir_with(&["docs", "doc", "documentation", "website", "book", "manual"]);
    let example_dir = dir_with(&["examples", "example", "samples", "sample", "demo", "demos"]);
    for (id, label, dir) in [
        ("docs-directory", "Documentation directory", &doc_dir),
        ("examples", "Examples", &example_dir),
    ] {
        let (status, evidence) = match dir {
            Some(dir) => (
                DocCheckStatus::Present,
                vec![Evidence::directory(dir.as_str())],
            ),
            None => (DocCheckStatus::NotDetected, Vec::new()),
        };
        checks.push(check(id, label, true, status, evidence));
    }
    for (id, label, found) in [
        (
            "issue-templates",
            "Issue templates",
            github_file(&|path| path.starts_with(".github/issue_template")),
        ),
        (
            "pull-request-template",
            "Pull request template",
            github_file(&|path| {
                paths::file_name(path).starts_with("pull_request_template")
                    || path.starts_with(".github/pull_request_template/")
            }),
        ),
    ] {
        let (status, evidence) = match found {
            Some(path) => (DocCheckStatus::Present, vec![Evidence::file(path)]),
            None => (DocCheckStatus::NotDetected, Vec::new()),
        };
        checks.push(check(id, label, true, status, evidence));
    }
    report.checks = checks;

    // Documentation files, directories, and examples.
    let docs: Vec<&ProjectFile<'_>> = files
        .iter()
        .filter(|file| file.category == FileCategory::Documentation)
        .collect();
    report.doc_files = docs.len() as u64;
    report.doc_lines = docs.iter().map(|file| file.total_lines).sum();
    let mut doc_directories = BTreeSet::new();
    let mut examples = BTreeSet::new();
    for file in files {
        let parts: Vec<&str> = file.path.split('/').collect();
        for (position, part) in parts[..parts.len() - 1].iter().enumerate() {
            let lower = part.to_ascii_lowercase();
            if file.category == FileCategory::Documentation
                && matches!(
                    lower.as_str(),
                    "docs" | "doc" | "documentation" | "website" | "book" | "manual"
                )
            {
                doc_directories.insert(parts[..=position].join("/"));
                break;
            }
            if matches!(
                lower.as_str(),
                "examples" | "example" | "samples" | "sample" | "demo" | "demos"
            ) {
                examples.insert(parts[..=position].join("/"));
                break;
            }
        }
    }
    report.doc_directories = doc_directories.into_iter().take(50).collect();
    report.examples = examples.into_iter().take(50).collect();

    // A purpose statement for the whole repository: a root manifest's description, then
    // the root README's first paragraph. A nested package's description describes only that
    // package, so it is used only when it is the repository's single description.
    let mut sorted_descriptions: Vec<&(String, String)> = descriptions.iter().collect();
    sorted_descriptions.sort_by_key(|(manifest, _)| (paths::depth(manifest), manifest.clone()));
    let root_manifest = sorted_descriptions
        .iter()
        .find(|(manifest, _)| paths::depth(manifest) == 1);
    let readme_paragraph = readme
        .as_ref()
        .filter(|info| paths::depth(&info.path) == 1)
        .and_then(|info| {
            let text = contents.get(&info.path)?;
            Some((info.path.clone(), first_paragraph(text)?))
        });
    let chosen = if let Some((manifest, description)) = root_manifest {
        Some((
            manifest.clone(),
            description.chars().take(MAX_DESCRIPTION).collect(),
        ))
    } else if let Some(found) = readme_paragraph {
        Some(found)
    } else if let [(manifest, description)] = sorted_descriptions.as_slice() {
        Some((
            manifest.clone(),
            description.chars().take(MAX_DESCRIPTION).collect(),
        ))
    } else {
        None
    };
    if let Some((source, description)) = chosen {
        report.description = Some(description);
        report.description_source = Some(source);
    }
    report.readme = readme;
    report
}

/// The first prose paragraph of a README, skipping headings, badges, and HTML.
fn first_paragraph(text: &str) -> Option<String> {
    let mut in_fence = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence
            || trimmed.is_empty()
            || trimmed.starts_with(['#', '<', '!', '[', '|', '=', '-', '*', '>'])
            || trimmed.len() < 20
        {
            continue;
        }
        let plain = MARKDOWN_ANY_LINK.replace_all(trimmed, "");
        let plain = plain.trim();
        if plain.len() >= 20 {
            return Some(plain.chars().take(MAX_DESCRIPTION).collect());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const README: &str = "# Widget\n\n[![CI](https://ci/badge.svg)](https://ci)\n\nWidget turns YAML into dashboards for small teams, quickly.\n\n## Installation\n\n```sh\ncargo install widget\n# not a heading\n```\n\n## Usage\n\nSee [the guide](docs/guide.md) and ![screenshot](shot.png).\n\n## Contributing\n\nPull requests welcome.\n";

    fn file(path: &str, category: FileCategory) -> ProjectFile<'_> {
        ProjectFile {
            path,
            category,
            language: None,
            code_lines: 0,
            total_lines: 10,
            inline_tests: false,
        }
    }

    #[test]
    fn parses_readme_structure() {
        let info = parse_readme("README.md", README);
        assert_eq!(
            info.headings,
            vec!["Widget", "Installation", "Usage", "Contributing"]
        );
        assert_eq!(info.code_blocks, 1);
        assert_eq!(info.links, 2);
        assert_eq!(info.images, 2);
        assert!(info.has_installation && info.has_usage);
        let rst = parse_readme("README.rst", "Title\n=====\n\nText.\n\nUsage\n-----\n");
        assert_eq!(rst.headings, vec!["Title", "Usage"]);
        assert_eq!(
            first_paragraph(README).as_deref(),
            Some("Widget turns YAML into dashboards for small teams, quickly.")
        );
    }

    #[test]
    fn identifies_common_licenses() {
        let cases = [
            ("Apache License\nVersion 2.0, January 2004", "Apache-2.0"),
            (
                "MIT License\n\nPermission is hereby granted, free of charge, to any person",
                "MIT",
            ),
            (
                "GNU GENERAL PUBLIC LICENSE\nVersion 3, 29 June 2007",
                "GPL-3.0",
            ),
            (
                "Redistribution and use in source and binary forms ... Neither the name of",
                "BSD-3-Clause",
            ),
            (
                "This is free and unencumbered software released into the public domain.",
                "Unlicense",
            ),
            ("Mozilla Public License Version 2.0", "MPL-2.0"),
        ];
        for (text, spdx) in cases {
            assert_eq!(detect_license(text).map(|(id, _)| id), Some(spdx), "{text}");
        }
        assert!(detect_license("All rights reserved.").is_none());
    }

    #[test]
    fn builds_the_docs_report() {
        let files = [
            file("README.md", FileCategory::Documentation),
            file("LICENSE-APACHE", FileCategory::License),
            file("LICENSE-MIT", FileCategory::License),
            file("SECURITY.md", FileCategory::Documentation),
            file("docs/guide.md", FileCategory::Documentation),
            file("examples/basic/main.rs", FileCategory::Source),
            file(
                ".github/ISSUE_TEMPLATE/bug.yml",
                FileCategory::Configuration,
            ),
            file("src/lib.rs", FileCategory::Source),
            // A script, not a changelog.
            file("history.py", FileCategory::Source),
        ];
        let contents: BTreeMap<String, String> = [
            ("README.md", README),
            ("LICENSE-APACHE", "Apache License\nVersion 2.0"),
            (
                "LICENSE-MIT",
                "Permission is hereby granted, free of charge",
            ),
        ]
        .into_iter()
        .map(|(p, t)| (p.to_owned(), t.to_owned()))
        .collect();
        let report = docs_report(&files, &contents, &[]);
        let license = report.license.as_ref().unwrap();
        assert_eq!(license.spdx.as_deref(), Some("Apache-2.0 OR MIT"));
        assert_eq!(license.confidence, Confidence::Medium);
        let status = |id: &str| report.checks.iter().find(|c| c.id == id).unwrap().status;
        assert_eq!(status("readme"), DocCheckStatus::Present);
        assert_eq!(status("license"), DocCheckStatus::Present);
        assert_eq!(status("installation"), DocCheckStatus::Present);
        assert_eq!(status("contributing"), DocCheckStatus::Partial);
        assert_eq!(status("security-policy"), DocCheckStatus::Present);
        assert_eq!(status("changelog"), DocCheckStatus::NotDetected);
        assert_eq!(status("docs-directory"), DocCheckStatus::Present);
        assert_eq!(status("examples"), DocCheckStatus::Present);
        assert_eq!(status("issue-templates"), DocCheckStatus::Present);
        assert_eq!(status("pull-request-template"), DocCheckStatus::NotDetected);
        assert_eq!(report.doc_files, 3);
        assert_eq!(report.doc_directories, vec!["docs"]);
        assert_eq!(report.examples, vec!["examples"]);
        assert_eq!(report.description_source.as_deref(), Some("README.md"));

        let manifest_only = docs_report(
            &[file("Cargo.toml", FileCategory::Manifest)],
            &[(
                "Cargo.toml".to_owned(),
                "[package]\nlicense = \"MIT\"\n".to_owned(),
            )]
            .into(),
            &[("Cargo.toml".to_owned(), "A tool.".to_owned())],
        );
        assert_eq!(
            manifest_only.license.as_ref().unwrap().spdx.as_deref(),
            Some("MIT")
        );
        assert_eq!(manifest_only.checks[1].status, DocCheckStatus::Partial);
        assert_eq!(manifest_only.checks[0].status, DocCheckStatus::NotDetected);
        assert_eq!(manifest_only.description.as_deref(), Some("A tool."));
    }

    #[test]
    fn prefers_repository_level_descriptions() {
        let files = [
            file("README.md", FileCategory::Documentation),
            file("crates/a/Cargo.toml", FileCategory::Manifest),
            file("crates/b/Cargo.toml", FileCategory::Manifest),
        ];
        let contents: BTreeMap<String, String> = [(
            "README.md".to_owned(),
            "# Tool\n\nA repository-wide tool.\n".to_owned(),
        )]
        .into();
        let nested = [
            ("crates/a/Cargo.toml".to_owned(), "Crate A".to_owned()),
            ("crates/b/Cargo.toml".to_owned(), "Crate B".to_owned()),
        ];
        let report = docs_report(&files, &contents, &nested);
        assert_eq!(
            report.description.as_deref(),
            Some("A repository-wide tool.")
        );
        assert_eq!(report.description_source.as_deref(), Some("README.md"));

        let no_readme = docs_report(&files[1..], &BTreeMap::new(), &nested);
        assert_eq!(no_readme.description, None);
        let single = docs_report(&files[1..2], &BTreeMap::new(), &nested[..1]);
        assert_eq!(single.description.as_deref(), Some("Crate A"));

        let mut with_root = nested.to_vec();
        with_root.push(("package.json".to_owned(), "Root package".to_owned()));
        let rooted = docs_report(&files, &contents, &with_root);
        assert_eq!(rooted.description.as_deref(), Some("Root package"));
    }
}
