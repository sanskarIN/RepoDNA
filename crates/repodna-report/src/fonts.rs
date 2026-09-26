//! Bundled fonts and text measurement.
//!
//! PNG rendering never loads system fonts, so output is identical on every machine. The
//! same font's metrics are used to fit text in SVG layouts.

use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, MetadataProvider};

/// DejaVu Sans (see `assets/fonts/LICENSE-DejaVu.txt`).
pub const SANS: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");

/// DejaVu Sans Bold.
pub const SANS_BOLD: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf");

/// Family name of the bundled fonts.
pub const FAMILY: &str = "DejaVu Sans";

/// CSS font stack for SVG text: the bundled family first, then common system fonts.
pub const FONT_STACK: &str = "'DejaVu Sans', system-ui, -apple-system, 'Segoe UI', sans-serif";

/// Width of `text` in pixels at `size`, from the bundled font's advance widths.
pub fn text_width(text: &str, size: f64, bold: bool) -> f64 {
    let data = if bold { SANS_BOLD } else { SANS };
    let Ok(font) = FontRef::new(data) else {
        // The bundled fonts always parse; fall back to a typical average width.
        return text.chars().count() as f64 * size * 0.6;
    };
    let location = LocationRef::default();
    let units = f64::from(font.metrics(Size::unscaled(), location).units_per_em);
    if units <= 0.0 {
        return text.chars().count() as f64 * size * 0.6;
    }
    let charmap = font.charmap();
    let glyphs = font.glyph_metrics(Size::unscaled(), location);
    let advance_of = |character: char| {
        charmap
            .map(character)
            .and_then(|glyph| glyphs.advance_width(glyph))
    };
    let fallback = advance_of('n').unwrap_or(1200.0);
    let advance: f64 = text
        .chars()
        .map(|character| f64::from(advance_of(character).unwrap_or(fallback)))
        .sum();
    advance * size / units
}

/// Shortens `text` with an ellipsis so it is at most `max_width` pixels wide at `size`.
pub fn fit(text: &str, size: f64, bold: bool, max_width: f64) -> String {
    if text_width(text, size, bold) <= max_width {
        return text.to_owned();
    }
    let mut characters: Vec<char> = text.chars().collect();
    while !characters.is_empty() {
        characters.pop();
        let candidate: String = characters.iter().collect::<String>() + "…";
        if text_width(&candidate, size, bold) <= max_width {
            return candidate;
        }
    }
    String::new()
}

/// Wraps `text` at spaces into at most `max_lines` lines no wider than `max_width`; the last
/// line ends with an ellipsis when the text does not fit.
pub fn wrap(text: &str, size: f64, bold: bool, max_width: f64, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut words = text.split_whitespace().peekable();
    while words.peek().is_some() && lines.len() + 1 < max_lines {
        let mut line = String::new();
        while let Some(word) = words.peek() {
            let candidate = if line.is_empty() {
                (*word).to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && text_width(&candidate, size, bold) > max_width {
                break;
            }
            line = candidate;
            words.next();
        }
        lines.push(fit(&line, size, bold, max_width));
    }
    let rest: Vec<&str> = words.collect();
    if !rest.is_empty() && max_lines > 0 {
        lines.push(fit(&rest.join(" "), size, bold, max_width));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_bundled_font_text() {
        assert!(FontRef::new(SANS).is_ok());
        assert!(FontRef::new(SANS_BOLD).is_ok());
        let regular = text_width("RepoDNA", 12.0, false);
        let bold = text_width("RepoDNA", 12.0, true);
        assert!(regular > 40.0 && regular < 70.0, "{regular}");
        assert!(bold > regular);
        assert_eq!(text_width("", 12.0, false), 0.0);
        assert!((text_width("ab", 20.0, false) - 2.0 * text_width("ab", 10.0, false)).abs() < 1e-9);
    }

    #[test]
    fn wraps_text_into_lines() {
        let text = "Open-source repository intelligence and code archaeology.";
        let one = text_width(text, 20.0, false);
        let lines = wrap(text, 20.0, false, one * 0.7, 2);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines.join(" "), text);
        let lines = wrap(text, 20.0, false, one * 0.4, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with('…'), "{lines:?}");
        assert_eq!(wrap(text, 20.0, false, one * 2.0, 2), vec![text.to_owned()]);
        assert!(wrap("", 20.0, false, 100.0, 2).is_empty());
    }

    #[test]
    fn fits_text_with_an_ellipsis() {
        assert_eq!(fit("short", 12.0, false, 500.0), "short");
        let fitted = fit(
            "a-very-long-module-name-that-does-not-fit",
            12.0,
            false,
            80.0,
        );
        assert!(fitted.ends_with('…'));
        assert!(text_width(&fitted, 12.0, false) <= 80.0);
        assert_eq!(fit("abc", 12.0, false, 1.0), "");
    }
}
