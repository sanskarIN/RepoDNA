//! The effective configuration: defaults, the user configuration, an explicit file, the
//! repository's own `repodna.toml` (which cannot enable AI, plugins, or command
//! execution), and finally command-line overrides.

use std::path::{Path, PathBuf};

use repodna_core::config::load::{ConfigLoader, LoadedConfig};
use repodna_core::config::{AnalysisProfile, Parallelism};

use crate::error::AppError;
use crate::paths::AppPaths;

/// Settings given on the command line or by a front end. They win over every file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overrides {
    /// Analysis profile.
    pub profile: Option<AnalysisProfile>,
    /// Use the per-file analysis cache.
    pub cache: Option<bool>,
    /// Replace contributor names with pseudonyms.
    pub anonymize_contributors: Option<bool>,
    /// Keep commit subjects in the artifact.
    pub include_commit_messages: Option<bool>,
    /// Allow AI providers that send data off this machine.
    pub remote_ai: Option<bool>,
    /// Plugins to enable in addition to the configured ones.
    pub plugins: Vec<String>,
    /// Extra plugin search directories.
    pub plugin_dirs: Vec<PathBuf>,
    /// Worker threads.
    pub threads: Option<usize>,
    /// Maximum commits to analyze.
    pub max_commits: Option<u64>,
}

impl Overrides {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Which configuration layers to read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigOptions {
    /// A configuration file named on the command line (trusted, like the user file).
    pub explicit: Option<PathBuf>,
    /// Skip the user configuration file.
    pub ignore_user_config: bool,
    /// Skip the repository's own configuration file.
    pub ignore_project_config: bool,
    /// Command-line overrides.
    pub overrides: Overrides,
}

/// Loads the effective configuration for a repository at `project_root` (a local
/// directory), or without repository configuration when it is `None`.
pub fn load_config(
    paths: &AppPaths,
    project_root: Option<&Path>,
    options: &ConfigOptions,
) -> Result<LoadedConfig, AppError> {
    let loader = ConfigLoader {
        user_config: (!options.ignore_user_config).then(|| paths.config_file.clone()),
        explicit: options.explicit.clone(),
        project_root: project_root.map(Path::to_path_buf),
        include_project: !options.ignore_project_config && project_root.is_some(),
    };
    let mut loaded = loader.load()?;
    let o = &options.overrides;
    let config = &mut loaded.config;
    if let Some(profile) = o.profile {
        config.analysis.profile = profile;
    }
    if let Some(cache) = o.cache {
        config.performance.cache = cache;
    }
    if let Some(anonymize) = o.anonymize_contributors {
        config.privacy.anonymize_contributors = anonymize;
    }
    if let Some(messages) = o.include_commit_messages {
        config.privacy.include_commit_messages = messages;
    }
    if let Some(remote) = o.remote_ai {
        config.privacy.remote_ai = remote;
    }
    for plugin in &o.plugins {
        if !config.plugins.enabled.contains(plugin) {
            config.plugins.enabled.push(plugin.clone());
        }
    }
    if let Some(threads) = o.threads {
        config.performance.parallelism = if threads == 0 {
            Parallelism::Auto
        } else {
            Parallelism::Threads(threads)
        };
    }
    if let Some(max) = o.max_commits {
        config.analysis.max_commits = max;
    }
    if !o.is_empty() {
        loaded.sources.push("command line".to_owned());
    }
    let problems = loaded.config.validate();
    if !problems.is_empty() {
        return Err(AppError::config(problems.join("; ")).with_hint(
            "Fix the setting in the file named above, or run `repodna config validate` to check every layer.",
        ));
    }
    Ok(loaded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_files_and_overrides() {
        let home = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        let paths = AppPaths::in_directory(home.path());
        std::fs::write(
            &paths.config_file,
            "[analysis]\nprofile = \"quick\"\n\n[plugins]\nenabled = [\"a\"]\n",
        )
        .unwrap();
        std::fs::write(
            repo.path().join("repodna.toml"),
            "[analysis]\nprofile = \"deep\"\n\n[plugins]\nenabled = [\"evil\"]\n",
        )
        .unwrap();

        let loaded = load_config(&paths, Some(repo.path()), &ConfigOptions::default()).unwrap();
        assert_eq!(loaded.config.analysis.profile, AnalysisProfile::Deep);
        assert_eq!(loaded.config.plugins.enabled, vec!["a"]);
        assert!(loaded.warnings.iter().any(|w| w.contains("plugins")));

        let options = ConfigOptions {
            ignore_project_config: true,
            overrides: Overrides {
                profile: Some(AnalysisProfile::Standard),
                plugins: vec!["b".into(), "a".into()],
                threads: Some(2),
                ..Overrides::default()
            },
            ..ConfigOptions::default()
        };
        let loaded = load_config(&paths, Some(repo.path()), &options).unwrap();
        assert_eq!(loaded.config.analysis.profile, AnalysisProfile::Standard);
        assert_eq!(loaded.config.plugins.enabled, vec!["a", "b"]);
        assert_eq!(
            loaded.config.performance.parallelism,
            Parallelism::Threads(2)
        );
        assert_eq!(loaded.sources.last().unwrap(), "command line");
        assert!(!loaded.sources.iter().any(|s| s.starts_with("project")));

        std::fs::write(&paths.config_file, "[ai]\nprovider = \"command\"\n").unwrap();
        let error = load_config(&paths, None, &ConfigOptions::default()).unwrap_err();
        assert_eq!(error.exit_code(), 4);
        assert!(error.message.contains("ai.command"));
    }
}
