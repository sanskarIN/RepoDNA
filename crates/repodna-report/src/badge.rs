//! README badges built from verifiable artifact values.

use std::fmt::Write;
use std::str::FromStr;

use repodna_core::model::artifact::RepositoryDna;

use crate::facts::facts;
use crate::fonts::text_width;
use crate::text::escape_html as esc;

/// Badge label background.
const LABEL_COLOR: &str = "#3d3d3a";
/// Informational value background.
const INFO_COLOR: &str = "#256abf";
/// Positive value background (dark enough for white text).
const GOOD_COLOR: &str = "#006300";
/// Neutral value background.
const NEUTRAL_COLOR: &str = "#6b6a65";

/// Which badge to render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeKind {
    /// The main languages.
    Languages,
    /// The inferred architecture style.
    Architecture,
    /// The activity level.
    Activity,
    /// Whether tests were detected.
    Tests,
    /// The DNA hash.
    Dna,
}

impl BadgeKind {
    /// Every badge.
    pub const ALL: [BadgeKind; 5] = [
        BadgeKind::Languages,
        BadgeKind::Architecture,
        BadgeKind::Activity,
        BadgeKind::Tests,
        BadgeKind::Dna,
    ];

    /// Identifier used on the command line and in file names.
    pub const fn id(self) -> &'static str {
        match self {
            BadgeKind::Languages => "languages",
            BadgeKind::Architecture => "architecture",
            BadgeKind::Activity => "activity",
            BadgeKind::Tests => "tests",
            BadgeKind::Dna => "dna",
        }
    }
}

impl FromStr for BadgeKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        BadgeKind::ALL
            .into_iter()
            .find(|kind| kind.id() == value.trim().to_ascii_lowercase())
            .ok_or_else(|| {
                let valid: Vec<&str> = BadgeKind::ALL.iter().map(|k| k.id()).collect();
                format!(
                    "unknown badge {value:?}; expected one of {}",
                    valid.join(", ")
                )
            })
    }
}

/// A badge's label, value, and value color.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Badge {
    /// Left text.
    pub label: String,
    /// Right text.
    pub value: String,
    /// Right background.
    pub color: &'static str,
}

/// The badge content for an artifact.
pub fn badge(dna: &RepositoryDna, kind: BadgeKind) -> Badge {
    let facts = facts(dna);
    let label = format!("RepoDNA {}", kind.id());
    let not_analyzed = || ("not analyzed".to_owned(), NEUTRAL_COLOR);
    let (value, color) = match kind {
        BadgeKind::Languages => {
            if facts.languages.is_empty() {
                ("none detected".to_owned(), NEUTRAL_COLOR)
            } else {
                let top: Vec<String> = facts
                    .languages
                    .iter()
                    .take(2)
                    .map(|l| format!("{} {:.0}%", l.name, l.share * 100.0))
                    .collect();
                (top.join(" · "), INFO_COLOR)
            }
        }
        BadgeKind::Architecture => facts
            .architecture
            .map_or_else(not_analyzed, |style| (style.to_lowercase(), INFO_COLOR)),
        BadgeKind::Activity => facts
            .activity
            .map_or_else(not_analyzed, |level| (level.to_lowercase(), INFO_COLOR)),
        BadgeKind::Tests => match facts.test_files {
            Some(0) => ("not detected".to_owned(), NEUTRAL_COLOR),
            Some(files) => (format!("detected ({files} files)"), GOOD_COLOR),
            None => not_analyzed(),
        },
        BadgeKind::Dna => {
            let short: String = facts.dna_hash.chars().take(14).collect();
            if short.is_empty() {
                not_analyzed()
            } else {
                (short, INFO_COLOR)
            }
        }
    };
    Badge {
        label,
        value,
        color,
    }
}

/// Renders a flat badge as SVG.
pub fn render_badge(badge: &Badge) -> String {
    let size = 11.0;
    let label_width = (text_width(&badge.label, size, false) + 12.0).round();
    let value_width = (text_width(&badge.value, size, false) + 12.0).round();
    let width = label_width + value_width;
    let title = format!("{}: {}", badge.label, badge.value);
    let mut out = String::new();
    let _ = write!(
        out,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="20" viewBox="0 0 {width} 20" role="img" aria-label="{t}"><title>{t}</title><clipPath id="r"><rect width="{width}" height="20" rx="3" fill="#fff"/></clipPath><g clip-path="url(#r)"><rect width="{label_width}" height="20" fill="{LABEL_COLOR}"/><rect x="{label_width}" width="{value_width}" height="20" fill="{color}"/></g><g fill="#fff" text-anchor="middle" font-family="DejaVu Sans,Verdana,Geneva,sans-serif" font-size="11"><text x="{lx}" y="14">{label}</text><text x="{vx}" y="14">{value}</text></g></svg>"##,
        t = esc(&title),
        color = badge.color,
        lx = label_width / 2.0,
        vx = label_width + value_width / 2.0,
        label = esc(&badge.label),
        value = esc(&badge.value)
    );
    out
}

/// A Markdown snippet embedding a badge image that links to the project.
pub fn markdown_snippet(kind: BadgeKind, image_path: &str) -> String {
    format!(
        "[![RepoDNA {}]({image_path})](https://github.com/sanskarIN/RepoDNA)",
        kind.id()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::model::SectionStatus;
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::metadata::AnalysisMetadata;
    use resvg::usvg::roxmltree::Document;

    #[test]
    fn describes_verifiable_values() {
        let mut dna =
            RepositoryDna::new(RepositoryIdentity::default(), AnalysisMetadata::default());
        assert_eq!(badge(&dna, BadgeKind::Architecture).value, "not analyzed");
        assert_eq!(badge(&dna, BadgeKind::Tests).value, "not analyzed");
        dna.tests.status = SectionStatus::Analyzed;
        dna.tests.test_files = 12;
        let tests = badge(&dna, BadgeKind::Tests);
        assert_eq!(tests.value, "detected (12 files)");
        assert_eq!(tests.label, "RepoDNA tests");
        dna.architecture.status = SectionStatus::Analyzed;
        dna.architecture.style = "Layered".into();
        assert_eq!(badge(&dna, BadgeKind::Architecture).value, "layered");
    }

    #[test]
    fn renders_valid_badges() {
        let svg = render_badge(&Badge {
            label: "RepoDNA tests".into(),
            value: "detected <12>".into(),
            color: GOOD_COLOR,
        });
        let document = Document::parse(&svg).unwrap();
        let root = document.root_element();
        let width: f64 = root.attribute("width").unwrap().parse().unwrap();
        assert!(width > 100.0 && width < 250.0, "{width}");
        assert!(svg.contains("detected &lt;12&gt;"));
        assert_eq!("dna".parse::<BadgeKind>().unwrap(), BadgeKind::Dna);
        assert!("nope".parse::<BadgeKind>().is_err());
        assert_eq!(
            markdown_snippet(BadgeKind::Tests, "badges/tests.svg"),
            "[![RepoDNA tests](badges/tests.svg)](https://github.com/sanskarIN/RepoDNA)"
        );
    }
}
