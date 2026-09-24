//! Repository identity: name, owner, remotes, revision, and license.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::project::LicenseInfo;
use super::structure::SizeClass;

/// A Git remote with credentials removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    /// Remote name, e.g. `origin`.
    pub name: String,
    /// URL with any user name, password, or token removed.
    pub url: String,
    /// Host name, e.g. `github.com`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Hosting provider inferred from the host: `github`, `gitlab`, `bitbucket`, or `other`.
    pub provider: String,
}

/// Who and what the repository is.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryIdentity {
    /// Repository name (from the remote URL or the directory name).
    pub name: String,
    /// Owner or namespace from the remote URL, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Remotes with credentials removed.
    #[serde(default)]
    pub remotes: Vec<RemoteInfo>,
    /// Default branch, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_branch: Option<String>,
    /// Commit analyzed, when the repository uses Git.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Detected license.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<LicenseInfo>,
    /// Primary language identifiers.
    #[serde(default)]
    pub primary_languages: Vec<String>,
    /// Purpose statement quoted from the README or a manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// `true` when Git metadata was available.
    pub is_git_repository: bool,
    /// Size class by file count.
    pub size_class: SizeClass,
}

impl RepositoryIdentity {
    /// Returns `owner/name` when the owner is known, otherwise the name.
    pub fn display_name(&self) -> String {
        match &self.owner {
            Some(owner) => format!("{owner}/{}", self.name),
            None => self.name.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_includes_owner_when_known() {
        let mut identity = RepositoryIdentity {
            name: "RepoDNA".into(),
            ..RepositoryIdentity::default()
        };
        assert_eq!(identity.display_name(), "RepoDNA");
        identity.owner = Some("sanskarIN".into());
        assert_eq!(identity.display_name(), "sanskarIN/RepoDNA");
    }
}
