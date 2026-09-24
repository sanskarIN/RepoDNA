//! Reports generated from RepoDNA artifacts.
//!
//! Every report is built from a [`RepositoryDna`] artifact alone, so reports can be
//! regenerated later, on another machine, and without the analyzed repository. Privacy
//! presets are applied before anything is rendered.
//!
//! [`RepositoryDna`]: repodna_core::model::artifact::RepositoryDna

pub mod badge;
pub mod card;
pub mod charts;
pub mod content;
pub mod doc;
pub mod error;
pub mod facts;
pub mod fonts;
pub mod html;
pub mod markdown;
pub mod palette;
pub mod png;
pub mod privacy;
pub mod report;
pub mod sections;
pub mod text;

pub use error::ReportError;
pub use privacy::apply_privacy;
pub use report::{PROJECT_URL, ReportOptions, html_report, json_report, markdown_report};
pub use sections::{Section, SectionSet};
