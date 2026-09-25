//! Terminal views: `architecture`, `dependencies`, `history`, `hotspots`, `timeline`,
//! `findings`, and `show`. They render the same report sections as the HTML and Markdown
//! reports, or the matching part of the artifact as JSON.

use repodna_app::AppError;
use repodna_core::finding::Finding;
use repodna_core::model::artifact::RepositoryDna;
use repodna_core::severity::Severity;
use repodna_report::content::{ContentOptions, report};
use repodna_report::doc::{Block, Blocks};
use repodna_report::{Section, SectionSet, apply_privacy, markdown};
use serde_json::{Value, json};

use super::{Ctx, print, to_json};
use crate::cli::{FindingsCmd, ShowCmd, ViewCmd, ViewFormat};
use crate::render::terminal;
use crate::term::width;

/// A terminal view: which sections it shows and which part of the artifact it exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// `repodna architecture`.
    Architecture,
    /// `repodna dependencies`.
    Dependencies,
    /// `repodna history`.
    History,
    /// `repodna hotspots`.
    Hotspots,
    /// `repodna timeline`.
    Timeline,
}

impl View {
    fn sections(self) -> &'static [Section] {
        match self {
            Self::Architecture => &[Section::Architecture],
            Self::Dependencies => &[Section::Dependencies],
            Self::History => &[Section::History, Section::Contributors],
            Self::Hotspots => &[Section::Hotspots, Section::Complexity],
            Self::Timeline => &[Section::TimeMachine, Section::Evolution],
        }
    }

    fn json(self, dna: &RepositoryDna) -> Value {
        match self {
            Self::Architecture => json!({ "architecture": dna.architecture }),
            Self::Dependencies => json!({ "dependencies": dna.dependencies }),
            Self::History => json!({ "git": dna.git }),
            Self::Hotspots => json!({
                "hotspots": dna.git.hot_spots,
                "complexity": dna.code_quality.complexity,
            }),
            Self::Timeline => json!({ "evolution": dna.evolution }),
        }
    }
}

fn render(ctx: &Ctx, blocks: &Blocks, format: ViewFormat) -> Result<(), AppError> {
    match format {
        ViewFormat::Markdown => print(&markdown::render(blocks)),
        _ => print(&terminal(blocks, ctx.out, width())),
    }
}

fn section_blocks(dna: &RepositoryDna, sections: &[Section]) -> Blocks {
    report(dna, SectionSet::of(sections), ContentOptions::default())
}

/// Runs a view command.
pub fn run_view(ctx: &Ctx, cmd: &ViewCmd, view: View) -> Result<(), AppError> {
    let loaded = ctx.load(&cmd.target, &cmd.analysis)?;
    let dna = apply_privacy(&loaded.dna, cmd.privacy.into());
    match cmd.format {
        ViewFormat::Json => {
            let mut value = view.json(&dna);
            if let Value::Object(map) = &mut value {
                map.insert("repository".to_owned(), json!(dna.identity.name));
                map.insert("revision".to_owned(), json!(dna.analysis_metadata.revision));
            }
            print(&to_json(&value)?)
        }
        format => render(ctx, &section_blocks(&dna, view.sections()), format),
    }
}

/// Runs `repodna findings`.
pub fn run_findings(ctx: &Ctx, cmd: &FindingsCmd) -> Result<(), AppError> {
    let loaded = ctx.load(&cmd.target, &cmd.analysis)?;
    let dna = apply_privacy(&loaded.dna, cmd.privacy.into());
    let minimum: Severity = cmd.severity.into();
    let mut findings: Vec<&Finding> = dna
        .findings
        .iter()
        .filter(|f| f.severity >= minimum)
        .filter(|f| cmd.include_suppressed || !f.is_suppressed())
        .filter(|f| {
            cmd.rule
                .as_deref()
                .is_none_or(|prefix| f.rule.starts_with(prefix))
        })
        .collect();
    findings.sort_by(|a, b| Finding::display_order(a, b));
    if cmd.format == ViewFormat::Json {
        return print(&to_json(&findings)?);
    }
    let mut blocks = Blocks::default();
    blocks.heading(2, format!("Findings in {}", dna.identity.name), None);
    if findings.is_empty() {
        blocks.text("No findings match these filters.");
    } else {
        blocks.text(format!(
            "{} findings. Findings are signals backed by evidence, not verdicts; each lists how it was measured.",
            findings.len()
        ));
    }
    for finding in findings {
        blocks.0.push(Block::Finding(Box::new(finding.clone())));
    }
    render(ctx, &blocks, cmd.format)
}

/// Runs `repodna show`.
pub fn run_show(ctx: &Ctx, cmd: &ShowCmd) -> Result<(), AppError> {
    if cmd.list {
        let mut out = String::new();
        for section in SectionSet::all().iter() {
            out.push_str(&format!("{:<16}{}\n", section.id(), section.title()));
        }
        return print(&out);
    }
    let sections = SectionSet::parse(&cmd.section).map_err(|error| {
        AppError::usage(error).with_hint("`repodna show --list` prints the section identifiers.")
    })?;
    if cmd.format == ViewFormat::Json {
        return Err(AppError::usage(
            "`repodna show` prints text or Markdown; use `repodna report --format json` for JSON",
        ));
    }
    let loaded = ctx.load(&cmd.target, &cmd.analysis)?;
    let dna = apply_privacy(&loaded.dna, cmd.privacy.into());
    render(
        ctx,
        &report(&dna, sections, ContentOptions::default()),
        cmd.format,
    )
}
