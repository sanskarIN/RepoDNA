//! Layered configuration loading with a trust boundary for repository configuration.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{Config, SuppressionRule};
use crate::error::{CoreError, Result};

/// File names searched in the repository root, in order.
pub const PROJECT_CONFIG_FILES: [&str; 2] = ["repodna.toml", ".repodna.toml"];

/// Settings a repository configuration file is never allowed to change, because they
/// execute code, contact the network, or send data elsewhere.
const UNTRUSTED_TABLES: [&str; 3] = ["ai", "plugins", "execution"];
const UNTRUSTED_PRIVACY_KEYS: [&str; 2] = ["remote_ai", "telemetry"];

/// Loads configuration from its layers.
#[derive(Debug, Clone, Default)]
pub struct ConfigLoader {
    /// User-level configuration file (trusted). Ignored when it does not exist.
    pub user_config: Option<PathBuf>,
    /// Explicit configuration file given on the command line (trusted). Must exist.
    pub explicit: Option<PathBuf>,
    /// Repository root searched for [`PROJECT_CONFIG_FILES`] (untrusted).
    pub project_root: Option<PathBuf>,
    /// Set to `false` to ignore repository configuration entirely.
    pub include_project: bool,
}

/// The effective configuration and how it was assembled.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedConfig {
    /// The merged configuration.
    pub config: Config,
    /// Layers that contributed, in order (e.g. `defaults`, `user: config.toml`,
    /// `project: repodna.toml`). Only file names are recorded, never directories, because
    /// sources are stored in artifacts that may be shared.
    pub sources: Vec<String>,
    /// Settings that were ignored, with the reason.
    pub warnings: Vec<String>,
}

impl ConfigLoader {
    /// Creates a loader that reads repository configuration from `project_root`.
    pub fn for_project(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: Some(project_root.into()),
            include_project: true,
            ..Self::default()
        }
    }

    /// Returns the repository configuration file that would be used, if one exists.
    pub fn project_config_path(&self) -> Option<PathBuf> {
        let root = self.project_root.as_ref()?;
        PROJECT_CONFIG_FILES
            .iter()
            .map(|name| root.join(name))
            .find(|path| path.is_file())
    }

    /// Loads and merges every layer, then validates the result.
    pub fn load(&self) -> Result<LoadedConfig> {
        let mut merged = toml::Table::new();
        let mut suppressions: Vec<SuppressionRule> = Vec::new();
        let mut ignore_patterns: Vec<String> = Vec::new();
        let mut sources = vec!["defaults".to_owned()];
        let mut warnings = Vec::new();

        if let Some(path) = &self.user_config
            && path.is_file()
        {
            let layer = read_layer(path, "user configuration")?;
            absorb(
                layer,
                &mut merged,
                &mut suppressions,
                &mut ignore_patterns,
                "user configuration",
            );
            sources.push(format!("user: {}", file_label(path)));
        }

        if let Some(path) = &self.explicit {
            if !path.is_file() {
                return Err(CoreError::config(
                    path.display().to_string(),
                    "the configuration file passed with --config does not exist",
                ));
            }
            let origin = file_label(path);
            let layer = read_layer(path, &origin)?;
            absorb(
                layer,
                &mut merged,
                &mut suppressions,
                &mut ignore_patterns,
                &origin,
            );
            sources.push(format!("explicit: {}", file_label(path)));
        }

        if self.include_project
            && let Some(path) = self.project_config_path()
        {
            let origin = file_label(&path);
            let mut layer = read_layer(&path, &origin)?;
            strip_untrusted(&mut layer, &origin, &mut warnings);
            absorb(
                layer,
                &mut merged,
                &mut suppressions,
                &mut ignore_patterns,
                &origin,
            );
            sources.push(format!("project: {origin}"));
        }

        let mut config = Config::deserialize(toml::Value::Table(merged))
            .map_err(|error| CoreError::config("merged configuration", error.to_string()))?;
        config.suppressions = suppressions;
        config.ignore.patterns = ignore_patterns;

        let problems = config.validate();
        if !problems.is_empty() {
            return Err(CoreError::config("configuration", problems.join("; ")));
        }
        Ok(LoadedConfig {
            config,
            sources,
            warnings,
        })
    }
}

fn file_label(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Reads one layer and validates it on its own, so errors name the file they came from.
fn read_layer(path: &Path, origin: &str) -> Result<toml::Table> {
    let text = std::fs::read_to_string(path).map_err(|source| CoreError::io(path, source))?;
    let table: toml::Table = text
        .parse()
        .map_err(|error: toml::de::Error| CoreError::config(origin, error.to_string()))?;
    Config::deserialize(toml::Value::Table(table.clone()))
        .map_err(|error| CoreError::config(origin, error.to_string()))?;
    Ok(table)
}

/// Removes settings that repository configuration may not change and records a warning.
fn strip_untrusted(layer: &mut toml::Table, origin: &str, warnings: &mut Vec<String>) {
    for table in UNTRUSTED_TABLES {
        if layer.remove(table).is_some() {
            warnings.push(format!(
                "Ignored [{table}] in {origin}: repository configuration cannot enable AI providers, plugins, or command execution. Move these settings to your user configuration."
            ));
        }
    }
    if let Some(toml::Value::Table(privacy)) = layer.get_mut("privacy") {
        for key in UNTRUSTED_PRIVACY_KEYS {
            if privacy.remove(key).is_some() {
                warnings.push(format!(
                    "Ignored privacy.{key} in {origin}: only your user configuration can change it."
                ));
            }
        }
    }
}

/// Merges a layer: suppressions and ignore patterns accumulate; everything else overrides.
fn absorb(
    mut layer: toml::Table,
    merged: &mut toml::Table,
    suppressions: &mut Vec<SuppressionRule>,
    ignore_patterns: &mut Vec<String>,
    origin: &str,
) {
    if let Some(value) = layer.remove("suppress")
        && let Ok(rules) = Vec::<SuppressionRule>::deserialize(value)
    {
        for mut rule in rules {
            rule.source = Some(origin.to_owned());
            suppressions.push(rule);
        }
    }
    if let Some(toml::Value::Table(ignore)) = layer.get_mut("ignore")
        && let Some(value) = ignore.remove("patterns")
        && let Ok(patterns) = Vec::<String>::deserialize(value)
    {
        for pattern in patterns {
            if !ignore_patterns.contains(&pattern) {
                ignore_patterns.push(pattern);
            }
        }
    }
    deep_merge(merged, layer);
}

fn deep_merge(base: &mut toml::Table, overlay: toml::Table) {
    for (key, value) in overlay {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(existing)), toml::Value::Table(incoming)) => {
                deep_merge(existing, incoming);
            }
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AiProviderKind, AnalysisProfile};

    fn write(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn defaults_when_no_files_exist() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = ConfigLoader::for_project(dir.path()).load().unwrap();
        assert_eq!(loaded.config, Config::default());
        assert_eq!(loaded.sources, vec!["defaults"]);
    }

    #[test]
    fn project_layer_overrides_user_layer() {
        let dir = tempfile::tempdir().unwrap();
        let user = write(
            dir.path(),
            "user.toml",
            "[analysis]\nprofile = \"deep\"\n[thresholds]\nlarge_file_lines = 700\n",
        );
        write(
            dir.path(),
            "repodna.toml",
            "[thresholds]\nlarge_file_lines = 900\n",
        );
        let loader = ConfigLoader {
            user_config: Some(user),
            ..ConfigLoader::for_project(dir.path())
        };
        let loaded = loader.load().unwrap();
        assert_eq!(loaded.config.analysis.profile, AnalysisProfile::Deep);
        assert_eq!(loaded.config.thresholds.large_file_lines, 900);
        assert_eq!(
            loaded.sources,
            vec!["defaults", "user: user.toml", "project: repodna.toml"]
        );
    }

    #[test]
    fn repository_configuration_cannot_enable_risky_features() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "repodna.toml",
            r#"
[ai]
provider = "command"
command = ["curl", "https://example.invalid"]

[plugins]
enabled = ["evil"]

[execution]
allow_build_commands = true

[privacy]
remote_ai = true
anonymize_contributors = true
"#,
        );
        let loaded = ConfigLoader::for_project(dir.path()).load().unwrap();
        assert_eq!(loaded.config.ai.provider, AiProviderKind::None);
        assert!(loaded.config.plugins.enabled.is_empty());
        assert!(!loaded.config.execution.allow_build_commands);
        assert!(!loaded.config.privacy.remote_ai);
        assert!(
            loaded.config.privacy.anonymize_contributors,
            "harmless keys still apply"
        );
        assert_eq!(loaded.warnings.len(), 4);
    }

    #[test]
    fn user_configuration_may_enable_ai() {
        let dir = tempfile::tempdir().unwrap();
        let user = write(
            dir.path(),
            "config.toml",
            "[ai]\nprovider = \"command\"\ncommand = [\"ollama\", \"run\", \"llama3.2\"]\n",
        );
        let loader = ConfigLoader {
            user_config: Some(user),
            ..ConfigLoader::default()
        };
        let loaded = loader.load().unwrap();
        assert_eq!(loaded.config.ai.provider, AiProviderKind::Command);
    }

    #[test]
    fn suppressions_and_ignores_accumulate_with_their_origin() {
        let dir = tempfile::tempdir().unwrap();
        let user = write(
            dir.path(),
            "user.toml",
            "[ignore]\npatterns = [\"scratch/\"]\n[[suppress]]\nrule = \"quality.*\"\nreason = \"personal preference\"\n",
        );
        write(
            dir.path(),
            ".repodna.toml",
            "[ignore]\npatterns = [\"fixtures/\"]\n[[suppress]]\npath = \"tests/**\"\nreason = \"fixtures\"\n",
        );
        let loader = ConfigLoader {
            user_config: Some(user),
            ..ConfigLoader::for_project(dir.path())
        };
        let config = loader.load().unwrap().config;
        assert_eq!(config.ignore.patterns, vec!["scratch/", "fixtures/"]);
        assert_eq!(config.suppressions.len(), 2);
        assert_eq!(
            config.suppressions[0].source.as_deref(),
            Some("user configuration")
        );
        assert_eq!(
            config.suppressions[1].source.as_deref(),
            Some(".repodna.toml")
        );
    }

    #[test]
    fn errors_name_the_offending_file() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "repodna.toml",
            "[analysis]\nprofile = \"turbo\"\n",
        );
        let error = ConfigLoader::for_project(dir.path()).load().unwrap_err();
        let message = error.to_string();
        assert!(message.contains("repodna.toml"), "{message}");
        assert!(message.contains("turbo"), "{message}");
    }

    #[test]
    fn project_configuration_can_be_disabled() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "repodna.toml",
            "[analysis]\nprofile = \"quick\"\n",
        );
        let loader = ConfigLoader {
            include_project: false,
            ..ConfigLoader::for_project(dir.path())
        };
        assert_eq!(
            loader.load().unwrap().config.analysis.profile,
            AnalysisProfile::Standard
        );
    }

    #[test]
    fn missing_explicit_file_is_an_error() {
        let loader = ConfigLoader {
            explicit: Some(PathBuf::from("/definitely/not/here.toml")),
            ..ConfigLoader::default()
        };
        assert!(loader.load().is_err());
    }
}
