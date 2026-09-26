//! The plugin manifest, `repodna-plugin.toml`.
//!
//! ```toml
//! name = "license-headers"
//! version = "1.0.0"
//! description = "Checks which source files start with an SPDX license identifier."
//! api = 1
//! command = ["python3", "license_headers.py"]
//! permissions = ["repository-files"]
//! languages = ["languages/example.toml"]
//! ```

use serde::{Deserialize, Serialize};

/// File name of a plugin manifest.
pub const MANIFEST_FILE: &str = "repodna-plugin.toml";

/// Plugin protocol version this build speaks.
pub const PLUGIN_API: u32 = 1;

/// Largest manifest accepted, in bytes.
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

/// Something a plugin may receive beyond the analysis summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Permission {
    /// Receives the path of the checkout, so it can read the repository's files.
    RepositoryFiles,
}

impl Permission {
    /// Identifier used in manifests.
    pub const fn id(self) -> &'static str {
        match self {
            Self::RepositoryFiles => "repository-files",
        }
    }

    /// What granting the permission means.
    pub const fn description(self) -> &'static str {
        match self {
            Self::RepositoryFiles => {
                "receives the checkout path and can read every file in the repository"
            }
        }
    }
}

/// A plugin's manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    /// Unique name: lowercase letters, digits, and hyphens.
    pub name: String,
    /// Plugin version.
    pub version: String,
    /// One-line description.
    #[serde(default)]
    pub description: String,
    /// Protocol version the plugin speaks; must be [`PLUGIN_API`].
    pub api: u32,
    /// Analyzer command, run without a shell from the plugin directory. A first element
    /// containing a path separator is resolved inside the plugin directory.
    #[serde(default)]
    pub command: Vec<String>,
    /// Permissions the analyzer needs.
    #[serde(default)]
    pub permissions: Vec<Permission>,
    /// Declarative language definitions shipped with the plugin, relative to its directory.
    #[serde(default)]
    pub languages: Vec<String>,
    /// Project page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    /// License of the plugin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

/// Returns `true` for names made of lowercase letters, digits, and inner hyphens.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Returns `true` for a relative path that stays inside the plugin directory.
fn contained(path: &str) -> bool {
    let unified = path.replace('\\', "/");
    !unified.is_empty()
        && !unified.starts_with('/')
        && !unified.contains(':')
        && unified.split('/').all(|part| part != "..")
}

impl PluginManifest {
    /// Parses a manifest.
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|error| error.message().to_owned())
    }

    /// Returns every problem that prevents the plugin from running.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !valid_name(&self.name) {
            problems.push(format!(
                "name `{}` must use lowercase letters, digits, and inner hyphens (at most 64 characters)",
                self.name
            ));
        }
        if self.version.trim().is_empty() {
            problems.push("version must not be empty".to_owned());
        }
        if self.api != PLUGIN_API {
            problems.push(format!(
                "api {} is not supported; this RepoDNA speaks plugin api {PLUGIN_API}",
                self.api
            ));
        }
        if self.command.is_empty() && self.languages.is_empty() {
            problems.push("declares neither a command nor languages".to_owned());
        }
        if self
            .command
            .first()
            .is_some_and(|program| program.trim().is_empty())
        {
            problems.push("command must start with a program".to_owned());
        }
        if let Some(program) = self.command.first()
            && program.contains(['/', '\\'])
            && !contained(program)
        {
            problems.push(format!(
                "command `{program}` must stay inside the plugin directory"
            ));
        }
        for language in &self.languages {
            if !contained(language) {
                problems.push(format!(
                    "language file `{language}` must be a relative path inside the plugin directory"
                ));
            }
        }
        problems
    }

    /// `true` when the manifest grants `permission`.
    pub fn has(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
name = "license-headers"
version = "1.0.0"
description = "Checks license headers."
api = 1
command = ["python3", "license_headers.py"]
permissions = ["repository-files"]
languages = ["languages/zig.toml"]
"#;

    #[test]
    fn parses_and_validates_manifests() {
        let manifest = PluginManifest::parse(VALID).unwrap();
        assert_eq!(manifest.name, "license-headers");
        assert!(manifest.has(Permission::RepositoryFiles));
        assert!(manifest.problems().is_empty());
        assert!(PluginManifest::parse("name = 1").is_err());
        assert!(
            PluginManifest::parse(&format!("{VALID}\nnetwork = true"))
                .unwrap_err()
                .contains("unknown field")
        );
    }

    #[test]
    fn reports_every_problem() {
        let manifest = PluginManifest {
            name: "Bad_Name".into(),
            version: " ".into(),
            description: String::new(),
            api: 2,
            command: vec!["../escape.sh".into()],
            permissions: Vec::new(),
            languages: vec!["/etc/lang.toml".into(), "a/../../b.toml".into()],
            homepage: None,
            license: None,
        };
        let problems = manifest.problems();
        assert_eq!(problems.len(), 6, "{problems:?}");
        let empty = PluginManifest {
            command: Vec::new(),
            languages: Vec::new(),
            ..PluginManifest::parse(VALID).unwrap()
        };
        assert!(empty.problems()[0].contains("neither"));
        assert!(valid_name("a-1"));
        assert!(!valid_name("-a"));
        assert!(!valid_name(""));
    }
}
