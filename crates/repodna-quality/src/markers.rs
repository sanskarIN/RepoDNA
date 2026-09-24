//! TODO, FIXME, and similar marker comments.

use std::collections::BTreeMap;

use repodna_core::model::quality::{MarkerCount, MarkerItem, MarkerKind, MarkerReport};
use repodna_core::redact::redact_secrets;

use crate::QualityFile;

/// Maximum marker items listed (counts always cover every marker).
pub const MAX_MARKER_ITEMS: usize = 500;

/// Collects marker comments from all files, test files included. Marker text is passed
/// through secret redaction before it is stored.
pub fn marker_report(files: &[QualityFile<'_>]) -> MarkerReport {
    let mut counts: BTreeMap<MarkerKind, u64> = BTreeMap::new();
    let mut items = Vec::new();
    for file in files {
        let Some(analysis) = file.analysis else {
            continue;
        };
        for marker in &analysis.markers {
            *counts.entry(marker.kind).or_default() += 1;
            items.push(MarkerItem {
                path: file.path.to_owned(),
                line: marker.line,
                kind: marker.kind,
                text: redact_secrets(&marker.text).into_owned(),
            });
        }
    }
    items.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
    let total = items.len() as u64;
    items.truncate(MAX_MARKER_ITEMS);
    MarkerReport {
        counts: counts
            .into_iter()
            .map(|(kind, count)| MarkerCount { kind, count })
            .collect(),
        items,
        total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_parser::{LanguageRegistry, analyze_source};

    #[test]
    fn counts_and_redacts_markers() {
        let spec = LanguageRegistry::builtin().get("rust").unwrap();
        let secret = ["pass", "word=hunter22"].concat();
        let a = analyze_source(
            spec,
            &format!("// TODO: split this\nfn f() {{}}\n// FIXME {secret} is hardcoded\n"),
        );
        let b = analyze_source(spec, "// TODO later\n");
        let files: Vec<QualityFile<'_>> = [("src/a.rs", &a), ("tests/b.rs", &b)]
            .into_iter()
            .map(|(path, analysis)| QualityFile {
                path,
                language: Some("rust"),
                test: path.starts_with("tests/"),
                lines: analysis.lines,
                analysis: Some(analysis),
                tokens: None,
            })
            .collect();
        let report = marker_report(&files);
        assert_eq!(report.total, 3);
        assert_eq!(
            report.counts,
            vec![
                MarkerCount {
                    kind: MarkerKind::Todo,
                    count: 2
                },
                MarkerCount {
                    kind: MarkerKind::Fixme,
                    count: 1
                },
            ]
        );
        let fixme = report
            .items
            .iter()
            .find(|item| item.kind == MarkerKind::Fixme)
            .unwrap();
        assert!(fixme.text.contains("[redacted]"), "{}", fixme.text);
        assert!(!fixme.text.contains("hunter22"));
        assert_eq!(report.items[0].path, "src/a.rs");
    }
}
