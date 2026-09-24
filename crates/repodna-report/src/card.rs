//! The Project DNA card: a shareable 1200 × 630 image of a repository's headline facts.
//!
//! The card's DNA visual is a double helix whose rungs are colored by language, in
//! proportion to each language's share of first-party code. Every value on the card comes
//! from the artifact; values whose analysis did not run are shown as "—".
//!
//! The SVG uses presentation attributes only (no stylesheet), so it renders identically
//! in browsers and in the bundled PNG renderer.

use std::f64::consts::PI;
use std::fmt::Write;

use repodna_core::model::artifact::RepositoryDna;

use crate::facts::{Facts, LanguageShare, facts, folded_languages};
use crate::fonts::{FONT_STACK, fit, text_width};
use crate::palette::{DARK, LIGHT, Palette};
use crate::text::{escape_html as esc, thousands};

/// Card width in pixels.
pub const WIDTH: f64 = 1200.0;
/// Card height in pixels.
pub const HEIGHT: f64 = 630.0;

/// Card appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CardOptions {
    /// Use the dark palette.
    pub dark: bool,
    /// Show "RepoDNA · Made by the Sanskar" in the footer.
    pub branding: bool,
}

fn c(value: f64) -> String {
    format!("{:.1}", value)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

/// Text appearance.
#[derive(Debug, Clone, Copy)]
struct Pen<'a> {
    size: f64,
    bold: bool,
    fill: &'a str,
    anchor: &'static str,
}

impl<'a> Pen<'a> {
    fn new(size: f64, bold: bool, fill: &'a str) -> Self {
        Self {
            size,
            bold,
            fill,
            anchor: "start",
        }
    }

    fn end(self) -> Self {
        Self {
            anchor: "end",
            ..self
        }
    }
}

fn text(out: &mut String, x: f64, y: f64, pen: Pen<'_>, content: &str) {
    let _ = write!(
        out,
        r#"<text x="{}" y="{}" font-size="{}" font-weight="{}" fill="{}" text-anchor="{}">{}</text>"#,
        c(x),
        c(y),
        pen.size,
        if pen.bold { "bold" } else { "normal" },
        pen.fill,
        pen.anchor,
        esc(content)
    );
}

/// Allocates `count` rungs to languages in proportion to their shares (largest remainder).
fn allocate(shares: &[LanguageShare], count: usize) -> Vec<Option<usize>> {
    let total: f64 = shares.iter().map(|s| s.share).sum();
    if total <= 0.0 {
        return vec![None; count];
    }
    let exact: Vec<f64> = shares
        .iter()
        .map(|s| s.share / total * count as f64)
        .collect();
    let mut counts: Vec<usize> = exact.iter().map(|e| e.floor() as usize).collect();
    let mut remaining = count.saturating_sub(counts.iter().sum());
    let mut order: Vec<usize> = (0..shares.len()).collect();
    order.sort_by(|&a, &b| {
        (exact[b] - exact[b].floor())
            .total_cmp(&(exact[a] - exact[a].floor()))
            .then(a.cmp(&b))
    });
    for index in order {
        if remaining == 0 {
            break;
        }
        counts[index] += 1;
        remaining -= 1;
    }
    let mut rungs = Vec::with_capacity(count);
    for (share, n) in shares.iter().zip(counts) {
        rungs.extend(std::iter::repeat_n(share.slot, n));
    }
    rungs.resize(count, None);
    rungs
}

fn helix(out: &mut String, palette: &Palette, languages: &[LanguageShare]) {
    let (left, right, center, amplitude, wavelength) = (64.0, 604.0, 330.0, 58.0, 180.0);
    let k = 2.0 * PI / wavelength;
    let strand = |phase: f64| -> String {
        let mut points = Vec::new();
        let mut x = left;
        while x <= right + 0.01 {
            points.push(format!(
                "{} {}",
                c(x),
                c(center + amplitude * (k * (x - left) + phase).sin())
            ));
            x += 6.0;
        }
        format!("M{}", points.join("L"))
    };
    let _ = write!(
        out,
        r#"<path d="{}" fill="none" stroke="{}" stroke-width="4" stroke-linecap="round"/>"#,
        strand(PI),
        palette.axis
    );
    let rung_count = 27;
    let colors = allocate(&folded_languages(languages), rung_count);
    for (index, slot) in colors.iter().enumerate() {
        let x = left + 10.0 + index as f64 * 20.0;
        let a = center + amplitude * (k * (x - left)).sin();
        let b = center + amplitude * (k * (x - left) + PI).sin();
        let (top, bottom) = (a.min(b) + 7.0, a.max(b) - 7.0);
        if bottom - top < 6.0 {
            continue;
        }
        let color = slot.map_or(palette.other, |slot| {
            palette.series[slot % palette.series.len()]
        });
        let _ = write!(
            out,
            r#"<line x1="{x}" y1="{}" x2="{x}" y2="{}" stroke="{color}" stroke-width="6" stroke-linecap="round"/>"#,
            c(top),
            c(bottom),
            x = c(x)
        );
    }
    let _ = write!(
        out,
        r#"<path d="{}" fill="none" stroke="{}" stroke-width="4" stroke-linecap="round"/>"#,
        strand(0.0),
        palette.ink_secondary
    );
}

fn value_or_dash<T>(value: Option<T>, format: impl Fn(T) -> String) -> String {
    value.map_or_else(|| "—".to_owned(), format)
}

fn years(days: i64) -> String {
    match days {
        i64::MIN..=0 => "< 1 day".to_owned(),
        1 => "1 day".to_owned(),
        2..=59 => format!("{days} days"),
        60..=729 => format!("{:.0} months", days as f64 / 30.44),
        _ => format!("{:.1} yrs", days as f64 / 365.25),
    }
}

/// Renders the card for `facts`.
pub fn render_facts(facts: &Facts, options: CardOptions) -> String {
    let palette = if options.dark { &DARK } else { &LIGHT };
    let mut out = String::new();
    let description = format!(
        "{}: {} files, {} code lines, {} commits, {} contributors.",
        facts.name,
        thousands(facts.files),
        thousands(facts.code_lines),
        value_or_dash(facts.commits, thousands),
        value_or_dash(facts.contributors, thousands),
    );
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" role="img" aria-labelledby="card-title card-desc" font-family="{font}"><!-- Generated by RepoDNA --><title id="card-title">Project DNA card for {name}</title><desc id="card-desc">{desc}</desc>"#,
        w = WIDTH,
        h = HEIGHT,
        font = esc(FONT_STACK),
        name = esc(&facts.name),
        desc = esc(&description)
    );
    let _ = write!(
        out,
        r#"<rect x="1" y="1" width="{}" height="{}" rx="28" fill="{}" stroke="{}" stroke-width="2"/>"#,
        WIDTH - 2.0,
        HEIGHT - 2.0,
        palette.surface,
        palette.grid
    );
    // Header.
    text(
        &mut out,
        64.0,
        84.0,
        Pen::new(30.0, true, palette.ink),
        "RepoDNA",
    );
    text(
        &mut out,
        64.0 + text_width("RepoDNA", 30.0, true) + 14.0,
        84.0,
        Pen::new(20.0, false, palette.muted),
        "Project DNA",
    );
    if !facts.dna_hash.is_empty() {
        let short: String = facts.dna_hash.chars().take(14).collect();
        text(
            &mut out,
            1136.0,
            84.0,
            Pen::new(16.0, false, palette.muted).end(),
            &format!("DNA {short}"),
        );
    }
    // Identity.
    text(
        &mut out,
        64.0,
        170.0,
        Pen::new(48.0, true, palette.ink),
        &fit(&facts.name, 48.0, true, 540.0),
    );
    let subline = facts
        .description
        .clone()
        .or_else(|| facts.location.clone())
        .or_else(|| facts.owner.clone())
        .unwrap_or_default();
    if !subline.is_empty() {
        text(
            &mut out,
            64.0,
            208.0,
            Pen::new(20.0, false, palette.ink_secondary),
            &fit(&subline, 20.0, false, 540.0),
        );
    }
    // DNA visual and language legend.
    helix(&mut out, palette, &facts.languages);
    for (index, language) in facts.languages.iter().take(4).enumerate() {
        let x = if index % 2 == 0 { 64.0 } else { 334.0 };
        let y = 450.0 + (index / 2) as f64 * 34.0;
        let color = language.slot.map_or(palette.other, |slot| {
            palette.series[slot % palette.series.len()]
        });
        let _ = write!(
            out,
            r#"<circle cx="{}" cy="{}" r="7" fill="{color}"/>"#,
            c(x + 7.0),
            c(y - 6.0)
        );
        let label = format!("{} {:.0}%", language.name, language.share * 100.0);
        text(
            &mut out,
            x + 22.0,
            y,
            Pen::new(18.0, false, palette.ink),
            &fit(&label, 18.0, false, 240.0),
        );
    }
    if facts.languages.is_empty() {
        text(
            &mut out,
            64.0,
            450.0,
            Pen::new(18.0, false, palette.muted),
            "No first-party code languages detected",
        );
    }
    // Statistics.
    let stats = [
        ("Files", thousands(facts.files)),
        ("Code lines", thousands(facts.code_lines)),
        ("Commits", value_or_dash(facts.commits, thousands)),
        ("Contributors", value_or_dash(facts.contributors, thousands)),
        ("Age", value_or_dash(facts.age_days, years)),
        ("Dependencies", value_or_dash(facts.dependencies, thousands)),
    ];
    for (index, (label, value)) in stats.iter().enumerate() {
        let x = if index % 2 == 0 { 660.0 } else { 900.0 };
        let y = 150.0 + (index / 2) as f64 * 96.0;
        text(&mut out, x, y, Pen::new(17.0, false, palette.muted), label);
        let fill = if value == "—" {
            palette.muted
        } else {
            palette.ink
        };
        text(
            &mut out,
            x,
            y + 42.0,
            Pen::new(36.0, true, fill),
            &fit(value, 36.0, true, 230.0),
        );
    }
    let signals = [
        ("Architecture", facts.architecture.clone()),
        ("Activity", facts.activity.clone()),
        (
            "Tests",
            facts.test_files.map(|files| {
                if files == 0 {
                    "Not detected".to_owned()
                } else {
                    format!("Detected ({} files)", thousands(files))
                }
            }),
        ),
    ];
    for (index, (label, value)) in signals.iter().enumerate() {
        let y = 460.0 + index as f64 * 34.0;
        text(
            &mut out,
            660.0,
            y,
            Pen::new(17.0, false, palette.muted),
            label,
        );
        let (content, fill) = match value {
            Some(value) => (value.as_str(), palette.ink),
            None => ("Not analyzed", palette.muted),
        };
        text(
            &mut out,
            820.0,
            y,
            Pen::new(19.0, true, fill),
            &fit(content, 19.0, true, 316.0),
        );
    }
    // Footer.
    let _ = write!(
        out,
        r#"<line x1="64" y1="560" x2="1136" y2="560" stroke="{}" stroke-width="1"/>"#,
        palette.grid
    );
    let mut footer: Vec<String> = Vec::new();
    if let Some(location) = &facts.location {
        footer.push(location.clone());
    }
    if let Some(revision) = &facts.revision {
        footer.push(format!("revision {revision}"));
    }
    footer.push(facts.generated.date_string());
    let branding = "RepoDNA · Made by the Sanskar";
    let footer_room = if options.branding {
        1072.0 - text_width(branding, 16.0, false) - 24.0
    } else {
        1072.0
    };
    text(
        &mut out,
        64.0,
        597.0,
        Pen::new(16.0, false, palette.muted),
        &fit(&footer.join(" · "), 16.0, false, footer_room),
    );
    if options.branding {
        text(
            &mut out,
            1136.0,
            597.0,
            Pen::new(16.0, false, palette.muted).end(),
            branding,
        );
    }
    out.push_str("</svg>");
    out
}

/// Renders the card for an artifact.
pub fn render(dna: &RepositoryDna, options: CardOptions) -> String {
    render_facts(&facts(dna), options)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::facts::LanguageShare;
    use repodna_core::time::Timestamp;
    use resvg::usvg::roxmltree::Document;

    pub(crate) fn sample() -> Facts {
        Facts {
            name: "widget".into(),
            owner: Some("acme".into()),
            description: Some("A small widget library & tools".into()),
            languages: vec![
                LanguageShare {
                    id: "rust".into(),
                    name: "Rust".into(),
                    share: 0.62,
                    slot: Some(0),
                },
                LanguageShare {
                    id: "typescript".into(),
                    name: "TypeScript".into(),
                    share: 0.3,
                    slot: Some(1),
                },
                LanguageShare {
                    id: "python".into(),
                    name: "Python".into(),
                    share: 0.08,
                    slot: Some(2),
                },
            ],
            files: 1842,
            code_lines: 120_400,
            commits: Some(3271),
            contributors: Some(14),
            age_days: Some(2630),
            dependencies: None,
            architecture: Some("Modular".into()),
            activity: Some("Active".into()),
            test_files: Some(120),
            dna_hash: "rdna1-0123456789abcdef0123456789abcdef".into(),
            generated: Timestamp::from_ymd(2026, 9, 24).unwrap(),
            revision: Some("abc1234".into()),
            location: Some("github.com/acme/widget".into()),
        }
    }

    #[test]
    fn allocates_rungs_proportionally() {
        let shares = sample().languages;
        let rungs = allocate(&shares, 27);
        assert_eq!(rungs.len(), 27);
        let count = |slot: usize| rungs.iter().filter(|r| **r == Some(slot)).count();
        assert_eq!((count(0), count(1), count(2)), (17, 8, 2));
        assert_eq!(allocate(&[], 5), vec![None; 5]);
    }

    #[test]
    fn renders_a_valid_card() {
        let svg = render_facts(
            &sample(),
            CardOptions {
                dark: false,
                branding: true,
            },
        );
        assert!(svg.contains(crate::GENERATED_MARKER));
        let document = Document::parse(&svg).unwrap();
        let texts: Vec<String> = document
            .descendants()
            .filter(|n| n.has_tag_name("text"))
            .map(|n| n.text().unwrap_or_default().to_owned())
            .collect();
        for expected in [
            "RepoDNA",
            "widget",
            "1,842",
            "3,271",
            "7.2 yrs",
            "Modular",
            "Detected (120 files)",
            "RepoDNA · Made by the Sanskar",
        ] {
            assert!(
                texts.iter().any(|t| t == expected),
                "missing {expected}: {texts:?}"
            );
        }
        assert!(
            texts.iter().any(|t| t == "—"),
            "dependencies should show a dash"
        );
        assert!(svg.contains("A small widget library &amp; tools"));
        let plain = render_facts(
            &sample(),
            CardOptions {
                dark: true,
                branding: false,
            },
        );
        assert!(!plain.contains("Made by the Sanskar"));
        assert!(plain.contains(DARK.surface));
    }
}
