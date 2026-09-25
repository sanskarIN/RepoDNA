//! `repodna plugins`: list, inspect, enable, disable, and check plugins. Enabling and
//! disabling edit only the `[plugins]` table of your user configuration, preserving the
//! rest of the file and its comments.

use std::path::Path;

use repodna_app::{AppError, load_config};
use repodna_plugin::{MANIFEST_FILE, Plugin, PluginManifest, discover};

use super::{Ctx, print};
use crate::cli::{AnalysisArgs, PluginsAction, PluginsCmd};

fn enabled(ctx: &Ctx) -> Result<Vec<String>, AppError> {
    let options = ctx.config_options(&AnalysisArgs::default());
    Ok(load_config(&ctx.paths, None, &options)?
        .config
        .plugins
        .enabled)
}

fn directories(ctx: &Ctx) -> Result<Vec<std::path::PathBuf>, AppError> {
    let options = ctx.config_options(&AnalysisArgs::default());
    let config = load_config(&ctx.paths, None, &options)?.config;
    Ok(ctx.paths.plugin_dirs(&config.plugins.directories, &[]))
}

fn kind(manifest: &PluginManifest) -> &'static str {
    match (manifest.command.is_empty(), manifest.languages.is_empty()) {
        (false, false) => "analyzer + languages",
        (false, true) => "analyzer",
        _ => "languages",
    }
}

fn describe(ctx: &Ctx, plugin: &Plugin, is_enabled: bool) -> String {
    let m = &plugin.manifest;
    let mut out = format!(
        "{} {}  ({}, {})\n",
        ctx.out.bold(&m.name),
        m.version,
        kind(m),
        if is_enabled { "enabled" } else { "not enabled" }
    );
    if !m.description.is_empty() {
        out.push_str(&format!("  {}\n", m.description));
    }
    out.push_str(&format!("  directory    {}\n", plugin.directory.display()));
    if !m.command.is_empty() {
        out.push_str(&format!("  command      {}\n", m.command.join(" ")));
    }
    for language in &m.languages {
        out.push_str(&format!("  language     {language}\n"));
    }
    if m.permissions.is_empty() {
        out.push_str("  permissions  none: receives the file list and summary only\n");
    }
    for permission in &m.permissions {
        out.push_str(&format!(
            "  permission   {}: {}\n",
            permission.id(),
            permission.description()
        ));
    }
    if let Some(license) = &m.license {
        out.push_str(&format!("  license      {license}\n"));
    }
    for problem in &plugin.problems {
        out.push_str(&format!("  {} {problem}\n", ctx.out.red("problem:")));
    }
    out
}

/// Adds or removes `name` in the user configuration's `plugins.enabled` list.
fn set_enabled(path: &Path, name: &str, enable: bool) -> Result<bool, AppError> {
    let text = if path.exists() {
        std::fs::read_to_string(path)
            .map_err(|error| AppError::config(format!("cannot read {}: {error}", path.display())))?
    } else {
        String::new()
    };
    let mut document: toml_edit::DocumentMut = text.parse().map_err(|error| {
        AppError::config(format!("{} is not valid TOML: {error}", path.display()))
    })?;
    let plugins = document["plugins"].or_insert(toml_edit::table());
    let Some(table) = plugins.as_table_like_mut() else {
        return Err(AppError::config(format!(
            "`plugins` in {} is not a table",
            path.display()
        )));
    };
    if !table.contains_key("enabled") {
        table.insert("enabled", toml_edit::value(toml_edit::Array::new()));
    }
    let Some(list) = table
        .get_mut("enabled")
        .and_then(toml_edit::Item::as_array_mut)
    else {
        return Err(AppError::config(format!(
            "`plugins.enabled` in {} is not a list",
            path.display()
        )));
    };
    let present = list.iter().any(|value| value.as_str() == Some(name));
    let changed = match (enable, present) {
        (true, false) => {
            list.push(name);
            true
        }
        (false, true) => {
            list.retain(|value| value.as_str() != Some(name));
            true
        }
        _ => false,
    };
    if changed {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                AppError::config(format!("cannot create {}: {error}", parent.display()))
            })?;
        }
        repodna_core::io::write_atomic(path, document.to_string().as_bytes())?;
    }
    Ok(changed)
}

/// Checks that a program can be started: a file inside the plugin directory, or a program
/// on PATH.
fn program_available(directory: &Path, program: &str) -> bool {
    if program.contains(['/', '\\']) {
        return directory.join(program).is_file();
    }
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let extensions: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".to_owned())
            .split(';')
            .map(str::to_owned)
            .chain(std::iter::once(String::new()))
            .collect()
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&path).any(|dir| {
        extensions
            .iter()
            .any(|ext| dir.join(format!("{program}{ext}")).is_file())
    })
}

/// Runs `repodna plugins`.
pub fn run(ctx: &Ctx, cmd: &PluginsCmd) -> Result<(), AppError> {
    match cmd.action.as_ref().unwrap_or(&PluginsAction::List) {
        PluginsAction::Path => print(&ctx.paths.plugin_dir().display().to_string()),
        PluginsAction::List => {
            let dirs = directories(ctx)?;
            let discovery = discover(&dirs);
            let active = enabled(ctx)?;
            let mut out = String::new();
            if discovery.plugins.is_empty() {
                out.push_str(&format!(
                    "No plugins found. Install one by copying its directory into {} (see docs/plugins.md).\n",
                    ctx.paths.plugin_dir().display()
                ));
            }
            for plugin in &discovery.plugins {
                out.push_str(&describe(
                    ctx,
                    plugin,
                    active.contains(&plugin.manifest.name),
                ));
                out.push('\n');
            }
            for name in active.iter().filter(|name| discovery.get(name).is_none()) {
                out.push_str(&format!(
                    "{} `{name}` is enabled but was not found\n",
                    ctx.out.yellow("warning:")
                ));
            }
            for warning in &discovery.warnings {
                out.push_str(&format!("{} {warning}\n", ctx.out.yellow("warning:")));
            }
            print(out.trim_end())
        }
        PluginsAction::Show { name } => {
            let discovery = discover(&directories(ctx)?);
            let plugin = discovery.get(name).ok_or_else(|| {
                AppError::input(format!("no plugin named `{name}` was found"))
                    .with_hint("`repodna plugins list` shows the plugins that were found.")
            })?;
            print(&describe(ctx, plugin, enabled(ctx)?.contains(name)))
        }
        PluginsAction::Enable { name } | PluginsAction::Disable { name } => {
            let enable = matches!(cmd.action, Some(PluginsAction::Enable { .. }));
            if !repodna_plugin::manifest::valid_name(name) {
                return Err(AppError::usage(format!(
                    "`{name}` is not a valid plugin name"
                )));
            }
            let found = discover(&directories(ctx)?).get(name).is_some();
            let changed = set_enabled(&ctx.paths.config_file, name, enable)?;
            let mut out = match (enable, changed) {
                (true, true) => format!("Enabled `{name}` in {}.", ctx.paths.config_file.display()),
                (true, false) => format!("`{name}` was already enabled."),
                (false, true) => {
                    format!("Disabled `{name}` in {}.", ctx.paths.config_file.display())
                }
                (false, false) => format!("`{name}` was not enabled."),
            };
            if enable && !found {
                out.push_str(&format!(
                    "\n{} it is not installed yet; copy it into {}.",
                    ctx.out.yellow("note:"),
                    ctx.paths.plugin_dir().display()
                ));
            }
            print(&out)
        }
        PluginsAction::Check { directory } => {
            let discovery = discover(std::slice::from_ref(directory));
            let Some(plugin) = discovery.plugins.first() else {
                let reason =
                    discovery.warnings.first().cloned().unwrap_or_else(|| {
                        format!("no {MANIFEST_FILE} in {}", directory.display())
                    });
                return Err(AppError::config(reason));
            };
            let mut problems = plugin.problems.clone();
            if let Some(program) = plugin.manifest.command.first()
                && !program_available(&plugin.directory, program)
            {
                problems.push(format!("the program `{program}` was not found"));
            }
            let config = repodna_core::config::PluginConfig {
                enabled: vec![plugin.manifest.name.clone()],
                ..repodna_core::config::PluginConfig::default()
            };
            if problems.is_empty() {
                let loaded = repodna_plugin::load_enabled(
                    &config,
                    std::slice::from_ref(&plugin.directory),
                    true,
                );
                problems.extend(loaded.warnings);
            }
            let mut out = describe(ctx, plugin, false);
            if problems.is_empty() {
                out.push_str(&format!(
                    "{} The plugin is valid. Nothing was run.",
                    ctx.out.green("✓")
                ));
                print(&out)
            } else {
                print(&out)?;
                Err(AppError::config(format!(
                    "the plugin has {} problems: {}",
                    problems.len(),
                    problems.join("; ")
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_only_the_plugin_list() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "# my settings\n[analysis]\nprofile = \"quick\" # fast\n",
        )
        .unwrap();
        assert!(set_enabled(&path, "license-headers", true).unwrap());
        assert!(!set_enabled(&path, "license-headers", true).unwrap());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# my settings"));
        assert!(text.contains("profile = \"quick\" # fast"));
        let config: repodna_core::config::Config = toml::from_str(&text).unwrap();
        assert_eq!(config.plugins.enabled, vec!["license-headers"]);
        assert!(set_enabled(&path, "license-headers", false).unwrap());
        let config: repodna_core::config::Config =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(config.plugins.enabled.is_empty());

        let fresh = dir.path().join("new/config.toml");
        assert!(set_enabled(&fresh, "zig-language", true).unwrap());
        assert!(
            std::fs::read_to_string(&fresh)
                .unwrap()
                .contains("zig-language")
        );
    }
}
