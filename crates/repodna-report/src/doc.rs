//! A small document model shared by the Markdown and HTML renderers.
//!
//! Report sections are built once as blocks and rendered to each format, so both formats
//! always contain the same facts. Figures are visual only: every figure is accompanied by
//! a table with the same values.

use repodna_core::finding::Finding;

/// A run of inline content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    /// Plain text.
    Text(String),
    /// Emphasized text.
    Strong(String),
    /// Code, paths, and identifiers.
    Code(String),
    /// A link to an absolute URL.
    Link {
        /// Link text.
        text: String,
        /// Target.
        url: String,
    },
}

/// Inline content of a paragraph, list item, or table cell.
pub type Rich = Vec<Inline>;

/// Plain text as rich content.
pub fn plain(text: impl Into<String>) -> Rich {
    vec![Inline::Text(text.into())]
}

/// Code as rich content.
pub fn code(text: impl Into<String>) -> Rich {
    vec![Inline::Code(text.into())]
}

/// The kind of a callout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    /// Context or limitations.
    Info,
    /// Something did not run or is incomplete.
    Caution,
}

/// A table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Table {
    /// Column headings.
    pub headers: Vec<String>,
    /// Right-aligned (numeric) columns.
    pub numeric: Vec<bool>,
    /// Rows of cells.
    pub rows: Vec<Vec<Rich>>,
    /// Rows omitted from a longer list, stated below the table.
    pub omitted: usize,
}

impl Table {
    /// A table with the given headings; headings ending in `#` are numeric (the marker is
    /// removed).
    pub fn new(headers: &[&str]) -> Self {
        Self {
            headers: headers
                .iter()
                .map(|h| h.trim_end_matches('#').to_owned())
                .collect(),
            numeric: headers.iter().map(|h| h.ends_with('#')).collect(),
            rows: Vec::new(),
            omitted: 0,
        }
    }

    /// Adds a row.
    pub fn row(&mut self, cells: Vec<Rich>) {
        self.rows.push(cells);
    }
}

/// A block of content.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// A heading (level 2 is a section, 3 a subsection).
    Heading {
        /// Level (1–4).
        level: u8,
        /// Text.
        text: String,
        /// Anchor identifier.
        id: Option<String>,
    },
    /// A paragraph.
    Paragraph(Rich),
    /// A callout.
    Note(NoteKind, Rich),
    /// A bulleted list.
    List(Vec<Rich>),
    /// A table.
    Table(Table),
    /// Headline numbers.
    Stats(Vec<(String, String)>),
    /// An SVG chart with its caption (rendered in HTML only; a table carries the values).
    Figure {
        /// SVG markup.
        svg: String,
        /// Caption.
        caption: String,
    },
    /// A full finding.
    Finding(Box<Finding>),
    /// Preformatted text such as command output.
    Preformatted(String),
}

/// A report section's blocks.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Blocks(pub Vec<Block>);

impl Blocks {
    /// Adds a heading.
    pub fn heading(&mut self, level: u8, text: impl Into<String>, id: Option<&str>) {
        self.0.push(Block::Heading {
            level,
            text: text.into(),
            id: id.map(str::to_owned),
        });
    }

    /// Adds a paragraph of plain text.
    pub fn text(&mut self, text: impl Into<String>) {
        self.0.push(Block::Paragraph(plain(text)));
    }

    /// Adds a paragraph of rich text.
    pub fn rich(&mut self, rich: Rich) {
        self.0.push(Block::Paragraph(rich));
    }

    /// Adds an informational callout.
    pub fn note(&mut self, text: impl Into<String>) {
        self.0.push(Block::Note(NoteKind::Info, plain(text)));
    }

    /// Adds a caution callout.
    pub fn caution(&mut self, text: impl Into<String>) {
        self.0.push(Block::Note(NoteKind::Caution, plain(text)));
    }

    /// Adds a list; empty lists are skipped.
    pub fn list(&mut self, items: Vec<Rich>) {
        if !items.is_empty() {
            self.0.push(Block::List(items));
        }
    }

    /// Adds a table; tables without rows are skipped.
    pub fn table(&mut self, table: Table) {
        if !table.rows.is_empty() {
            self.0.push(Block::Table(table));
        }
    }

    /// Adds headline numbers.
    pub fn stats(&mut self, stats: Vec<(String, String)>) {
        if !stats.is_empty() {
            self.0.push(Block::Stats(stats));
        }
    }

    /// Adds a figure when there is one.
    pub fn figure(&mut self, svg: Option<String>, caption: impl Into<String>) {
        if let Some(svg) = svg {
            self.0.push(Block::Figure {
                svg,
                caption: caption.into(),
            });
        }
    }
}

/// Keeps the first `limit` items and returns how many were dropped.
pub fn truncate<T>(items: &mut Vec<T>, limit: usize) -> usize {
    let dropped = items.len().saturating_sub(limit);
    items.truncate(limit);
    dropped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_blocks() {
        let mut blocks = Blocks::default();
        blocks.heading(2, "Languages", Some("languages"));
        blocks.list(Vec::new());
        blocks.table(Table::new(&["Name", "Files#"]));
        let mut table = Table::new(&["Name", "Files#"]);
        assert_eq!(table.headers, vec!["Name", "Files"]);
        assert_eq!(table.numeric, vec![false, true]);
        table.row(vec![plain("Rust"), plain("3")]);
        blocks.table(table);
        blocks.figure(None, "nothing");
        assert_eq!(blocks.0.len(), 2);
        let mut items = vec![1, 2, 3];
        assert_eq!(truncate(&mut items, 2), 1);
        assert_eq!(items, vec![1, 2]);
    }
}
