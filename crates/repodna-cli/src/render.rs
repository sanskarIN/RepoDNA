//! Renders report blocks as terminal text, so terminal views show the same facts as the
//! Markdown and HTML reports. Figures are skipped: each has a table with the same values.

use repodna_report::doc::{Block, Blocks, Inline, NoteKind, Rich, Table};

use crate::term::{Style, display_width, wrap};

/// Widest table column; longer cells wrap.
const MAX_COLUMN: usize = 60;
/// Text columns are not squeezed below their longest word, up to this width.
const MIN_TEXT_COLUMN: usize = 24;
/// Evidence items shown per finding.
const FINDING_EVIDENCE: usize = 3;

fn inline(rich: &Rich, style: Style) -> String {
    rich.iter()
        .map(|part| match part {
            Inline::Text(text) => text.clone(),
            Inline::Strong(text) => style.bold(text),
            Inline::Code(text) => style.cyan(text),
            Inline::Link { text, url } => {
                if text == url {
                    url.clone()
                } else {
                    format!("{text} ({url})")
                }
            }
        })
        .collect()
}

fn plain(rich: &Rich) -> String {
    inline(rich, Style::plain())
}

/// Splits `text` into lines of at most `width` columns, breaking at spaces and, for
/// longer words such as paths, after `/` or anywhere as a last resort.
fn wrap_cell(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let mut word = word.to_owned();
        loop {
            let used = display_width(&line);
            let separator = usize::from(!line.is_empty());
            if used + separator + display_width(&word) <= width {
                if separator == 1 {
                    line.push(' ');
                }
                line.push_str(&word);
                break;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                continue;
            }
            // The word alone is too long: break it, preferably after a slash.
            let chars: Vec<char> = word.chars().collect();
            let limit = width.min(chars.len());
            let cut = chars[..limit]
                .iter()
                .rposition(|&c| c == '/')
                .filter(|&at| at > 0 && at + 1 < limit)
                .map_or(limit, |at| at + 1);
            lines.push(chars[..cut].iter().collect());
            word = chars[cut..].iter().collect();
            if word.is_empty() {
                break;
            }
        }
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

/// Column widths that fit `available` columns when possible. Each column starts at its
/// minimum (numeric columns: their natural width; text columns: their longest word, up to
/// [`MIN_TEXT_COLUMN`]), and the remaining space goes to text columns in proportion to how
/// much wider they would naturally be. When even the minimums do not fit, the table is
/// wider than `available` rather than breaking words into fragments.
fn allocate(
    natural: &[usize],
    minimum: &[usize],
    numeric: &[bool],
    available: usize,
) -> Vec<usize> {
    let gaps = 2 * natural.len().saturating_sub(1);
    if natural.iter().sum::<usize>() + gaps <= available {
        return natural.to_vec();
    }
    let floor: Vec<usize> = natural
        .iter()
        .zip(minimum)
        .zip(numeric)
        .map(|((natural, minimum), numeric)| {
            if *numeric {
                *natural
            } else {
                (*minimum).min(*natural)
            }
        })
        .collect();
    let used: usize = floor.iter().sum::<usize>() + gaps;
    let extra = available.saturating_sub(used);
    let wanted: usize = natural.iter().zip(&floor).map(|(n, f)| n - f).sum();
    if extra == 0 || wanted == 0 {
        return floor;
    }
    natural
        .iter()
        .zip(&floor)
        .map(|(natural, floor)| floor + (natural - floor) * extra / wanted)
        .collect()
}

fn longest_word(text: &str) -> usize {
    text.split_whitespace()
        .map(display_width)
        .max()
        .unwrap_or(0)
}

fn table(out: &mut String, table: &Table, style: Style, width: usize) {
    let cells: Vec<Vec<String>> = table
        .rows
        .iter()
        .map(|row| row.iter().map(plain).collect())
        .collect();
    let columns = table.headers.len();
    let mut natural: Vec<usize> = table
        .headers
        .iter()
        .map(|header| display_width(header).min(MAX_COLUMN))
        .collect();
    for row in &cells {
        for (index, cell) in row.iter().enumerate().take(columns) {
            natural[index] = natural[index].max(display_width(cell).min(MAX_COLUMN));
        }
    }
    let numeric: Vec<bool> = (0..columns)
        .map(|index| table.numeric.get(index).copied().unwrap_or(false))
        .collect();
    // Numeric headers may wrap between words; their values never do.
    for (index, header) in table.headers.iter().enumerate() {
        if numeric[index] {
            let values = cells
                .iter()
                .filter_map(|row| row.get(index))
                .map(|cell| display_width(cell))
                .max()
                .unwrap_or(0);
            natural[index] = values.max(longest_word(header));
        }
    }
    let minimum: Vec<usize> = (0..columns)
        .map(|index| {
            let words = cells
                .iter()
                .filter_map(|row| row.get(index))
                .map(|cell| longest_word(cell))
                .chain(std::iter::once(longest_word(&table.headers[index])))
                .max()
                .unwrap_or(0);
            words.clamp(1, MIN_TEXT_COLUMN)
        })
        .collect();
    let widths = allocate(&natural, &minimum, &numeric, width.saturating_sub(2));
    let render_row = |values: &[String], bold: bool| -> String {
        let wrapped: Vec<Vec<String>> = (0..columns)
            .map(|index| wrap_cell(values.get(index).map_or("", String::as_str), widths[index]))
            .collect();
        let height = wrapped.iter().map(Vec::len).max().unwrap_or(1);
        let mut lines = String::new();
        for line_index in 0..height {
            let parts: Vec<String> = wrapped
                .iter()
                .enumerate()
                .map(|(index, lines)| {
                    let value = lines.get(line_index).map_or("", String::as_str);
                    let pad = " ".repeat(widths[index].saturating_sub(display_width(value)));
                    let padded = if numeric[index] {
                        format!("{pad}{value}")
                    } else {
                        format!("{value}{pad}")
                    };
                    if bold { style.bold(&padded) } else { padded }
                })
                .collect();
            lines.push_str(format!("  {}", parts.join("  ")).trim_end());
            lines.push('\n');
        }
        lines
    };
    out.push_str(&render_row(&table.headers, true));
    let rule: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
    out.push_str(&style.dim(&format!("  {}", rule.join("  "))));
    out.push('\n');
    for row in &cells {
        out.push_str(&render_row(row, false));
    }
    if table.omitted > 0 {
        out.push_str(&style.dim(&format!("  … {} more rows not shown", table.omitted)));
        out.push('\n');
    }
}

/// Renders `blocks` for a terminal `width` columns wide.
pub fn terminal(blocks: &Blocks, style: Style, width: usize) -> String {
    let mut out = String::new();
    for block in &blocks.0 {
        match block {
            Block::Heading { level, text, .. } => {
                if !out.is_empty() {
                    out.push('\n');
                }
                if *level <= 2 {
                    out.push_str(&style.bold(text));
                    out.push('\n');
                    out.push_str(&style.dim(&"─".repeat(display_width(text).min(width))));
                } else {
                    out.push_str(&style.bold(text));
                }
                out.push('\n');
            }
            Block::Paragraph(rich) => {
                out.push_str(&wrap(&inline(rich, style), width, "", ""));
                out.push('\n');
            }
            Block::Note(kind, rich) => {
                let label = match kind {
                    NoteKind::Info => style.dim("Note:"),
                    NoteKind::Caution => style.yellow("Caution:"),
                };
                out.push_str(&wrap(
                    &format!("{label} {}", inline(rich, style)),
                    width,
                    "",
                    "  ",
                ));
                out.push('\n');
            }
            Block::List(items) => {
                for item in items {
                    out.push_str(&wrap(&inline(item, style), width, "  • ", "    "));
                    out.push('\n');
                }
            }
            Block::Table(data) => table(&mut out, data, style, width),
            Block::Stats(stats) => {
                let label_width = stats
                    .iter()
                    .map(|(label, _)| display_width(label))
                    .max()
                    .unwrap_or(0);
                for (label, value) in stats {
                    let pad = " ".repeat(label_width - display_width(label));
                    out.push_str(&format!("  {}{pad}  {value}\n", style.dim(label)));
                }
            }
            Block::Figure { .. } => {}
            Block::Finding(finding) => {
                let title = format!("{} {}", style.severity(finding.severity), finding.title);
                out.push_str(&wrap(&title, width, "  ", "    "));
                out.push('\n');
                if !finding.summary.is_empty() {
                    out.push_str(&wrap(&finding.summary, width, "    ", "    "));
                    out.push('\n');
                }
                for evidence in finding.evidence.iter().take(FINDING_EVIDENCE) {
                    out.push_str(&wrap(
                        &style.dim(&format!("evidence: {}", evidence.describe())),
                        width,
                        "    ",
                        "      ",
                    ));
                    out.push('\n');
                }
                let mut meta = format!(
                    "rule {} · {} confidence",
                    finding.rule,
                    finding.confidence.label().to_ascii_lowercase()
                );
                if let Some(suppression) = &finding.suppressed {
                    meta.push_str(&format!(" · suppressed: {}", suppression.reason));
                }
                out.push_str(&wrap(&style.dim(&meta), width, "    ", "    "));
                out.push('\n');
            }
            Block::Preformatted(text) => {
                for line in text.lines() {
                    out.push_str(&format!("    {line}\n"));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_report::doc::{code, plain as rich};

    #[test]
    fn renders_blocks_as_text() {
        let mut blocks = Blocks::default();
        blocks.heading(2, "Languages", None);
        blocks.text("Rust dominates this repository.");
        blocks.caution("Some files were skipped.");
        blocks.list(vec![rich("first"), code("second")]);
        let mut data = Table::new(&["Language", "Files#"]);
        data.row(vec![rich("Rust"), rich("1,204")]);
        data.row(vec![rich("TypeScript"), rich("88")]);
        data.omitted = 3;
        blocks.table(data);
        blocks.stats(vec![
            ("Files".into(), "10".into()),
            ("Code lines".into(), "900".into()),
        ]);
        let text = terminal(&blocks, Style::plain(), 80);
        assert!(text.starts_with("Languages\n─────────\n"));
        assert!(text.contains("Caution: Some files were skipped."));
        assert!(text.contains("  • second\n"));
        assert!(text.contains("  Language    Files\n"));
        assert!(text.contains("  Rust        1,204\n"));
        assert!(text.contains("  TypeScript     88\n"));
        assert!(text.contains("… 3 more rows not shown"));
        assert!(text.contains("  Files       10\n"));
    }

    #[test]
    fn wraps_cells_at_spaces_and_slashes() {
        assert_eq!(wrap_cell("one two three", 7), vec!["one two", "three"]);
        assert_eq!(
            wrap_cell("crates/repodna-core/src/lib.rs", 14),
            vec!["crates/", "repodna-core/", "src/lib.rs"]
        );
        assert_eq!(wrap_cell("abcdefgh", 3), vec!["abc", "def", "gh"]);
        assert_eq!(wrap_cell("", 5), vec![""]);
        assert_eq!(allocate(&[5, 5], &[5, 5], &[false, true], 80), vec![5, 5]);
        // 60 columns: gaps 4, numeric 5, floors 10 + 10, extra 31 split 50:30.
        assert_eq!(
            allocate(&[60, 40, 5], &[10, 10, 5], &[false, false, true], 60),
            vec![29, 21, 5]
        );
        // Minimums win over the available width.
        assert_eq!(
            allocate(&[60, 40], &[24, 24], &[false, false], 30),
            vec![24, 24]
        );
    }

    #[test]
    fn narrows_wide_tables() {
        let mut blocks = Blocks::default();
        let mut data = Table::new(&["Path", "Note"]);
        data.row(vec![rich("x".repeat(70)), rich("y".repeat(70))]);
        blocks.table(data);
        let text = terminal(&blocks, Style::plain(), 60);
        assert!(text.lines().all(|line| display_width(line) <= 60), "{text}");
        assert_eq!(text.matches('x').count(), 70, "{text}");
        assert_eq!(text.matches('y').count(), 70, "{text}");
    }
}
