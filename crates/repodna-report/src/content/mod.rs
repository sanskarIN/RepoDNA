//! Report content: every section of the full report as document blocks.

mod code;
mod overview;

use repodna_core::confidence::Confidence;
use repodna_core::model::SectionStatus;
use repodna_core::model::artifact::RepositoryDna;

use crate::doc::Blocks;
use crate::sections::{Section, SectionSet};

/// Options that change what content is produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContentOptions {
    /// Produce figures (HTML) in addition to tables.
    pub figures: bool,
    /// Figures and the card use the dark palette.
    pub dark: bool,
    /// Show "RepoDNA · Made by the Sanskar" on the card.
    pub branding: bool,
}

/// Adds the section heading.
pub(crate) fn heading(blocks: &mut Blocks, section: Section) {
    blocks.heading(2, section.title(), Some(section.id()));
}

/// When a section was not analyzed, explains why and returns `true`.
pub(crate) fn not_analyzed(blocks: &mut Blocks, status: SectionStatus, notes: &[String]) -> bool {
    if status.has_results() {
        if status == SectionStatus::Partial {
            blocks.note(format!(
                "Partially analyzed{}",
                notes
                    .first()
                    .map_or_else(|| ".".to_owned(), |note| format!(": {note}"))
            ));
        }
        return false;
    }
    let reason = notes
        .first()
        .cloned()
        .unwrap_or_else(|| "The analysis profile or configuration did not enable it.".to_owned());
    blocks.caution(format!("{}. {reason}", status.label()));
    true
}

/// Adds the section's notes (other than the first, which `not_analyzed` shows for
/// incomplete sections) as a list.
pub(crate) fn notes(blocks: &mut Blocks, notes: &[String]) {
    if !notes.is_empty() {
        blocks.list(
            notes
                .iter()
                .map(|note| crate::doc::plain(note.clone()))
                .collect(),
        );
    }
}

/// A confidence label for table cells.
pub(crate) fn confidence(confidence: Confidence) -> String {
    confidence.label().to_owned()
}

/// Builds one section.
pub fn section(dna: &RepositoryDna, section: Section, options: ContentOptions) -> Blocks {
    let mut blocks = Blocks::default();
    match section {
        Section::Cover => overview::cover(&mut blocks, dna, options),
        Section::Summary => overview::summary(&mut blocks, dna),
        Section::Identity => overview::identity(&mut blocks, dna),
        Section::Languages => overview::languages(&mut blocks, dna, options),
        Section::Structure => overview::structure(&mut blocks, dna, options),
        Section::Architecture => code::architecture(&mut blocks, dna, options),
        Section::Dependencies => code::dependencies(&mut blocks, dna),
        Section::Hotspots => code::hotspots(&mut blocks, dna, options),
        Section::Complexity => code::complexity(&mut blocks, dna, options),
        Section::Duplication => code::duplication(&mut blocks, dna),
        _ => {}
    }
    blocks
}

/// Builds every selected section in report order.
pub fn report(dna: &RepositoryDna, sections: SectionSet, options: ContentOptions) -> Blocks {
    let mut blocks = Blocks::default();
    for selected in sections.iter() {
        blocks.0.extend(section(dna, selected, options).0);
    }
    blocks
}
