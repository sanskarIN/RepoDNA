//! Housekeeping: `init`, `config`, `cache`, `clean`, `version`, and `completions`.

use std::path::Path;

use clap::CommandFactory;
use repodna_app::{AppError, load_config};
use repodna_core::config::CONFIG_TEMPLATE;
use repodna_core::model::metadata::SCHEMA_VERSION;
use repodna_report::text::bytes;
use repodna_store::{ScanRecord, StorePaths};
use serde_json::json;

use super::{Ctx, print, to_json};
use crate::cli::{
    AnalysisArgs, CacheAction, CacheCmd, CleanCmd, Cli, CompletionsCmd, ConfigAction, ConfigCmd,
    InitCmd, SchemaCmd, SchemaKind, VersionCmd,
};
use crate::term::thousands;

/// Template written by `repodna config init`.
pub const USER_CONFIG_TEMPLATE: &str = r#"# RepoDNA user configuration.
# Documentation: https://github.com/sanskarIN/RepoDNA/blob/main/docs/configuration.md
#
# These settings apply to every analysis. A repository's own repodna.toml is applied on
# top of them, but it can never enable plugins or command execution: only this file (or a
# file passed with --config) can.

[analysis]
# quick | standard | deep | history-only | architecture-only | dependencies-only | security-only
profile = "standard"

[privacy]
# RepoDNA never collects telemetry; this setting exists to make that explicit.
telemetry = false
# Replace contributor names with pseudonyms in artifacts.
anonymize_contributors = false
# Keep commit subject lines in artifacts.
include_commit_messages = true

[performance]
# Reuse per-file analysis results between runs of unchanged files.
cache = true

[plugins]
# Plugins run only when enabled by name here or with --plugin. See docs/plugins.md.
enabled = []
"#;

/// Runs `repodna init`.
pub fn run_init(cmd: &InitCmd) -> Result<(), AppError> {
    if !cmd.directory.is_dir() {
        return Err(AppError::input(format!(
            "{} is not a directory",
            cmd.directory.display()
        )));
    }
    let path = cmd.directory.join("repodna.toml");
    if path.exists() && !cmd.force {
        return Err(
            AppError::input(format!("{} already exists", path.display()))
                .with_hint("Pass --force to replace it."),
        );
    }
    repodna_core::io::write_atomic(&path, CONFIG_TEMPLATE.as_bytes())?;
    print(&format!(
        "Wrote {}. It only tunes what is analyzed; commit it so everyone gets the same results.",
        path.display()
    ))
}

/// Runs `repodna config`.
pub fn run_config(ctx: &Ctx, cmd: &ConfigCmd) -> Result<(), AppError> {
    let options = ctx.config_options(&AnalysisArgs::default());
    match &cmd.action {
        ConfigAction::Path => print(&format!(
            "{}{}",
            ctx.paths.config_file.display(),
            if ctx.paths.config_file.exists() {
                ""
            } else {
                " (does not exist yet; `repodna config init` creates it)"
            }
        )),
        ConfigAction::Show { directory, json } => {
            let loaded = load_config(&ctx.paths, Some(directory), &options)?;
            if *json {
                return print(&to_json(&json!({
                    "sources": loaded.sources,
                    "warnings": loaded.warnings,
                    "config": loaded.config,
                }))?);
            }
            let toml = loaded
                .config
                .to_toml()
                .map_err(|error| AppError::internal(error.to_string()))?;
            let mut out = format!("# Sources: {}\n", loaded.sources.join(", "));
            for warning in &loaded.warnings {
                out.push_str(&format!("# Ignored: {warning}\n"));
            }
            out.push('\n');
            out.push_str(&toml);
            print(&out)
        }
        ConfigAction::Init { force } => {
            let path = &ctx.paths.config_file;
            if path.exists() && !force {
                return Err(
                    AppError::input(format!("{} already exists", path.display()))
                        .with_hint("Pass --force to replace it."),
                );
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    AppError::input(format!("cannot create {}: {error}", parent.display()))
                })?;
            }
            repodna_core::io::write_atomic(path, USER_CONFIG_TEMPLATE.as_bytes())?;
            print(&format!("Wrote {}", path.display()))
        }
        ConfigAction::Validate { directory } => {
            let loaded = load_config(&ctx.paths, Some(directory), &options)?;
            let mut out = format!(
                "{} Configuration is valid. Sources: {}\n",
                ctx.out.green("✓"),
                loaded.sources.join(", ")
            );
            for warning in &loaded.warnings {
                out.push_str(&format!("  {} {warning}\n", ctx.out.yellow("ignored:")));
            }
            print(&out)
        }
    }
}

/// Runs `repodna cache`.
pub fn run_cache(ctx: &Ctx, cmd: &CacheCmd) -> Result<(), AppError> {
    let action = cmd
        .action
        .as_ref()
        .unwrap_or(&CacheAction::Stats { json: false });
    match action {
        CacheAction::Stats { json } => {
            let store = ctx.paths.open_store()?;
            let stats = store.stats()?;
            if *json {
                return print(&to_json(&json!({
                    "home": stats.home,
                    "databaseBytes": stats.database_bytes,
                    "repositories": stats.repositories,
                    "scans": stats.scans,
                    "artifacts": stats.artifacts,
                    "artifactBytes": stats.artifact_bytes,
                    "cacheEntries": stats.cache.entries,
                    "cacheBytes": stats.cache.bytes,
                }))?);
            }
            print(&format!(
                "Storage: {}\n  Repositories         {}\n  Stored analyses      {} ({})\n  Database             {}\n  File analysis cache  {} entries ({})\n",
                stats.home.display(),
                thousands(stats.repositories),
                thousands(stats.scans),
                bytes(stats.artifact_bytes),
                bytes(stats.database_bytes),
                thousands(stats.cache.entries),
                bytes(stats.cache.bytes)
            ))
        }
        CacheAction::Clear => {
            let store = ctx.paths.open_store()?;
            let cleared = store.clear_cache()?;
            print(&format!(
                "Cleared {} cached file analyses ({}). Stored analyses were kept.",
                thousands(cleared.entries),
                bytes(cleared.bytes)
            ))
        }
        CacheAction::Repair => {
            let report = repodna_store::repair(&StorePaths::new(&ctx.paths.data_home))?;
            let mut out = String::new();
            if let Some(moved) = &report.moved_database {
                out.push_str(&format!(
                    "The database was damaged and was moved to {}.\n",
                    moved.display()
                ));
            }
            out.push_str(&format!(
                "{} Re-indexed {} stored analyses.",
                ctx.out.green("✓"),
                report.indexed
            ));
            for skipped in &report.skipped {
                out.push_str(&format!("\n  {} {skipped}", ctx.out.yellow("skipped:")));
            }
            print(&out)
        }
        CacheAction::Reset { yes } => {
            if !yes {
                return Err(AppError::usage(
                    "`repodna cache reset` deletes the database and cache; pass --yes to confirm",
                )
                .with_hint(
                    "Stored artifacts are kept; `repodna cache repair` re-indexes them afterwards.",
                ));
            }
            let report = repodna_store::reset(&StorePaths::new(&ctx.paths.data_home))?;
            print(&format!(
                "Removed {} database files. Your repositories were not touched.",
                report.removed.len()
            ))
        }
    }
}

/// Runs `repodna clean`.
pub fn run_clean(ctx: &Ctx, cmd: &CleanCmd) -> Result<(), AppError> {
    let store = ctx.paths.open_store()?;
    let repositories = if cmd.all {
        store.repositories()?
    } else {
        let query = cmd.target.as_deref().unwrap_or_default();
        let canonical = Path::new(query)
            .canonicalize()
            .map_or_else(|_| query.to_owned(), |p| p.to_string_lossy().into_owned());
        let repository = store
            .find_repository(&canonical)?
            .or(store.find_repository(query)?)
            .ok_or_else(|| {
                AppError::input(format!("no stored repository matches `{query}`"))
                    .with_hint("`repodna list` shows stored repositories.")
            })?;
        vec![repository]
    };
    let mut victims: Vec<(String, ScanRecord)> = Vec::new();
    for repository in &repositories {
        for scan in store
            .scans(&repository.id, usize::MAX)?
            .into_iter()
            .skip(cmd.keep)
        {
            victims.push((repository.name.clone(), scan));
        }
    }
    if victims.is_empty() {
        return print("Nothing to delete.");
    }
    let total: u64 = victims.iter().map(|(_, scan)| scan.artifact_bytes).sum();
    if !cmd.yes {
        let mut out = format!(
            "Would delete {} stored analyses ({}):\n",
            victims.len(),
            bytes(total)
        );
        for (name, scan) in &victims {
            out.push_str(&format!(
                "  {name}  {}  {}\n",
                scan.generated_at.date_string(),
                &scan.id[..scan.id.len().min(12)]
            ));
        }
        out.push_str(
            "Run again with --yes to delete them. Repositories themselves are never touched.",
        );
        return print(&out);
    }
    for (_, scan) in &victims {
        store.delete_scan(&scan.id)?;
    }
    if cmd.keep == 0 {
        for repository in &repositories {
            store.delete_repository(&repository.id)?;
        }
    }
    print(&format!(
        "Deleted {} stored analyses ({}).",
        victims.len(),
        bytes(total)
    ))
}

/// The commit RepoDNA was built from, when the build recorded it.
pub(crate) fn build_commit() -> Option<&'static str> {
    option_env!("REPODNA_BUILD_COMMIT").filter(|commit| !commit.is_empty())
}

/// Runs `repodna version`.
pub fn run_version(cmd: &VersionCmd) -> Result<(), AppError> {
    let version = env!("CARGO_PKG_VERSION");
    if cmd.json {
        return print(&to_json(&json!({
            "name": "RepoDNA",
            "version": version,
            "schemaVersion": SCHEMA_VERSION,
            "pluginApi": repodna_plugin::PLUGIN_API,
            "commit": build_commit(),
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "license": "Apache-2.0",
            "homepage": "https://github.com/sanskarIN/RepoDNA",
        }))?);
    }
    print(&format!(
        "RepoDNA {version}\n  artifact schema  {SCHEMA_VERSION}\n  plugin api       {}\n  build            {}\n  platform         {}-{}\n  license          Apache-2.0\n  homepage         https://github.com/sanskarIN/RepoDNA\nMade by the Sanskar.",
        repodna_plugin::PLUGIN_API,
        build_commit().unwrap_or("local build"),
        std::env::consts::OS,
        std::env::consts::ARCH
    ))
}

/// Runs `repodna schema`.
pub fn run_schema(cmd: &SchemaCmd) -> Result<(), AppError> {
    let schema = match cmd.kind {
        SchemaKind::Artifact => repodna_core::schema::artifact_schema(),
        SchemaKind::Config => repodna_core::schema::config_schema(),
    };
    print(&to_json(&schema)?)
}

/// Runs `repodna completions`.
pub fn run_completions(cmd: &CompletionsCmd) -> Result<(), AppError> {
    let mut command = Cli::command();
    let mut out = Vec::new();
    clap_complete::generate(cmd.shell, &mut command, "repodna", &mut out);
    print(&String::from_utf8_lossy(&out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::config::Config;

    #[test]
    fn user_template_parses_and_validates() {
        let config: Config = toml::from_str(USER_CONFIG_TEMPLATE).unwrap();
        assert!(config.validate().is_empty());
        assert!(!config.privacy.telemetry);
    }
}
