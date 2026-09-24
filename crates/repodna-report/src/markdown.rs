//! Renders the document model as GitHub-flavored Markdown.

use repodna_core::finding::Finding;

use crate::doc::{Block, Blocks, Inline, NoteKind, Rich, Table};
use crate::text::{code, escape_markdown};

/// Renders inline content.
pub fn inline(rich: &Rich) -> String {
    rich.iter()
        .map(|part| match part {
            Inline::Text(text) => escape_markdown(text),
            Inline::Strong(text) => format!("**{}**", escape_markdown(text)),
            Inline::Code(text) => code(text),
            Inline::Link { text, url } => {
                format!("[{}](<{}>)", escape_markdown(text), url.replace('>', "%3E"))
            }
        })
        .collect()
}

fn table(out: &mut String, table: &Table) {
    out.push_str(&format!(
        "| {} |\n",
        table
            .headers
            .iter()
            .map(|h| escape_markdown(h))
            .collect::<Vec<_>>()
            .join(" | ")
    ));
    out.push_str(&format!(
        "|{}|\n",
        table
            .numeric
            .iter()
            .map(|numeric| if *numeric { " ---: " } else { " --- " })
            .collect::<Vec<_>>()
            .join("|")
    ));
    for row in &table.rows {
        let cells: Vec<String> = row.iter().map(inline).collect();
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    if table.omitted > 0 {
        out.push_str(&format!(
            "\n_{} more not shown; the JSON artifact has the complete list._\n",
            table.omitted
        ));
    }
    out.push('\n');
}

fn finding(out: &mut String, finding: &Finding) {
    out.push_str(&format!(
        "#### {} · {}\n\n",
        finding.severity.label(),
        escape_markdown(&finding.title)
    ));
    let mut meta = format!(
        "Rule {} · {} confidence · ID {}",
        code(&finding.rule),
        finding.confidence.label(),
        code(&finding.id)
    );
    if let Some(suppression) = &finding.suppressed {
        meta.push_str(&format!(
            " · **Suppressed** ({}: {})",
            escape_markdown(&suppression.source),
            escape_markdown(&suppression.reason)
        ));
    }
    out.push_str(&meta);
    out.push_str("\n\n");
    for (label, text) in [
        ("What was observed", &finding.summary),
        ("Why it may matter", &finding.rationale),
        ("How it was determined", &finding.method),
    ] {
        if !text.is_empty() {
            out.push_str(&format!("**{label}:** {}\n\n", escape_markdown(text)));
        }
    }
    if !finding.evidence.is_empty() {
        out.push_str("**Evidence:**\n\n");
        for evidence in finding.evidence.iter().take(10) {
            out.push_str(&format!("- {}\n", code(&evidence.describe())));
        }
        if finding.evidence.len() > 10 {
            out.push_str(&format!("- …and {} more\n", finding.evidence.len() - 10));
        }
        out.push('\n');
    }
    for (label, items) in [
        ("Limitations", &finding.limitations),
        ("Next steps", &finding.next_steps),
    ] {
        if !items.is_empty() {
            out.push_str(&format!("**{label}:**\n\n"));
            for item in items {
                out.push_str(&format!("- {}\n", escape_markdown(item)));
            }
            out.push('\n');
        }
    }
}

/// Renders blocks as Markdown.
pub fn render(blocks: &Blocks) -> String {
    let mut out = String::new();
    for block in &blocks.0 {
        match block {
            Block::Heading { level, text, .. } => {
                out.push_str(&format!(
                    "{} {}\n\n",
                    "#".repeat(usize::from((*level).clamp(1, 6))),
                    escape_markdown(text)
                ));
            }
            Block::Paragraph(rich) => {
                out.push_str(&inline(rich));
                out.push_str("\n\n");
            }
            Block::Note(kind, rich) => {
                let label = match kind {
                    NoteKind::Info => "Note",
                    NoteKind::Caution => "Not available",
                };
                out.push_str(&format!("> **{label}:** {}\n\n", inline(rich)));
            }
            Block::List(items) => {
                for item in items {
                    out.push_str(&format!("- {}\n", inline(item)));
                }
                out.push('\n');
            }
            Block::Table(t) => table(&mut out, t),
            Block::Stats(stats) => {
                let mut t = Table::new(&["Measure", "Value#"]);
                for (label, value) in stats {
                    t.row(vec![
                        crate::doc::plain(label.clone()),
                        crate::doc::plain(value.clone()),
                    ]);
                }
                table(&mut out, &t);
            }
            // Charts are visual only; the accompanying tables carry the values.
            Block::Figure { .. } => {}
            Block::Finding(f) => finding(&mut out, f),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{code as code_cell, plain};
    use repodna_core::confidence::Confidence;
    use repodna_core::evidence::Evidence;
    use repodna_core::finding::FindingCategory;
    use repodna_core::severity::Severity;

    #[test]
    fn renders_blocks() {
        let mut blocks = Blocks::default();
        blocks.heading(2, "Languages", None);
        blocks.rich(vec![
            Inline::Text("See ".into()),
            Inline::Code("src/main.rs".into()),
            Inline::Text(" and ".into()),
            Inline::Link {
                text: "RepoDNA".into(),
                url: "https://github.com/sanskarIN/RepoDNA".into(),
            },
        ]);
        blocks.caution("Git history was not analyzed.");
        let mut t = Table::new(&["Name", "Files#"]);
        t.row(vec![plain("a|b"), code_cell("x")]);
        t.omitted = 3;
        blocks.table(t);
        blocks.stats(vec![("Files".into(), "12".into())]);
        blocks.figure(Some("<svg/>".into()), "chart");
        let markdown = render(&blocks);
        assert!(markdown.starts_with("## Languages\n\n"));
        assert!(
            markdown.contains(
                "See `src/main.rs` and [RepoDNA](<https://github.com/sanskarIN/RepoDNA>)"
            )
        );
        assert!(markdown.contains("> **Not available:** Git history was not analyzed."));
        assert!(markdown.contains("| Name | Files |\n| --- | ---: |\n| a\\|b | `x` |"));
        assert!(markdown.contains("_3 more not shown"));
        assert!(markdown.contains("| Files | 12 |"));
        assert!(!markdown.contains("<svg"));
    }

    #[test]
    fn renders_findings_completely() {
        let mut f = Finding::new(
            "architecture.cycle",
            "a|b",
            FindingCategory::Architecture,
            Severity::Warning,
            Confidence::Medium,
            "Modules a and b depend on each other",
        )
        .summary("a imports b and b imports a.")
        .rationale("Cycles make modules hard to change separately.")
        .method("Strongly connected components.")
        .evidence(Evidence::file("src/a.rs"))
        .limitation("Dynamic imports are not seen.")
        .next_step("Extract the shared part.");
        f.suppressed = Some(repodna_core::finding::Suppression {
            reason: "Known".into(),
            source: "repodna.toml".into(),
        });
        let mut blocks = Blocks::default();
        blocks.0.push(Block::Finding(Box::new(f)));
        let markdown = render(&blocks);
        for expected in [
            "#### Warning · Modules a and b depend on each other",
            "Medium confidence",
            "**Suppressed** (repodna.toml: Known)",
            "**What was observed:** a imports b and b imports a.",
            "- `src/a.rs`",
            "**Limitations:**",
            "- Extract the shared part.",
        ] {
            assert!(
                markdown.contains(expected),
                "missing {expected}\n{markdown}"
            );
        }
    }
}
