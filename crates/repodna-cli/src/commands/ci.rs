//! `repodna ci`: a concise summary for continuous integration, with an optional failure
//! threshold. Findings are signals backed by evidence, not formal guarantees, so the
//! default is to report without failing.

use std::io::Write;

use repodna_app::AppError;
use repodna_core::io::read_artifact;
use repodna_core::severity::Severity;
use repodna_report::ci::{CiPolicy, evaluate, github_annotations, json, markdown, text};

use super::{Ctx, print, to_json};
use crate::cli::{CiCmd, CiFormat, FailOn};

fn threshold(fail_on: FailOn) -> Option<Severity> {
    match fail_on {
        FailOn::Never => None,
        FailOn::Info => Some(Severity::Info),
        FailOn::Attention => Some(Severity::Attention),
        FailOn::Warning => Some(Severity::Warning),
        FailOn::Critical => Some(Severity::Critical),
    }
}

/// Runs `repodna ci`.
pub fn run(ctx: &Ctx, cmd: &CiCmd) -> Result<(), AppError> {
    let baseline = cmd
        .baseline
        .as_ref()
        .map(|path| read_artifact(path).map(|loaded| loaded.artifact))
        .transpose()?;
    let loaded = ctx.load(&cmd.target, &cmd.analysis)?;
    let policy = CiPolicy {
        fail_on: threshold(cmd.fail_on),
        new_only: cmd.new_only,
    };
    let outcome = evaluate(&loaded.dna, baseline.as_ref(), policy);
    match cmd.format {
        CiFormat::Text => print(&text(&outcome))?,
        CiFormat::Markdown => print(&markdown(&outcome))?,
        CiFormat::Json => print(&to_json(&json(&outcome))?)?,
        CiFormat::Github => {
            let minimum = policy.fail_on.unwrap_or(Severity::Warning);
            let mut out = github_annotations(&loaded.dna, minimum).join("\n");
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&text(&outcome));
            print(&out)?;
        }
    }
    if let Some(path) = &cmd.summary_file {
        // Job summary files are appended to, as CI systems expect.
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|error| AppError::input(format!("cannot open {}: {error}", path.display())))?;
        file.write_all(markdown(&outcome).as_bytes())
            .map_err(|error| {
                AppError::input(format!("cannot write {}: {error}", path.display()))
            })?;
    }
    if outcome.failed {
        let severity = policy
            .fail_on
            .map_or("any", |s| s.label())
            .to_ascii_lowercase();
        return Err(AppError::policy(format!(
            "{} findings at or above {severity} severity{}",
            outcome.failing.len(),
            if cmd.new_only { " that the baseline does not have" } else { "" }
        ))
        .with_hint(
            "Review them with `repodna findings`, fix them, or record accepted ones as suppressions in repodna.toml.",
        ));
    }
    Ok(())
}
