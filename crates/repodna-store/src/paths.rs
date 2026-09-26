//! Where RepoDNA keeps its data and user configuration.
//!
//! `REPODNA_HOME` overrides everything: data and the user configuration file then live in
//! that one directory. Otherwise platform conventions apply:
//!
//! | Platform | Data | User configuration |
//! |---|---|---|
//! | Linux and other Unix | `$XDG_DATA_HOME/repodna` or `~/.local/share/repodna` | `$XDG_CONFIG_HOME/repodna/config.toml` or `~/.config/repodna/config.toml` |
//! | macOS | `~/Library/Application Support/RepoDNA` | the same directory, `config.toml` |
//! | Windows | `%LOCALAPPDATA%\RepoDNA` | `%APPDATA%\RepoDNA\config.toml` |

use std::path::{Path, PathBuf};

use crate::StoreError;

/// Environment variable that overrides the storage directory.
pub const HOME_VARIABLE: &str = "REPODNA_HOME";

/// Reads an environment variable, treating empty values as unset.
fn variable(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn user_home() -> Option<PathBuf> {
    variable("HOME").or_else(|| variable("USERPROFILE"))
}

/// The directory holding the database, artifacts, and cache.
pub fn data_home() -> Result<PathBuf, StoreError> {
    if let Some(home) = variable(HOME_VARIABLE) {
        return Ok(home);
    }
    let home = if cfg!(windows) {
        variable("LOCALAPPDATA").map(|dir| dir.join("RepoDNA"))
    } else if cfg!(target_os = "macos") {
        user_home().map(|home| home.join("Library/Application Support/RepoDNA"))
    } else {
        variable("XDG_DATA_HOME")
            .map(|dir| dir.join("repodna"))
            .or_else(|| user_home().map(|home| home.join(".local/share/repodna")))
    };
    home.ok_or(StoreError::NoHome)
}

/// The user configuration file (it may not exist).
pub fn config_file() -> Result<PathBuf, StoreError> {
    if let Some(home) = variable(HOME_VARIABLE) {
        return Ok(home.join("config.toml"));
    }
    let directory = if cfg!(windows) {
        variable("APPDATA").map(|dir| dir.join("RepoDNA"))
    } else if cfg!(target_os = "macos") {
        user_home().map(|home| home.join("Library/Application Support/RepoDNA"))
    } else {
        variable("XDG_CONFIG_HOME")
            .map(|dir| dir.join("repodna"))
            .or_else(|| user_home().map(|home| home.join(".config/repodna")))
    };
    directory
        .map(|dir| dir.join("config.toml"))
        .ok_or(StoreError::NoHome)
}

/// The files inside a storage directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorePaths {
    home: PathBuf,
}

impl StorePaths {
    /// Paths inside `home`.
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }

    /// Paths inside the default storage directory.
    pub fn resolve() -> Result<Self, StoreError> {
        data_home().map(Self::new)
    }

    /// The storage directory.
    pub fn home(&self) -> &Path {
        &self.home
    }

    /// The SQLite database.
    pub fn database(&self) -> PathBuf {
        self.home.join("repodna.db")
    }

    /// The directory of stored artifacts.
    pub fn artifacts(&self) -> PathBuf {
        self.home.join("artifacts")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lays_out_files_inside_the_home() {
        let paths = StorePaths::new("/data/repodna");
        assert_eq!(paths.home(), Path::new("/data/repodna"));
        assert_eq!(paths.database(), Path::new("/data/repodna/repodna.db"));
        assert_eq!(paths.artifacts(), Path::new("/data/repodna/artifacts"));
    }

    #[test]
    fn defaults_resolve_on_this_platform() {
        // Every CI platform sets a home directory, so the defaults resolve.
        let data = data_home().unwrap();
        let config = config_file().unwrap();
        assert!(data.is_absolute() || std::env::var_os(HOME_VARIABLE).is_some());
        assert_eq!(config.file_name().unwrap(), "config.toml");
    }
}
