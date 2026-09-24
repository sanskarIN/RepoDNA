//! TODO/FIXME-style marker extraction from comments.

use std::sync::LazyLock;

use regex::Regex;
use repodna_core::model::quality::MarkerKind;
use serde::{Deserialize, Serialize};

use crate::scanner::ScannedLine;

/// A marker comment found in a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawMarker {
    /// Marker kind.
    pub kind: MarkerKind,
    /// Line (1-based).
    pub line: u32,
    /// Text following the marker, truncated to [`MAX_MARKER_TEXT`] characters.
    pub text: String,
}

/// Maximum characters of marker text kept.
pub const MAX_MARKER_TEXT: usize = 120;

/// Maximum markers recorded per file.
const MAX_MARKERS: usize = 500;

static MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(TODO|FIXME|HACK|XXX|BUG|DEPRECATED)\b[\s:(\-\]\[]*(.*)")
        .unwrap_or_else(|error| panic!("invalid marker pattern: {error}"))
});

fn kind(keyword: &str) -> Option<MarkerKind> {
    Some(match keyword {
        "TODO" => MarkerKind::Todo,
        "FIXME" => MarkerKind::Fixme,
        "HACK" => MarkerKind::Hack,
        "XXX" => MarkerKind::Xxx,
        "BUG" => MarkerKind::Bug,
        "DEPRECATED" => MarkerKind::Deprecated,
        _ => return None,
    })
}

/// Truncates `text` to at most `limit` characters, adding an ellipsis when shortened.
pub fn truncate_chars(text: &str, limit: usize) -> String {
    let mut chars = text.chars();
    let truncated: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{}…", truncated.trim_end())
    } else {
        truncated
    }
}

/// Extracts markers from the comment text of scanned lines. Markers in code or strings are
/// ignored because only comment text is searched.
pub fn extract_markers(lines: &[ScannedLine]) -> Vec<RawMarker> {
    let mut markers = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.comment.is_empty() {
            continue;
        }
        if let Some(captures) = MARKER.captures(&line.comment)
            && let Some(kind) = kind(&captures[1])
        {
            let text = captures.get(2).map_or("", |m| m.as_str()).trim();
            let text = text.trim_end_matches(['*', '/', '-']).trim();
            markers.push(RawMarker {
                kind,
                line: u32::try_from(index + 1).unwrap_or(u32::MAX),
                text: truncate_chars(text, MAX_MARKER_TEXT),
            });
            if markers.len() >= MAX_MARKERS {
                break;
            }
        }
    }
    markers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::LanguageRegistry;
    use crate::scanner::scan;

    fn markers(language: &str, source: &str) -> Vec<(MarkerKind, u32, String)> {
        let spec = LanguageRegistry::builtin().get(language).unwrap();
        extract_markers(&scan(source, &spec.syntax).lines)
            .into_iter()
            .map(|m| (m.kind, m.line, m.text))
            .collect()
    }

    #[test]
    fn finds_markers_only_in_comments() {
        let source = "// TODO: handle errors\nlet s = \"TODO in a string\";\n/* FIXME(sanskar) - flaky */\nfn debug() {} // DEBUG is not BUG-free\n# not a comment in rust\n";
        let found = markers("rust", source);
        assert_eq!(
            found,
            vec![
                (MarkerKind::Todo, 1, "handle errors".into()),
                (MarkerKind::Fixme, 3, "sanskar) - flaky".into()),
                (MarkerKind::Bug, 4, "free".into()),
            ]
        );
    }

    #[test]
    fn truncates_long_text_on_character_boundaries() {
        let long = "é".repeat(200);
        let truncated = truncate_chars(&long, 10);
        assert_eq!(truncated.chars().count(), 11);
        assert!(truncated.ends_with('…'));
        assert_eq!(truncate_chars("short", 10), "short");
    }

    #[test]
    fn python_markers() {
        let found = markers("python", "x = 1  # HACK: temporary workaround\n");
        assert_eq!(
            found,
            vec![(MarkerKind::Hack, 1, "temporary workaround".into())]
        );
    }
}
