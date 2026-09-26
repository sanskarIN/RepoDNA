//! `repodna doctor`: checks the installation, storage, tools, and configuration. It never
//! uses the network.

use repodna_app::{AppError, ErrorKind, load_config};
use repodna_core::config::AiProviderKind;
use repodna_core::model::metadata::SCHEMA_VERSION;
use repodna_git::GitRunner;
use repodna_parser::builtin_languages;
use repodna_plugin::discover;
use serde::Serialize;

use super::{Ctx, print, to_json};
use crate::cli::{AnalysisArgs, DoctorCmd};

/// Outcome of one check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Serialize)]
struct Check {
    name: &'static str,
    status: Status,
    detail: String,
    #[serde(skip)]
    kind: ErrorKind,
}

fn check(name: &'static str, status: Status, detail: impl Into<String>, kind: ErrorKind) -> Check {
    Check {
        name,
        status,
        detail: detail.into(),
        kind,
    }
}

fn checks(ctx: &Ctx) -> Vec<Check> {
    let mut checks = vec![check(
        "version",
        Status::Ok,
        format!(
            "RepoDNA {} (artifact schema {SCHEMA_VERSION}, plugin api {})",
            env!("CARGO_PKG_VERSION"),
            repodna_plugin::PLUGIN_API
        ),
        ErrorKind::Internal,
    )];

    match ctx.paths.open_store() {
        Ok(store) => {
            match store.check() {
                Ok(problems) if problems.is_empty() => checks.push(check(
                    "storage",
                    Status::Ok,
                    format!(
                        "{} is readable and consistent",
                        ctx.paths.data_home.display()
                    ),
                    ErrorKind::Storage,
                )),
                Ok(problems) => {
                    let mut detail = problems.join(" ");
                    if !detail.contains("cache repair") {
                        detail.push_str(" Run `repodna cache repair`.");
                    }
                    checks.push(check("storage", Status::Warn, detail, ErrorKind::Storage));
                }
                Err(error) => checks.push(check(
                    "storage",
                    Status::Fail,
                    format!("{error}; run `repodna cache repair`"),
                    ErrorKind::Storage,
                )),
            }
            if let Ok(stats) = store.cache_stats() {
                checks.push(check(
                    "cache",
                    Status::Ok,
                    format!(
                        "{} file analyses cached ({})",
                        stats.entries,
                        repodna_report::text::bytes(stats.bytes)
                    ),
                    ErrorKind::Storage,
                ));
            }
        }
        Err(error) => checks.push(check(
            "storage",
            Status::Fail,
            format!("{}: {}", ctx.paths.data_home.display(), error.message),
            ErrorKind::Storage,
        )),
    }

    match GitRunner::detect() {
        Ok(git) => checks.push(check(
            "git",
            Status::Ok,
            git.version().to_owned(),
            ErrorKind::External,
        )),
        Err(error) => checks.push(check(
            "git",
            Status::Warn,
            format!("{error}; history analysis and cloning URLs are unavailable"),
            ErrorKind::External,
        )),
    }

    checks.push(check(
        "languages",
        Status::Ok,
        format!(
            "{} built-in language definitions",
            builtin_languages().len()
        ),
        ErrorKind::Internal,
    ));

    let options = ctx.config_options(&AnalysisArgs::default());
    let user = load_config(&ctx.paths, None, &options);
    match &user {
        Ok(loaded) => checks.push(check(
            "configuration",
            if loaded.warnings.is_empty() {
                Status::Ok
            } else {
                Status::Warn
            },
            if loaded.warnings.is_empty() {
                format!("valid ({})", loaded.sources.join(", "))
            } else {
                loaded.warnings.join("; ")
            },
            ErrorKind::Config,
        )),
        Err(error) => checks.push(check(
            "configuration",
            Status::Fail,
            error.message.clone(),
            ErrorKind::Config,
        )),
    }
    let cwd = std::path::Path::new(".");
    if repodna_core::config::PROJECT_CONFIG_FILES
        .iter()
        .any(|name| cwd.join(name).is_file())
    {
        match load_config(&ctx.paths, Some(cwd), &options) {
            Ok(loaded) => checks.push(check(
                "repository configuration",
                if loaded.warnings.is_empty() {
                    Status::Ok
                } else {
                    Status::Warn
                },
                if loaded.warnings.is_empty() {
                    "valid".to_owned()
                } else {
                    loaded.warnings.join("; ")
                },
                ErrorKind::Config,
            )),
            Err(error) => checks.push(check(
                "repository configuration",
                Status::Fail,
                error.message,
                ErrorKind::Config,
            )),
        }
    }

    if let Ok(loaded) = &user {
        let config = &loaded.config;
        if config.ai.provider == AiProviderKind::None {
            checks.push(check(
                "ai",
                Status::Ok,
                "off (optional; every other feature works without it)",
                ErrorKind::Config,
            ));
        } else {
            match repodna_app::explain::provider(config, false) {
                Ok(provider) => checks.push(check(
                    "ai",
                    Status::Ok,
                    format!(
                        "{} / {} at {} ({}; not contacted by this check)",
                        provider.id(),
                        provider.model(),
                        provider.destination(),
                        if provider.remote() { "remote" } else { "local" }
                    ),
                    ErrorKind::Config,
                )),
                Err(error) => {
                    checks.push(check("ai", Status::Warn, error.message, ErrorKind::Config))
                }
            }
        }
        let dirs = ctx.paths.plugin_dirs(&config.plugins.directories, &[]);
        let discovery = discover(&dirs);
        let mut problems = Vec::new();
        for name in &config.plugins.enabled {
            match discovery.get(name) {
                None => problems.push(format!("`{name}` is enabled but was not found")),
                Some(plugin) if !plugin.problems.is_empty() => {
                    problems.push(format!("`{name}`: {}", plugin.problems.join("; ")));
                }
                Some(_) => {}
            }
        }
        checks.push(check(
            "plugins",
            if problems.is_empty() {
                Status::Ok
            } else {
                Status::Warn
            },
            if problems.is_empty() {
                format!(
                    "{} found, {} enabled",
                    discovery.plugins.len(),
                    config.plugins.enabled.len()
                )
            } else {
                problems.join("; ")
            },
            ErrorKind::Config,
        ));
    }

    checks.push(check(
        "network",
        Status::Ok,
        "not checked: RepoDNA uses the network only to clone URLs you pass and to reach AI providers you configure",
        ErrorKind::External,
    ));
    checks
}

/// Runs `repodna doctor`.
pub fn run(ctx: &Ctx, cmd: &DoctorCmd) -> Result<(), AppError> {
    let checks = checks(ctx);
    if cmd.json {
        print(&to_json(&checks)?)?;
    } else {
        let mut out = String::new();
        for c in &checks {
            let symbol = match c.status {
                Status::Ok => ctx.out.green("✓"),
                Status::Warn => ctx.out.yellow("!"),
                Status::Fail => ctx.out.red("✗"),
            };
            out.push_str(&format!("{symbol} {:<26}{}\n", c.name, c.detail));
        }
        print(&out)?;
    }
    let failed: Vec<&Check> = checks.iter().filter(|c| c.status == Status::Fail).collect();
    match failed.first() {
        Some(first) => Err(AppError::new(
            first.kind,
            format!(
                "{} check{} failed: {}",
                failed.len(),
                if failed.len() == 1 { "" } else { "s" },
                failed.iter().map(|c| c.name).collect::<Vec<_>>().join(", ")
            ),
        )),
        None => Ok(()),
    }
}
