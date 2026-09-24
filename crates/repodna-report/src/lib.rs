//! Reports generated from RepoDNA artifacts.
//!
//! Every report is built from a [`RepositoryDna`] artifact alone, so reports can be
//! regenerated later, on another machine, and without the analyzed repository. Privacy
//! presets are applied before anything is rendered.
//!
//! [`RepositoryDna`]: repodna_core::model::artifact::RepositoryDna

pub mod fonts;
pub mod palette;
pub mod privacy;
pub mod sections;
pub mod text;

pub use privacy::apply_privacy;
pub use sections::{Section, SectionSet};
