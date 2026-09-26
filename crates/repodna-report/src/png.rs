//! SVG to PNG rendering with the bundled fonts only.

use std::sync::Arc;

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{self, fontdb};

use crate::ReportError;
use crate::fonts::{FAMILY, SANS, SANS_BOLD};

/// Largest image dimension produced, a guard against runaway memory use.
const MAX_DIMENSION: f32 = 8192.0;

fn font_database() -> fontdb::Database {
    let mut database = fontdb::Database::new();
    database.load_font_data(SANS.to_vec());
    database.load_font_data(SANS_BOLD.to_vec());
    database.set_sans_serif_family(FAMILY);
    database
}

/// Renders SVG markup to PNG bytes at `scale` (1.0 = the SVG's own size).
pub fn render_png(svg: &str, scale: f32) -> Result<Vec<u8>, ReportError> {
    let options = usvg::Options {
        font_family: FAMILY.to_owned(),
        fontdb: Arc::new(font_database()),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(svg, &options)
        .map_err(|error| ReportError::Render(error.to_string()))?;
    let size = tree.size();
    let scale = scale.clamp(0.1, 8.0);
    let (width, height) = (size.width() * scale, size.height() * scale);
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(ReportError::Render(format!(
            "the image would be {width:.0} × {height:.0} pixels, above the {MAX_DIMENSION:.0} pixel limit"
        )));
    }
    let mut pixmap = Pixmap::new(width.ceil() as u32, height.ceil() as u32)
        .ok_or_else(|| ReportError::Render("the image has no area".to_owned()))?;
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .map_err(|error| ReportError::Render(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{CardOptions, render_facts};

    #[test]
    fn renders_cards_to_png_with_text() {
        let facts = crate::card::tests::sample();
        let svg = render_facts(
            &facts,
            CardOptions {
                dark: false,
                branding: true,
            },
        );
        let png = render_png(&svg, 1.0).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        // Width and height from the IHDR chunk.
        let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
        let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
        assert_eq!((width, height), (1200, 630));
        let double = render_png(&svg, 2.0).unwrap();
        assert_eq!(
            u32::from_be_bytes([double[16], double[17], double[18], double[19]]),
            2400
        );
        // Text is rendered with the bundled font: a card without text would be smaller.
        let blank = render_png(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630"><rect width="1200" height="630" fill="#fcfcfb"/></svg>"##,
            1.0,
        )
        .unwrap();
        assert!(png.len() > blank.len() * 4);
    }

    #[test]
    fn rejects_invalid_or_huge_images() {
        assert!(render_png("<svg", 1.0).is_err());
        let huge = r#"<svg xmlns="http://www.w3.org/2000/svg" width="5000" height="10"/>"#;
        assert!(render_png(huge, 4.0).is_err());
    }
}
