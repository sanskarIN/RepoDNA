//! Output commands: `report`, `card`, `badge`, `onboarding`, `export`, `import`, and
//! `list`.

use std::path::PathBuf;

use repodna_app::{
    AppError, OutputFile, artifact_file_name, import_artifact, report_bundle, write_bundle,
    write_file,
};
use repodna_core::config::PrivacyPreset;
use repodna_report::badge::{BadgeKind, badge, markdown_snippet, render_badge};
use repodna_report::{
    CsvTable, ReportOptions, SectionSet, apply_privacy, csv, html_report, json_report,
    markdown_report, onboarding_bundle,
};
use serde_json::json;

use super::{Ctx, print, to_json};
use crate::cli::{
    BadgeCmd, CardCmd, CardFormat, ExportCmd, ImportCmd, ListCmd, ListFormat, OnboardingCmd,
    ReportCmd, ReportFormat,
};
use crate::term::thousands;

/// Default directory of report bundles.
const DEFAULT_REPORT_DIR: &str = "repodna-report";

/// Runs `repodna report`.
pub fn run_report(ctx: &Ctx, cmd: &ReportCmd) -> Result<(), AppError> {
    let config = ctx.config_for(&cmd.target, &cmd.analysis)?;
    let mut options = ReportOptions::from_config(&config.report)?;
    if let Some(theme) = cmd.theme {
        options.theme = theme.into();
    }
    if let Some(privacy) = cmd.privacy {
        options.privacy = privacy.into();
    }
    if cmd.no_branding {
        options.branding = false;
    }
    if !cmd.sections.is_empty() {
        options.sections = SectionSet::parse(&cmd.sections).map_err(|error| {
            AppError::usage(error)
                .with_hint("`repodna show --list` prints the section identifiers.")
        })?;
    }
    let table: Option<CsvTable> = match cmd.format {
        ReportFormat::Csv => Some(cmd.table.parse().map_err(AppError::usage)?),
        _ => None,
    };
    let loaded = ctx.load(&cmd.target, &cmd.analysis)?;
    let dna = &loaded.dna;
    match cmd.format {
        ReportFormat::Bundle => {
            let dir = cmd
                .output
                .clone()
                .unwrap_or_else(|| PathBuf::from(DEFAULT_REPORT_DIR));
            let files = report_bundle(dna, &options)?;
            write_bundle(&dir, &files, cmd.force)?;
            let index = dir.join("index.html");
            print(&format!(
                "Report written to {}\n  {}  interactive report\n  {}  Markdown report\n  {}  analysis artifact\n  {}  Project DNA card (SVG and PNG)\n  {}  tables\n",
                dir.display(),
                index.display(),
                dir.join("report.md").display(),
                dir.join("repodna.json").display(),
                dir.join("dna-card.svg").display(),
                dir.join("data").display()
            ))
        }
        ReportFormat::Html => ctx.emit(cmd.output.as_ref(), &html_report(dna, &options), cmd.force),
        ReportFormat::Markdown => ctx.emit(
            cmd.output.as_ref(),
            &markdown_report(dna, &options),
            cmd.force,
        ),
        ReportFormat::Json => ctx.emit(
            cmd.output.as_ref(),
            &json_report(dna, options.privacy, true)?,
            cmd.force,
        ),
        ReportFormat::Csv => {
            let shared = apply_privacy(dna, options.privacy);
            let table = table.unwrap_or(CsvTable::Findings);
            ctx.emit(cmd.output.as_ref(), &csv(&shared, table), cmd.force)
        }
    }
}

/// Runs `repodna card`.
pub fn run_card(ctx: &Ctx, cmd: &CardCmd) -> Result<(), AppError> {
    let loaded = ctx.load(&cmd.target, &cmd.analysis)?;
    let dna = apply_privacy(&loaded.dna, cmd.privacy.into());
    let branding = !cmd.no_branding;
    let (bytes, default) = match cmd.format {
        CardFormat::Svg => (
            repodna_app::export::card_svg(&dna, cmd.dark, branding).into_bytes(),
            "dna-card.svg",
        ),
        CardFormat::Png => (
            repodna_app::export::card_png(&dna, cmd.dark, branding)?,
            "dna-card.png",
        ),
    };
    let path = cmd.output.clone().unwrap_or_else(|| PathBuf::from(default));
    write_file(&path, &bytes, cmd.force)?;
    print(&format!("Project DNA card written to {}", path.display()))
}

/// Runs `repodna badge`.
pub fn run_badge(ctx: &Ctx, cmd: &BadgeCmd) -> Result<(), AppError> {
    let kinds: Vec<BadgeKind> = if cmd.kind == "all" {
        BadgeKind::ALL.to_vec()
    } else {
        cmd.kind
            .split(',')
            .map(|kind| kind.trim().parse::<BadgeKind>().map_err(AppError::usage))
            .collect::<Result<_, _>>()?
    };
    let loaded = ctx.load(&cmd.target, &cmd.analysis)?;
    let dna = apply_privacy(&loaded.dna, PrivacyPreset::Share);
    let files: Vec<OutputFile> = kinds
        .iter()
        .map(|kind| {
            OutputFile::text(
                format!("{}.svg", kind.id()),
                render_badge(&badge(&dna, *kind)),
            )
        })
        .collect();
    write_bundle(&cmd.output, &files, cmd.force)?;
    let mut out = format!(
        "Badges written to {}. Markdown for your README:\n\n",
        cmd.output.display()
    );
    // README links are relative, so show the directory relative to the current one.
    let directory = std::env::current_dir()
        .ok()
        .and_then(|current| cmd.output.strip_prefix(current).ok().map(PathBuf::from))
        .filter(|relative| !relative.as_os_str().is_empty())
        .unwrap_or_else(|| cmd.output.clone());
    for kind in kinds {
        let path = format!(
            "{}/{}.svg",
            directory.to_string_lossy().replace('\\', "/"),
            kind.id()
        );
        out.push_str(&markdown_snippet(kind, &path));
        out.push('\n');
    }
    print(&out)
}

/// Runs `repodna onboarding`.
pub fn run_onboarding(ctx: &Ctx, cmd: &OnboardingCmd) -> Result<(), AppError> {
    let loaded = ctx.load(&cmd.target, &cmd.analysis)?;
    let files: Vec<OutputFile> = onboarding_bundle(&loaded.dna, cmd.privacy.into())
        .into_iter()
        .map(|file| OutputFile::text(file.name, file.content))
        .collect();
    write_bundle(&cmd.output, &files, cmd.force)?;
    print(&format!(
        "Onboarding guide written to {} ({} files; start with README.md)",
        cmd.output.display(),
        files.len()
    ))
}

/// Runs `repodna export`.
pub fn run_export(ctx: &Ctx, cmd: &ExportCmd) -> Result<(), AppError> {
    let loaded = ctx.load(&cmd.target, &cmd.analysis)?;
    let json = json_report(&loaded.dna, cmd.privacy.into(), true)?;
    let path = cmd
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from(artifact_file_name(&loaded.dna)));
    write_file(&path, json.as_bytes(), cmd.force)?;
    print(&format!(
        "Exported {} to {}. `repodna import {}` loads it on another machine.",
        loaded.dna.identity.name,
        path.display(),
        path.display()
    ))
}

/// Runs `repodna import`.
pub fn run_import(ctx: &Ctx, cmd: &ImportCmd) -> Result<(), AppError> {
    let (dna, scan, warnings) = import_artifact(&ctx.paths, &cmd.file)?;
    for warning in &warnings {
        ctx.warn(warning);
    }
    print(&format!(
        "Imported {}: a snapshot analyzed {}{}. Explore it with `repodna show {} --section summary`.",
        dna.identity.name,
        scan.generated_at.date_string(),
        super::revision_suffix(&dna),
        dna.identity.name
    ))
}

/// Runs `repodna list`.
pub fn run_list(ctx: &Ctx, cmd: &ListCmd) -> Result<(), AppError> {
    let store = ctx.paths.open_store()?;
    let repositories = store.repositories()?;
    if cmd.format == ListFormat::Json {
        let items: Vec<_> = repositories
            .iter()
            .map(|r| {
                json!({
                    "id": r.id,
                    "name": r.name,
                    "location": r.location,
                    "kind": r.kind,
                    "scans": r.scans,
                    "firstScannedAt": r.first_scanned_at,
                    "lastScannedAt": r.last_scanned_at,
                })
            })
            .collect();
        return print(&to_json(&items)?);
    }
    if repositories.is_empty() {
        return print("No stored analyses yet. `repodna analyze <path>` analyzes and stores one.");
    }
    let name_width = repositories
        .iter()
        .map(|r| r.name.chars().count())
        .max()
        .unwrap_or(4)
        .clamp(4, 40);
    let mut out = format!(
        "{}\n",
        ctx.out.bold(&format!(
            "{:<name_width$}  {:>6}  {:<10}  {}",
            "Name", "Scans", "Latest", "Location"
        ))
    );
    for repository in &repositories {
        out.push_str(&format!(
            "{:<name_width$}  {:>6}  {:<10}  {}\n",
            repository.name.chars().take(40).collect::<String>(),
            thousands(repository.scans),
            repository.last_scanned_at.date_string(),
            repository.location
        ));
    }
    print(&out)
}
