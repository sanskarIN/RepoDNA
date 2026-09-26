//! RepoDNA plugins.
//!
//! A plugin is a directory with a `repodna-plugin.toml` manifest. It can ship declarative
//! language definitions (pure data, compiled with linear-time regular expressions), an
//! analyzer command that speaks a versioned JSON protocol over standard input and output,
//! or both. Plugins run only when enabled by name in the user configuration or on the
//! command line; a repository's own configuration can never enable them.
//!
//! Analyzers run without a shell, from the plugin directory, with a minimal environment
//! (API keys and tokens are not passed), a time limit, and an output limit. Their findings
//! and metrics are validated and namespaced under `plugin.<name>.`. Only analyzers granted
//! the `repository-files` permission receive the checkout path. These constraints are not
//! an operating-system sandbox; a plugin runs with the user's permissions.

pub mod discover;
pub mod extension;
pub mod manifest;
pub mod process;
pub mod protocol;

use std::path::{Path, PathBuf};

use repodna_core::config::PluginConfig;
use repodna_engine::Extension;
use repodna_parser::{LanguageSpec, parse_definition};

pub use discover::{Discovery, Plugin, discover};
pub use extension::PluginExtension;
pub use manifest::{MANIFEST_FILE, PLUGIN_API, Permission, PluginManifest};

/// Largest language definition read, in bytes.
const MAX_LANGUAGE_BYTES: u64 = 256 * 1024;

/// Plugins prepared for one analysis.
#[derive(Default)]
pub struct LoadedPlugins {
    /// Analyzer extensions, in the order they were enabled.
    pub extensions: Vec<Box<dyn Extension>>,
    /// Language definitions shipped by enabled plugins.
    pub languages: Vec<LanguageSpec>,
    /// Names of the enabled plugins that loaded.
    pub active: Vec<String>,
    /// Problems with enabled plugins; each disabled that plugin or one of its parts.
    pub warnings: Vec<String>,
}

fn read_language(directory: &Path, relative: &str) -> Result<LanguageSpec, String> {
    use std::io::Read;
    let path = directory.join(relative);
    let file = std::fs::File::open(&path).map_err(|error| format!("{relative}: {error}"))?;
    let mut text = String::new();
    file.take(MAX_LANGUAGE_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|error| format!("{relative}: {error}"))?;
    if text.len() as u64 > MAX_LANGUAGE_BYTES {
        return Err(format!(
            "{relative}: larger than {MAX_LANGUAGE_BYTES} bytes"
        ));
    }
    parse_definition(&text, relative).map_err(|error| error.to_string())
}

/// Loads the plugins enabled in `config` from `directories` (searched in order).
pub fn load_enabled(
    config: &PluginConfig,
    directories: &[PathBuf],
    reproducible: bool,
) -> LoadedPlugins {
    let mut loaded = LoadedPlugins::default();
    if config.enabled.is_empty() {
        return loaded;
    }
    let discovery = discover(directories);
    let limits = extension::limits(config.timeout_seconds, config.max_output_bytes);
    for name in &config.enabled {
        if loaded.active.contains(name) {
            continue;
        }
        let Some(plugin) = discovery.get(name) else {
            loaded.warnings.push(format!(
                "plugin `{name}` is enabled but was not found in: {}",
                directories
                    .iter()
                    .map(|dir| dir.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            continue;
        };
        if !plugin.problems.is_empty() {
            loaded.warnings.push(format!(
                "plugin `{name}` was not loaded: {}",
                plugin.problems.join("; ")
            ));
            continue;
        }
        for relative in &plugin.manifest.languages {
            match read_language(&plugin.directory, relative) {
                Ok(spec) => loaded.languages.push(spec),
                Err(error) => loaded
                    .warnings
                    .push(format!("plugin `{name}`: language {error}")),
            }
        }
        if !plugin.manifest.command.is_empty() {
            loaded.extensions.push(Box::new(PluginExtension::new(
                plugin.clone(),
                limits,
                reproducible,
            )));
        }
        loaded.active.push(name.clone());
    }
    loaded
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZIG: &str = r#"id = "zig"
name = "Zig"
kind = "programming"
extensions = ["zig"]
line_comments = ["//"]
"#;

    #[test]
    fn loads_enabled_plugins_and_reports_problems() {
        let root = tempfile::tempdir().unwrap();
        let good = root.path().join("zig-support");
        std::fs::create_dir_all(good.join("languages")).unwrap();
        std::fs::write(good.join("languages/zig.toml"), ZIG).unwrap();
        std::fs::write(
            good.join(MANIFEST_FILE),
            "name = \"zig-support\"\nversion = \"1.0.0\"\napi = 1\nlanguages = [\"languages/zig.toml\", \"languages/missing.toml\"]\n",
        )
        .unwrap();
        let bad = root.path().join("bad");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(
            bad.join(MANIFEST_FILE),
            "name = \"bad\"\nversion = \"1\"\napi = 7\ncommand = [\"x\"]\n",
        )
        .unwrap();

        let config = PluginConfig {
            enabled: vec!["zig-support".into(), "bad".into(), "absent".into()],
            ..PluginConfig::default()
        };
        let loaded = load_enabled(&config, &[root.path().to_path_buf()], true);
        assert_eq!(loaded.active, vec!["zig-support"]);
        assert_eq!(loaded.languages.len(), 1);
        assert_eq!(loaded.languages[0].id, "zig");
        assert!(loaded.extensions.is_empty());
        assert_eq!(loaded.warnings.len(), 3, "{:?}", loaded.warnings);
        assert!(loaded.warnings.iter().any(|w| w.contains("missing.toml")));
        assert!(loaded.warnings.iter().any(|w| w.contains("api 7")));
        assert!(loaded.warnings.iter().any(|w| w.contains("`absent`")));

        let none = load_enabled(&PluginConfig::default(), &[root.path().to_path_buf()], true);
        assert!(none.active.is_empty() && none.warnings.is_empty());
    }
}
