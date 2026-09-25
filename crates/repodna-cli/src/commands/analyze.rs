//! `repodna analyze`: run an analysis, store it, and summarize it.

use repodna_app::{AnalysisOutcome, AppError, report_bundle, run_analysis, write_bundle};
use repodna_core::finding::Finding;
use repodna_core::model::artifact::RepositoryDna;
use repodna_report::{ReportOptions, json_report, markdown_report};

use super::{Ctx, print};
use crate::cli::{AnalyzeCmd, AnalyzeFormat};
use crate::term::{Style, thousands, wrap};

/// Findings listed in the summary.
const TOP_FINDINGS: usize = 5;

fn plural(count: u64, one: &str, many: &str) -> String {
    format!(
        "{} {}",
        thousands(count),
        if count == 1 { one } else { many }
    )
}

fn languages(dna: &RepositoryDna) -> String {
    let shown: Vec<String> = dna
        .languages
        .languages
        .iter()
        .filter(|language| language.share >= 0.01)
        .take(4)
        .map(|language| format!("{} {:.0}%", language.name, language.share * 100.0))
        .collect();
    if shown.is_empty() {
        "none detected".to_owned()
    } else {
        shown.join(", ")
    }
}

/// The readable summary of an analysis.
pub fn summary(
    dna: &RepositoryDna,
    outcome: Option<&AnalysisOutcome>,
    style: Style,
    width: usize,
) -> String {
    let mut rows: Vec<(&str, String)> = Vec::new();
    let s = &dna.structure;
    let category = |id: &str| {
        s.categories
            .iter()
            .find(|c| {
                serde_json::to_value(c.category)
                    .ok()
                    .and_then(|v| v.as_str().map(|v| v == id))
                    .unwrap_or(false)
            })
            .map_or(0, |c| c.files)
    };
    rows.push((
        "Files",
        format!(
            "{} ({} source, {} test)",
            thousands(s.total_files),
            thousands(category("source")),
            thousands(category("test"))
        ),
    ));
    rows.push(("Code lines", thousands(s.code_lines)));
    rows.push(("Languages", languages(dna)));
    let g = &dna.git;
    if g.commit_count > 0 {
        let span = match (&g.first_commit, &g.last_commit) {
            (Some(first), Some(last)) => format!(
                ", {} → {}",
                first.timestamp.date_string(),
                last.timestamp.date_string()
            ),
            _ => String::new(),
        };
        rows.push((
            "History",
            format!(
                "{} by {}{span}",
                plural(g.commit_count, "commit", "commits"),
                plural(
                    u64::from(g.ownership.contributors),
                    "contributor",
                    "contributors"
                )
            ),
        ));
    }
    let a = &dna.architecture;
    if !a.style.is_empty() {
        rows.push((
            "Architecture",
            format!(
                "{} ({} confidence), {}",
                a.style,
                a.style_confidence.label().to_ascii_lowercase(),
                plural(a.modules.len() as u64, "module", "modules")
            ),
        ));
    }
    if dna.dependencies.direct_count > 0 {
        rows.push((
            "Dependencies",
            format!(
                "{} declared directly",
                thousands(dna.dependencies.direct_count)
            ),
        ));
    }
    let t = &dna.tests;
    rows.push((
        "Tests",
        if t.test_files + t.inline_test_files == 0 {
            "none detected".to_owned()
        } else {
            format!(
                "{} and {} with inline tests",
                plural(t.test_files, "test file", "test files"),
                plural(t.inline_test_files, "source file", "source files")
            )
        },
    ));
    let counts = dna.finding_counts();
    let mut findings = format!(
        "{} critical · {} · {} attention · {} info",
        counts.critical,
        plural(counts.warning as u64, "warning", "warnings"),
        counts.attention,
        counts.info
    );
    if counts.suppressed > 0 {
        findings.push_str(&format!(" ({} suppressed)", counts.suppressed));
    }
    rows.push(("Findings", findings));

    let mut out = String::new();
    let revision = dna
        .analysis_metadata
        .revision
        .as_deref()
        .map(|r| format!(" · revision {}", &r[..r.len().min(12)]))
        .unwrap_or_default();
    out.push_str(&format!(
        "{}{revision} · {} profile\n\n",
        style.bold(&dna.identity.name),
        dna.analysis_metadata.profile
    ));
    if let Some(description) = &dna.identity.description {
        out.push_str(&wrap(description, width, "  ", "  "));
        out.push_str("\n\n");
    }
    for (label, value) in &rows {
        out.push_str(&format!("  {:<14}{value}\n", style.dim(label)));
    }
    if let Some(changes) = outcome.and_then(|o| o.changes.as_ref()) {
        let added = changes.added.len();
        out.push_str(&format!(
            "\n  Since the previous analysis: {added} new {} and {} resolved.\n",
            if added == 1 { "finding" } else { "findings" },
            changes.resolved.len()
        ));
    }
    let mut top: Vec<&Finding> = dna.findings.iter().filter(|f| !f.is_suppressed()).collect();
    top.sort_by(|a, b| Finding::display_order(a, b));
    if !top.is_empty() {
        out.push_str(&format!("\n{}\n", style.bold("Top findings")));
        for finding in top.iter().take(TOP_FINDINGS) {
            out.push_str(&wrap(
                &format!("{} {}", style.severity(finding.severity), finding.title),
                width,
                "  ",
                "    ",
            ));
            out.push('\n');
        }
        if top.len() > TOP_FINDINGS {
            out.push_str(&style.dim(&format!(
                "  … and {} more (`repodna findings` lists them all)\n",
                top.len() - TOP_FINDINGS
            )));
        }
    }
    let warnings = &dna.analysis_metadata.warnings;
    if !warnings.is_empty() {
        out.push_str(&format!("\n{}\n", style.bold("Notes")));
        for warning in warnings {
            out.push_str(&wrap(warning, width, "  • ", "    "));
            out.push('\n');
        }
    }
    out
}

/// Runs `repodna analyze`.
pub fn run(ctx: &Ctx, cmd: &AnalyzeCmd) -> Result<(), AppError> {
    let options = ctx.analyze_options(&cmd.input, &cmd.analysis);
    ctx.note(&format!(
        "{} {} · analyzing {}",
        ctx.err.bold("RepoDNA"),
        env!("CARGO_PKG_VERSION"),
        cmd.input
    ));
    let result = run_analysis(&ctx.paths, &options, ctx.progress(), &ctx.cancel);
    if let Some(reporter) = &ctx.reporter {
        reporter.finish();
    }
    let outcome = result?;
    for warning in &outcome.warnings {
        ctx.warn(warning);
    }
    let dna = &outcome.dna;
    let report_options = ReportOptions {
        privacy: cmd.privacy.into(),
        ..ReportOptions::default()
    };
    match cmd.format {
        AnalyzeFormat::Text => {
            print(&summary(dna, Some(&outcome), ctx.out, crate::term::width()))?;
            if let Some(scan) = &outcome.scan {
                ctx.note(&ctx.err.dim(&format!(
                    "Stored as analysis {}. `repodna report {}` writes the full report.",
                    &scan.id[..scan.id.len().min(12)],
                    cmd.input
                )));
            }
        }
        AnalyzeFormat::Json => print(&json_report(dna, report_options.privacy, true)?)?,
        AnalyzeFormat::Markdown => print(&markdown_report(dna, &report_options))?,
    }
    if let Some(dir) = &cmd.output {
        let files = report_bundle(dna, &report_options)?;
        write_bundle(dir, &files, cmd.force)?;
        ctx.note(&format!("Report: {}", dir.join("index.html").display()));
    }
    Ok(())
}
