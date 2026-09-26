//! `repodna compare`.

use repodna_app::AppError;
use repodna_report::compare::{blocks, html_comparison, json_comparison, markdown_comparison};
use repodna_report::{apply_privacy, compare};

use super::{Ctx, to_json};
use crate::cli::{CompareCmd, CompareFormat};
use crate::render::terminal;
use crate::term::width;

/// Runs `repodna compare`.
pub fn run(ctx: &Ctx, cmd: &CompareCmd) -> Result<(), AppError> {
    let mut artifacts = Vec::with_capacity(cmd.targets.len());
    for target in &cmd.targets {
        let loaded = ctx.load(target, &cmd.analysis)?;
        artifacts.push(apply_privacy(&loaded.dna, cmd.privacy.into()));
    }
    let comparison = compare(&artifacts);
    let text = match cmd.format {
        CompareFormat::Text => terminal(&blocks(&comparison, false), ctx.out, width()),
        CompareFormat::Markdown => markdown_comparison(&comparison),
        CompareFormat::Json => to_json(&json_comparison(&comparison))?,
        CompareFormat::Html => html_comparison(&comparison, cmd.theme.into(), true),
    };
    ctx.emit(cmd.output.as_ref(), &text, cmd.force)
}
