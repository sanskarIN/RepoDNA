//! SVG charts for the HTML report.
//!
//! Charts use semantic classes (`viz-s1`, `viz-grid`, `viz-label`, …) that the report's
//! theme styles, so one chart serves the light and dark themes. Marks follow fixed specs:
//! bars at most 24 px thick with a 4 px rounded data end, 2 px lines, markers of at least
//! 8 px with a surface ring, a 2 px surface gap between touching fills, and hairline grids.
//! Every mark carries a `<title>` tooltip, and the report renders a table next to every
//! chart, so no value is available only through color or hover.

use std::f64::consts::PI;

use crate::fonts::text_width;
use crate::text::{escape_html as esc, number, thousands};

/// Label size in pixels.
const LABEL: f64 = 12.0;
/// Small label size in pixels.
const SMALL: f64 = 11.0;
/// Maximum bar thickness.
const MAX_BAR: f64 = 24.0;
/// Rounded data-end radius.
const RADIUS: f64 = 4.0;
/// Gap between touching fills.
const GAP: f64 = 2.0;

/// Formats a coordinate compactly.
fn c(value: f64) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.1}")
    }
}

/// An SVG document under construction.
struct Svg {
    width: f64,
    height: f64,
    label: String,
    body: String,
}

impl Svg {
    fn new(width: f64, height: f64, label: &str) -> Self {
        Self {
            width,
            height,
            label: label.to_owned(),
            body: String::new(),
        }
    }

    fn tip(tip: &str) -> String {
        if tip.is_empty() {
            String::new()
        } else {
            format!("<title>{}</title>", esc(tip))
        }
    }

    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, class: &str, tip: &str) {
        self.body.push_str(&format!(
            r#"<rect x="{}" y="{}" width="{}" height="{}" class="{class}">{}</rect>"#,
            c(x),
            c(y),
            c(w.max(0.0)),
            c(h.max(0.0)),
            Self::tip(tip)
        ));
    }

    /// A column growing up from `baseline`, rounded at the top.
    fn column(&mut self, x: f64, baseline: f64, w: f64, h: f64, class: &str, tip: &str) {
        if h <= 0.0 || w <= 0.0 {
            return;
        }
        let r = RADIUS.min(w / 2.0).min(h);
        let top = baseline - h;
        self.body.push_str(&format!(
            r#"<path d="M{x0} {b}V{t1}Q{x0} {t} {x1} {t}H{x2}Q{x3} {t} {x3} {t1}V{b}Z" class="{class}">{tip}</path>"#,
            x0 = c(x),
            x1 = c(x + r),
            x2 = c(x + w - r),
            x3 = c(x + w),
            b = c(baseline),
            t = c(top),
            t1 = c(top + r),
            tip = Self::tip(tip)
        ));
    }

    /// A horizontal bar growing right from `x`, rounded at the right end.
    fn bar(&mut self, x: f64, y: f64, w: f64, h: f64, class: &str, tip: &str) {
        if h <= 0.0 || w <= 0.0 {
            return;
        }
        let r = RADIUS.min(h / 2.0).min(w);
        let right = x + w;
        self.body.push_str(&format!(
            r#"<path d="M{x0} {y0}H{r1}Q{r2} {y0} {r2} {y1}V{y2}Q{r2} {y3} {r1} {y3}H{x0}Z" class="{class}">{tip}</path>"#,
            x0 = c(x),
            r1 = c(right - r),
            r2 = c(right),
            y0 = c(y),
            y1 = c(y + r),
            y2 = c(y + h - r),
            y3 = c(y + h),
            tip = Self::tip(tip)
        ));
    }

    fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, class: &str) {
        self.body.push_str(&format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" class="{class}"/>"#,
            c(x1),
            c(y1),
            c(x2),
            c(y2)
        ));
    }

    fn text(&mut self, x: f64, y: f64, anchor: &str, class: &str, content: &str) {
        self.body.push_str(&format!(
            r#"<text x="{}" y="{}" text-anchor="{anchor}" class="{class}">{}</text>"#,
            c(x),
            c(y),
            esc(content)
        ));
    }

    fn dot(&mut self, x: f64, y: f64, class: &str, tip: &str) {
        self.body.push_str(&format!(
            r#"<circle cx="{}" cy="{}" r="4" class="{class} viz-ring">{}</circle>"#,
            c(x),
            c(y),
            Self::tip(tip)
        ));
    }

    fn raw(&mut self, markup: &str) {
        self.body.push_str(markup);
    }

    fn finish(self) -> String {
        format!(
            r#"<svg class="viz" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" role="img" aria-label="{label}"><title>{label}</title>{body}</svg>"#,
            w = c(self.width),
            h = c(self.height),
            label = esc(&self.label),
            body = self.body
        )
    }
}

/// A clean axis step and top value for data up to `max` with about `ticks` intervals.
fn nice_scale(max: f64, ticks: u32) -> (f64, f64) {
    if max.is_nan() || max <= 0.0 || !max.is_finite() {
        return (1.0, 1.0);
    }
    let raw = max / f64::from(ticks.max(1));
    let magnitude = 10f64.powf(raw.log10().floor());
    let normalized = raw / magnitude;
    let step = magnitude
        * if normalized <= 1.0 {
            1.0
        } else if normalized <= 2.0 {
            2.0
        } else if normalized <= 5.0 {
            5.0
        } else {
            10.0
        };
    (step, (max / step).ceil() * step)
}

fn tick_label(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        thousands(value.abs() as u64)
    } else {
        number(value)
    }
}

/// A language in the composition bar.
#[derive(Debug, Clone, PartialEq)]
pub struct Share {
    /// Display name.
    pub name: String,
    /// Share (0–1).
    pub share: f64,
    /// Categorical slot (0-based), or `None` for the folded "Other" entry.
    pub slot: Option<usize>,
}

fn slot_class(slot: Option<usize>) -> String {
    match slot {
        Some(slot) => format!("viz-s{}", slot % crate::palette::SLOTS + 1),
        None => "viz-other".to_owned(),
    }
}

/// A horizontal stacked bar of shares with a legend below it.
pub fn share_bar(entries: &[Share], label: &str) -> Option<String> {
    let entries: Vec<&Share> = entries.iter().filter(|e| e.share > 0.0).collect();
    if entries.is_empty() {
        return None;
    }
    let width = 720.0;
    let bar_height = 20.0;
    let total: f64 = entries.iter().map(|e| e.share).sum();
    // Legend layout: items flow in rows.
    let mut legend: Vec<(f64, f64, &Share, String)> = Vec::new();
    let (mut x, mut y) = (0.0, bar_height + 22.0);
    for entry in &entries {
        let text = format!("{} {:.1}%", entry.name, entry.share / total * 100.0);
        let item_width = 16.0 + text_width(&text, LABEL, false) + 18.0;
        if x > 0.0 && x + item_width > width {
            x = 0.0;
            y += 22.0;
        }
        legend.push((x, y, entry, text));
        x += item_width;
    }
    let mut svg = Svg::new(width, y + 10.0, label);
    let mut offset = 0.0;
    let count = entries.len();
    for (index, entry) in entries.iter().enumerate() {
        let segment = entry.share / total * width;
        let last = index + 1 == count;
        let drawn = if last {
            segment
        } else {
            (segment - GAP).max(1.0)
        };
        let tip = format!("{}: {:.1}%", entry.name, entry.share / total * 100.0);
        let class = slot_class(entry.slot);
        if last {
            svg.bar(offset, 0.0, drawn, bar_height, &class, &tip);
        } else {
            svg.rect(offset, 0.0, drawn, bar_height, &class, &tip);
        }
        offset += segment;
    }
    for (x, y, entry, text) in legend {
        svg.raw(&format!(
            r#"<rect x="{}" y="{}" width="10" height="10" rx="2" class="{}"/>"#,
            c(x),
            c(y - 9.0),
            slot_class(entry.slot)
        ));
        svg.text(x + 16.0, y, "start", "viz-label", &text);
    }
    Some(svg.finish())
}

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const WEEKDAY_NAMES: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// Commits by weekday and hour as a heatmap on the sequential ramp.
pub fn heatmap(grid: &[Vec<u32>], label: &str) -> Option<String> {
    let max = grid.iter().flatten().copied().max().unwrap_or(0);
    if max == 0 {
        return None;
    }
    let cell = 20.0;
    let pitch = cell + GAP;
    let left = 40.0;
    let top = 22.0;
    let width = left + 24.0 * pitch;
    let height = top + 7.0 * pitch + 40.0;
    let mut svg = Svg::new(width, height, label);
    for hour in (0..24).step_by(3) {
        svg.text(
            left + f64::from(hour) * pitch + cell / 2.0,
            14.0,
            "middle",
            "viz-muted",
            &format!("{hour:02}"),
        );
    }
    for (day, name) in WEEKDAYS.iter().enumerate() {
        let y = top + day as f64 * pitch;
        svg.text(left - 8.0, y + 14.0, "end", "viz-muted", name);
        for hour in 0..24usize {
            let count = grid
                .get(day)
                .and_then(|row| row.get(hour))
                .copied()
                .unwrap_or(0);
            let bucket = if count == 0 {
                0
            } else {
                ((f64::from(count) / f64::from(max) * 6.0).ceil() as u32).clamp(1, 6)
            };
            let tip = format!(
                "{} {hour:02}:00 – {} commit{}",
                WEEKDAY_NAMES[day],
                thousands(u64::from(count)),
                if count == 1 { "" } else { "s" }
            );
            svg.raw(&format!(
                r#"<rect x="{}" y="{}" width="{cell}" height="{cell}" rx="3" class="viz-q{bucket}">{}</rect>"#,
                c(left + hour as f64 * pitch),
                c(y),
                Svg::tip(&tip)
            ));
        }
    }
    let legend_y = top + 7.0 * pitch + 18.0;
    svg.text(left, legend_y + 11.0, "start", "viz-muted", "Fewer");
    let start = left + text_width("Fewer", LABEL, false) + 8.0;
    for bucket in 0..7 {
        svg.raw(&format!(
            r#"<rect x="{}" y="{}" width="14" height="14" rx="3" class="viz-q{bucket}"/>"#,
            c(start + f64::from(bucket) * 17.0),
            c(legend_y)
        ));
    }
    svg.text(
        start + 7.0 * 17.0 + 6.0,
        legend_y + 11.0,
        "start",
        "viz-muted",
        "More",
    );
    Some(svg.finish())
}

/// A single-series column chart.
pub fn columns(points: &[(String, f64)], unit: &str, label: &str) -> Option<String> {
    if points.is_empty() || points.iter().all(|(_, v)| *v <= 0.0) {
        return None;
    }
    let max = points.iter().map(|(_, v)| *v).fold(0.0, f64::max);
    let (step, top_value) = nice_scale(max, 4);
    let ticks: Vec<f64> = (0..)
        .map(|i| f64::from(i) * step)
        .take_while(|v| *v <= top_value + step / 2.0)
        .collect();
    let axis_width = ticks
        .iter()
        .map(|t| text_width(&tick_label(*t), SMALL, false))
        .fold(0.0, f64::max)
        + 10.0;
    let width = 720.0;
    let height = 240.0;
    let (plot_top, plot_bottom) = (10.0, height - 30.0);
    let plot_left = axis_width;
    let plot_width = width - plot_left - 4.0;
    let scale = |v: f64| plot_bottom - v / top_value * (plot_bottom - plot_top);
    let mut svg = Svg::new(width, height, label);
    for tick in &ticks {
        let y = scale(*tick);
        svg.line(plot_left, y, width - 4.0, y, "viz-grid");
        svg.text(
            plot_left - 6.0,
            y + 4.0,
            "end",
            "viz-muted",
            &tick_label(*tick),
        );
    }
    let band = plot_width / points.len() as f64;
    let bar = (band * 0.7).clamp(1.0, MAX_BAR);
    let every = ((points.len() as f64 / 8.0).ceil() as usize).max(1);
    for (index, (name, value)) in points.iter().enumerate() {
        let x = plot_left + band * index as f64 + (band - bar) / 2.0;
        let tip = format!("{name}: {} {unit}", number(*value));
        svg.column(
            x,
            plot_bottom,
            bar,
            plot_bottom - scale(*value),
            "viz-s1",
            &tip,
        );
        if index % every == 0 {
            svg.text(x + bar / 2.0, height - 10.0, "middle", "viz-muted", name);
        }
    }
    svg.line(plot_left, plot_bottom, width - 4.0, plot_bottom, "viz-axis");
    Some(svg.finish())
}

/// A single-series line over time, with a wash under it and the latest value labeled.
pub fn line(points: &[(i64, f64)], unit: &str, label: &str) -> Option<String> {
    if points.len() < 2 {
        return None;
    }
    let (first, last) = (points[0].0, points[points.len() - 1].0);
    if last <= first {
        return None;
    }
    let max = points.iter().map(|(_, v)| *v).fold(0.0, f64::max);
    let (step, top_value) = nice_scale(max, 4);
    let ticks: Vec<f64> = (0..)
        .map(|i| f64::from(i) * step)
        .take_while(|v| *v <= top_value + step / 2.0)
        .collect();
    let axis_width = ticks
        .iter()
        .map(|t| text_width(&tick_label(*t), SMALL, false))
        .fold(0.0, f64::max)
        + 10.0;
    let width = 720.0;
    let height = 240.0;
    let (plot_top, plot_bottom) = (12.0, height - 30.0);
    let plot_left = axis_width;
    let end_label = format!("{} {unit}", number(points[points.len() - 1].1));
    let plot_right = width - text_width(&end_label, LABEL, true) - 14.0;
    let x_of =
        |t: i64| plot_left + (t - first) as f64 / (last - first) as f64 * (plot_right - plot_left);
    let y_of = |v: f64| plot_bottom - v / top_value * (plot_bottom - plot_top);
    let mut svg = Svg::new(width, height, label);
    for tick in &ticks {
        let y = y_of(*tick);
        svg.line(plot_left, y, plot_right, y, "viz-grid");
        svg.text(
            plot_left - 6.0,
            y + 4.0,
            "end",
            "viz-muted",
            &tick_label(*tick),
        );
    }
    let path: Vec<String> = points
        .iter()
        .map(|(t, v)| format!("{} {}", c(x_of(*t)), c(y_of(*v))))
        .collect();
    svg.raw(&format!(
        r#"<path d="M{} {}L{}L{} {}Z" class="viz-wash"/>"#,
        c(x_of(first)),
        c(plot_bottom),
        path.join("L"),
        c(x_of(last)),
        c(plot_bottom)
    ));
    svg.raw(&format!(
        r#"<polyline points="{}" class="viz-line"/>"#,
        path.join(" ")
    ));
    svg.line(plot_left, plot_bottom, plot_right, plot_bottom, "viz-axis");
    // Time ticks: up to five evenly spaced dates.
    for index in 0..5 {
        let t = first + (last - first) * index / 4;
        let date = repodna_core::time::Timestamp::from_unix(t).month_key();
        svg.text(x_of(t), height - 10.0, "middle", "viz-muted", &date);
    }
    for (t, v) in points {
        let date = repodna_core::time::Timestamp::from_unix(*t).date_string();
        svg.raw(&format!(
            r#"<circle cx="{}" cy="{}" r="10" class="viz-hit">{}</circle>"#,
            c(x_of(*t)),
            c(y_of(*v)),
            Svg::tip(&format!("{date}: {} {unit}", number(*v)))
        ));
    }
    let (t, v) = points[points.len() - 1];
    svg.dot(x_of(t), y_of(v), "viz-dot", &format!("Latest: {end_label}"));
    svg.text(
        x_of(t) + 10.0,
        y_of(v) + 4.0,
        "start",
        "viz-value",
        &end_label,
    );
    Some(svg.finish())
}

/// One axis of the radar.
#[derive(Debug, Clone, PartialEq)]
pub struct RadarAxis {
    /// Axis label.
    pub label: String,
    /// Value (0–1).
    pub value: f64,
    /// `false` when the measurement is unavailable.
    pub available: bool,
    /// Tooltip text.
    pub tip: String,
}

/// The DNA fingerprint as a radar: one closed shape over all dimensions.
pub fn radar(axes: &[RadarAxis], label: &str) -> Option<String> {
    if axes.len() < 3 {
        return None;
    }
    let width = 560.0;
    let height = 380.0;
    let (cx, cy, radius) = (width / 2.0, height / 2.0, 130.0);
    let count = axes.len() as f64;
    let point = |index: usize, value: f64| {
        let angle = -PI / 2.0 + 2.0 * PI * index as f64 / count;
        (
            cx + radius * value * angle.cos(),
            cy + radius * value * angle.sin(),
        )
    };
    let mut svg = Svg::new(width, height, label);
    for ring in [0.25, 0.5, 0.75, 1.0] {
        let ring_points: Vec<String> = (0..axes.len())
            .map(|i| {
                let (x, y) = point(i, ring);
                format!("{} {}", c(x), c(y))
            })
            .collect();
        svg.raw(&format!(
            r#"<polygon points="{}" class="viz-grid-shape"/>"#,
            ring_points.join(" ")
        ));
    }
    for (index, axis) in axes.iter().enumerate() {
        let (x, y) = point(index, 1.0);
        svg.line(cx, cy, x, y, "viz-grid");
        let (lx, ly) = point(index, 1.16);
        let anchor = if (lx - cx).abs() < 8.0 {
            "middle"
        } else if lx > cx {
            "start"
        } else {
            "end"
        };
        let text = if axis.available {
            axis.label.clone()
        } else {
            format!("{} (n/a)", axis.label)
        };
        svg.text(
            lx,
            ly + 4.0,
            anchor,
            if axis.available {
                "viz-label"
            } else {
                "viz-muted"
            },
            &text,
        );
    }
    let shape: Vec<String> = axes
        .iter()
        .enumerate()
        .map(|(i, axis)| {
            let (x, y) = point(i, axis.value.clamp(0.0, 1.0));
            format!("{} {}", c(x), c(y))
        })
        .collect();
    svg.raw(&format!(
        r#"<polygon points="{}" class="viz-wash viz-outline"/>"#,
        shape.join(" ")
    ));
    for (index, axis) in axes.iter().enumerate() {
        if axis.available {
            let (x, y) = point(index, axis.value.clamp(0.0, 1.0));
            svg.dot(x, y, "viz-dot", &axis.tip);
        }
    }
    Some(svg.finish())
}

/// Lays out `values` (sorted descending) as squarified rectangles inside the box.
fn squarify(values: &[f64], x: f64, y: f64, w: f64, h: f64) -> Vec<(f64, f64, f64, f64)> {
    let total: f64 = values.iter().sum();
    let mut rects = Vec::with_capacity(values.len());
    if total <= 0.0 || w <= 0.0 || h <= 0.0 {
        return rects;
    }
    let area_scale = w * h / total;
    let areas: Vec<f64> = values.iter().map(|v| v * area_scale).collect();
    let (mut x, mut y, mut w, mut h) = (x, y, w, h);
    let mut start = 0;
    while start < areas.len() {
        let side = w.min(h);
        let mut end = start + 1;
        let worst = |row: &[f64]| {
            let sum: f64 = row.iter().sum();
            let max = row.iter().copied().fold(0.0, f64::max);
            let min = row.iter().copied().fold(f64::INFINITY, f64::min);
            let s2 = sum * sum;
            ((side * side * max) / s2).max(s2 / (side * side * min))
        };
        while end < areas.len() && worst(&areas[start..=end]) <= worst(&areas[start..end]) {
            end += 1;
        }
        let row = &areas[start..end];
        let sum: f64 = row.iter().sum();
        if w >= h {
            let row_width = sum / h;
            let mut offset = y;
            for area in row {
                let item = area / row_width;
                rects.push((x, offset, row_width, item));
                offset += item;
            }
            x += row_width;
            w -= row_width;
        } else {
            let row_height = sum / w;
            let mut offset = x;
            for area in row {
                let item = area / row_height;
                rects.push((offset, y, item, row_height));
                offset += item;
            }
            y += row_height;
            h -= row_height;
        }
        start = end;
    }
    rects
}

/// Sizes of items (for example directories by code lines) as a squarified treemap.
pub fn treemap(items: &[(String, f64)], unit: &str, label: &str) -> Option<String> {
    let mut items: Vec<&(String, f64)> = items.iter().filter(|(_, v)| *v > 0.0).collect();
    if items.is_empty() {
        return None;
    }
    items.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let values: Vec<f64> = items.iter().map(|(_, v)| *v).collect();
    let (width, height) = (720.0, 320.0);
    let mut svg = Svg::new(width, height, label);
    for ((x, y, w, h), (name, value)) in squarify(&values, 0.0, 0.0, width, height)
        .into_iter()
        .zip(items)
    {
        let inset = GAP / 2.0;
        let tip = format!("{name}: {} {unit}", number(*value));
        svg.raw(&format!(
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3" class="viz-tile">{}</rect>"#,
            c(x + inset),
            c(y + inset),
            c((w - GAP).max(0.0)),
            c((h - GAP).max(0.0)),
            Svg::tip(&tip)
        ));
        let name_width = text_width(name, LABEL, true);
        if w - 12.0 >= name_width && h >= 24.0 {
            svg.text(x + 7.0, y + 18.0, "start", "viz-tile-label", name);
            let value_text = format!("{} {unit}", number(*value));
            if h >= 40.0 && w - 12.0 >= text_width(&value_text, SMALL, false) {
                svg.text(x + 7.0, y + 33.0, "start", "viz-tile-value", &value_text);
            }
        }
    }
    Some(svg.finish())
}

/// A module in the dependency diagram.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphNode {
    /// Identifier referenced by edges.
    pub id: String,
    /// Label.
    pub label: String,
    /// Layer (0 = modules without internal dependencies).
    pub layer: u32,
}

/// Module dependencies drawn in layers: dependents above, their dependencies below.
pub fn layered_graph(
    nodes: &[GraphNode],
    edges: &[(String, String, u32)],
    label: &str,
) -> Option<String> {
    if nodes.is_empty() {
        return None;
    }
    let max_layer = nodes.iter().map(|n| n.layer).max().unwrap_or(0);
    let width = 960.0;
    let row_height = 86.0;
    let node_height = 28.0;
    let height = f64::from(max_layer + 1) * row_height + 10.0;
    let mut positions: std::collections::HashMap<&str, (f64, f64, f64)> =
        std::collections::HashMap::new();
    let mut boxes = String::new();
    for layer in 0..=max_layer {
        let mut row: Vec<&GraphNode> = nodes.iter().filter(|n| n.layer == layer).collect();
        row.sort_by(|a, b| a.label.cmp(&b.label));
        if row.is_empty() {
            continue;
        }
        let slot = width / row.len() as f64;
        let box_width = (slot - 12.0).clamp(24.0, 170.0);
        let y = f64::from(max_layer - layer) * row_height + 12.0;
        for (index, node) in row.iter().enumerate() {
            let center = slot * index as f64 + slot / 2.0;
            positions.insert(node.id.as_str(), (center, y, box_width));
            let text = crate::fonts::fit(&node.label, LABEL, false, box_width - 12.0);
            boxes.push_str(&format!(
                r#"<g class="viz-node"><title>{}</title><rect x="{}" y="{}" width="{}" height="{node_height}" rx="6"/><text x="{}" y="{}" text-anchor="middle" class="viz-label">{}</text></g>"#,
                esc(&format!("{} (layer {layer})", node.label)),
                c(center - box_width / 2.0),
                c(y),
                c(box_width),
                c(center),
                c(y + 18.0),
                esc(&text)
            ));
        }
    }
    let mut svg = Svg::new(width, height, label);
    svg.raw(r#"<defs><marker id="viz-arrow" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L8 4L0 8Z" class="viz-arrowhead"/></marker></defs>"#);
    for (from, to, weight) in edges {
        let (Some(&(x1, y1, _)), Some(&(x2, y2, _))) =
            (positions.get(from.as_str()), positions.get(to.as_str()))
        else {
            continue;
        };
        let (start_y, end_y) = if y1 <= y2 {
            (y1 + node_height, y2)
        } else {
            (y1, y2 + node_height)
        };
        let middle = (start_y + end_y) / 2.0;
        svg.raw(&format!(
            r#"<path d="M{} {}C{} {} {} {} {} {}" class="viz-edge" marker-end="url(#viz-arrow)">{}</path>"#,
            c(x1),
            c(start_y),
            c(x1),
            c(middle),
            c(x2),
            c(middle),
            c(x2),
            c(end_y),
            Svg::tip(&format!("{from} → {to}: {weight} import{}", if *weight == 1 { "" } else { "s" }))
        ));
    }
    svg.raw(&boxes);
    Some(svg.finish())
}

/// A horizontal bar for each item, labeled on the left with the value at the bar's end.
pub fn bars(items: &[(String, f64)], unit: &str, label: &str) -> Option<String> {
    if items.is_empty() || items.iter().all(|(_, v)| *v <= 0.0) {
        return None;
    }
    let max = items.iter().map(|(_, v)| *v).fold(0.0, f64::max);
    let label_width = 200.0;
    let width = 720.0;
    let row = 28.0;
    let value_room = items
        .iter()
        .map(|(_, v)| text_width(&format!("{} {unit}", number(*v)), LABEL, false))
        .fold(0.0, f64::max)
        + 12.0;
    let plot_width = width - label_width - value_room;
    let mut svg = Svg::new(width, row * items.len() as f64 + 4.0, label);
    for (index, (name, value)) in items.iter().enumerate() {
        let y = index as f64 * row + 4.0;
        let text = crate::fonts::fit(name, LABEL, false, label_width - 12.0);
        svg.text(label_width - 10.0, y + 15.0, "end", "viz-label", &text);
        let length = (value / max * plot_width).max(if *value > 0.0 { 2.0 } else { 0.0 });
        let tip = format!("{name}: {} {unit}", number(*value));
        svg.bar(label_width, y + 2.0, length, 18.0, "viz-s1", &tip);
        svg.text(
            label_width + length + 6.0,
            y + 15.0,
            "start",
            "viz-value",
            &format!("{} {unit}", number(*value)),
        );
    }
    Some(svg.finish())
}

/// A period on the evolution timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct Period {
    /// Start (Unix seconds).
    pub start: i64,
    /// End (Unix seconds).
    pub end: i64,
    /// Label.
    pub label: String,
    /// Categorical slot for the period kind.
    pub slot: usize,
}

/// Epochs as bands and events as markers along one time axis.
pub fn timeline(
    periods: &[Period],
    events: &[(i64, String)],
    kinds: &[(usize, String)],
    label: &str,
) -> Option<String> {
    let start = periods
        .iter()
        .map(|p| p.start)
        .chain(events.iter().map(|e| e.0))
        .min()?;
    let end = periods
        .iter()
        .map(|p| p.end)
        .chain(events.iter().map(|e| e.0))
        .max()?;
    if end <= start {
        return None;
    }
    let width = 960.0;
    let (left, right) = (10.0, width - 10.0);
    let x_of = |t: i64| left + (t - start) as f64 / (end - start) as f64 * (right - left);
    let mut svg = Svg::new(width, 150.0, label);
    for period in periods {
        let x = x_of(period.start);
        let w = (x_of(period.end) - x - GAP).max(2.0);
        svg.rect(
            x,
            10.0,
            w,
            26.0,
            &format!("viz-s{}", period.slot % crate::palette::SLOTS + 1),
            &period.label,
        );
    }
    svg.line(left, 70.0, right, 70.0, "viz-axis");
    for (time, text) in events {
        svg.dot(x_of(*time), 70.0, "viz-dot", text);
    }
    for index in 0..5 {
        let t = start + (end - start) * index / 4;
        let anchor = match index {
            0 => "start",
            4 => "end",
            _ => "middle",
        };
        svg.text(
            x_of(t),
            96.0,
            anchor,
            "viz-muted",
            &repodna_core::time::Timestamp::from_unix(t).month_key(),
        );
    }
    let mut x = left;
    for (slot, name) in kinds {
        svg.raw(&format!(
            r#"<rect x="{}" y="116" width="10" height="10" rx="2" class="viz-s{}"/>"#,
            c(x),
            slot % crate::palette::SLOTS + 1
        ));
        svg.text(x + 16.0, 125.0, "start", "viz-label", name);
        x += 16.0 + text_width(name, LABEL, false) + 18.0;
    }
    Some(svg.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use resvg::usvg::roxmltree::Document;

    fn parse(svg: &str) -> Document<'_> {
        Document::parse(svg).unwrap_or_else(|error| panic!("invalid SVG: {error}\n{svg}"))
    }

    fn count(svg: &str, tag: &str) -> usize {
        parse(svg)
            .descendants()
            .filter(|node| node.has_tag_name(tag))
            .count()
    }

    #[test]
    fn scales_to_clean_ticks() {
        assert_eq!(nice_scale(87.0, 4), (50.0, 100.0));
        assert_eq!(nice_scale(8.0, 4), (2.0, 8.0));
        assert_eq!(nice_scale(0.0, 4), (1.0, 1.0));
        assert_eq!(nice_scale(1234.0, 4), (500.0, 1500.0));
        assert_eq!(tick_label(1500.0), "1,500");
        assert_eq!(tick_label(0.5), "0.5");
    }

    #[test]
    fn draws_share_bars_with_legends() {
        let entries = vec![
            Share {
                name: "Rust".into(),
                share: 0.7,
                slot: Some(0),
            },
            Share {
                name: "TypeScript <x>".into(),
                share: 0.2,
                slot: Some(1),
            },
            Share {
                name: "Other".into(),
                share: 0.1,
                slot: None,
            },
        ];
        let svg = share_bar(&entries, "Languages").unwrap();
        assert!(svg.contains("viz-s1") && svg.contains("viz-s2") && svg.contains("viz-other"));
        assert!(svg.contains("TypeScript &lt;x&gt; 20.0%"));
        assert_eq!(count(&svg, "title"), 4);
        assert!(share_bar(&[], "empty").is_none());
    }

    #[test]
    fn draws_heatmaps_on_the_sequential_ramp() {
        let mut grid = vec![vec![0u32; 24]; 7];
        grid[1][14] = 6;
        grid[4][9] = 1;
        let svg = heatmap(&grid, "Activity").unwrap();
        assert_eq!(count(&svg, "rect"), 7 * 24 + 7);
        assert!(svg.contains("Tuesday 14:00 – 6 commits"));
        assert!(svg.contains("Friday 09:00 – 1 commit<"));
        assert!(svg.contains("viz-q6") && svg.contains("viz-q1"));
        assert!(heatmap(&vec![vec![0u32; 24]; 7], "none").is_none());
    }

    #[test]
    fn draws_columns_lines_and_bars() {
        let points: Vec<(String, f64)> = (1..=12)
            .map(|m| (format!("2024-{m:02}"), f64::from(m)))
            .collect();
        let svg = columns(&points, "commits", "Commits per month").unwrap();
        assert_eq!(count(&svg, "path"), 12);
        assert!(svg.contains("2024-12: 12 commits"));
        let series: Vec<(i64, f64)> = (0..5).map(|i| (i * 86_400 * 30, (i * 10) as f64)).collect();
        let svg = line(&series, "files", "Growth").unwrap();
        assert!(svg.contains("Latest: 40 files"));
        assert!(line(&series[..1], "files", "Growth").is_none());
        let svg = bars(
            &[("src".into(), 3.0), ("docs".into(), 1.0)],
            "commits",
            "Areas",
        )
        .unwrap();
        assert_eq!(count(&svg, "path"), 2);
        assert!(columns(&[], "x", "none").is_none());
    }

    #[test]
    fn draws_radar_treemap_graph_and_timeline() {
        let axes: Vec<RadarAxis> = ["A", "B", "C", "D"]
            .iter()
            .enumerate()
            .map(|(i, label)| RadarAxis {
                label: (*label).into(),
                value: 0.25 * i as f64,
                available: i != 3,
                tip: (*label).to_owned(),
            })
            .collect();
        let svg = radar(&axes, "DNA").unwrap();
        assert_eq!(count(&svg, "circle"), 3);
        assert!(svg.contains("D (n/a)"));

        let items = vec![
            ("src".to_owned(), 600.0),
            ("docs".to_owned(), 300.0),
            ("tests".to_owned(), 100.0),
        ];
        let svg = treemap(&items, "lines", "Directories").unwrap();
        assert_eq!(count(&svg, "rect"), 3);
        let rects = squarify(&[6.0, 3.0, 1.0], 0.0, 0.0, 100.0, 100.0);
        let area: f64 = rects.iter().map(|(_, _, w, h)| w * h).sum();
        assert!((area - 10_000.0).abs() < 1e-6);

        let nodes = vec![
            GraphNode {
                id: "app".into(),
                label: "app".into(),
                layer: 1,
            },
            GraphNode {
                id: "core".into(),
                label: "core".into(),
                layer: 0,
            },
        ];
        let svg = layered_graph(&nodes, &[("app".into(), "core".into(), 3)], "Modules").unwrap();
        assert!(svg.contains("app → core: 3 imports"));

        let periods = vec![Period {
            start: 0,
            end: 100_000,
            label: "Initial".into(),
            slot: 0,
        }];
        let svg = timeline(
            &periods,
            &[(50_000, "Release 1.0".into())],
            &[(0, "Initial".into())],
            "Evolution",
        )
        .unwrap();
        assert!(svg.contains("Release 1.0"));
        assert!(timeline(&[], &[], &[], "none").is_none());
    }
}
