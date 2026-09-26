//! Colors for charts and cards.
//!
//! Categorical slots are assigned in a fixed order and never cycled; past eight entities,
//! the tail folds into "Other". The categorical order and steps were validated for color
//! vision deficiencies on both surfaces (adjacent-pair ΔE ≥ 8, normal-vision ΔE ≥ 15);
//! three light-mode slots sit below 3:1 contrast, so every chart carries visible labels and
//! a table view. Status colors are reserved for severity and always appear with a label.

/// Colors for one surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Chart surface.
    pub surface: &'static str,
    /// Page background.
    pub page: &'static str,
    /// Primary text.
    pub ink: &'static str,
    /// Secondary text.
    pub ink_secondary: &'static str,
    /// Muted text (axes and labels).
    pub muted: &'static str,
    /// Gridlines.
    pub grid: &'static str,
    /// Baselines and axes.
    pub axis: &'static str,
    /// Categorical series, in fixed order.
    pub series: [&'static str; 8],
    /// The folded "Other" category.
    pub other: &'static str,
    /// Sequential magnitude ramp from "none" (index 0) to "most" (index 6).
    pub sequential: [&'static str; 7],
}

/// The light palette.
pub const LIGHT: Palette = Palette {
    surface: "#fcfcfb",
    page: "#f9f9f7",
    ink: "#0b0b0b",
    ink_secondary: "#52514e",
    muted: "#898781",
    grid: "#e1e0d9",
    axis: "#c3c2b7",
    series: [
        "#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#4a3aa7", "#e34948",
    ],
    other: "#b4b2a9",
    sequential: [
        "#f0efec", "#cde2fb", "#9ec5f4", "#6da7ec", "#3987e5", "#256abf", "#104281",
    ],
};

/// The dark palette: the same hues stepped for the dark surface.
pub const DARK: Palette = Palette {
    surface: "#1a1a19",
    page: "#0d0d0d",
    ink: "#ffffff",
    ink_secondary: "#c3c2b7",
    muted: "#898781",
    grid: "#2c2c2a",
    axis: "#383835",
    series: [
        "#3987e5", "#d95926", "#199e70", "#c98500", "#d55181", "#008300", "#9085e9", "#e66767",
    ],
    other: "#5f5e5a",
    sequential: [
        "#262625", "#0d366b", "#184f95", "#256abf", "#3987e5", "#6da7ec", "#b7d3f6",
    ],
};

/// Severity colors (fixed on both surfaces), always shown with an icon and a label.
pub mod status {
    /// Critical.
    pub const CRITICAL: &str = "#d03b3b";
    /// Serious (warning severity).
    pub const SERIOUS: &str = "#ec835a";
    /// Warning (attention severity).
    pub const WARNING: &str = "#fab219";
    /// Good.
    pub const GOOD: &str = "#0ca30c";
    /// Neutral (informational).
    pub const NEUTRAL: &str = "#898781";
}

/// Number of categorical slots.
pub const SLOTS: usize = 8;

/// CSS custom properties for a palette, for the HTML report.
pub fn css_variables(palette: &Palette) -> String {
    let mut css = format!(
        "--surface:{};--page:{};--ink:{};--ink-2:{};--muted:{};--grid:{};--axis:{};--other:{};",
        palette.surface,
        palette.page,
        palette.ink,
        palette.ink_secondary,
        palette.muted,
        palette.grid,
        palette.axis,
        palette.other
    );
    for (index, color) in palette.series.iter().enumerate() {
        css.push_str(&format!("--series-{}:{color};", index + 1));
    }
    for (index, color) in palette.sequential.iter().enumerate() {
        css.push_str(&format!("--seq-{index}:{color};"));
    }
    css
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_every_role_as_a_variable() {
        let css = css_variables(&LIGHT);
        assert!(css.contains("--series-1:#2a78d6;"));
        assert!(css.contains("--series-8:#e34948;"));
        assert!(css.contains("--seq-6:#104281;"));
        assert_eq!(LIGHT.series.len(), SLOTS);
        assert_ne!(LIGHT.surface, DARK.surface);
    }
}
