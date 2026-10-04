//! Where RepoDNA keeps user configuration, plugins, stored analyses, and explanations.

use std::path::{Path, PathBuf};

use repodna_store::{Store, StorePaths};

use crate::error::AppError;

/// Resolved locations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// The user configuration file (it may not exist).
    pub config_file: PathBuf,
    /// The storage directory: database, artifacts, and cache.
    pub data_home: PathBuf,
}

impl AppPaths {
    /// Uses the platform locations, or `REPODNA_HOME` when it is set.
    pub fn resolve() -> Result<Self, AppError> {
        Ok(Self {
            config_file: repodna_store::config_file()?,
            data_home: repodna_store::data_home()?,
        })
    }

    /// Uses one directory for everything, as `REPODNA_HOME` does. Useful for tests and
    /// portable installations.
    pub fn in_directory(home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        Self {
            config_file: home.join("config.toml"),
            data_home: home,
        }
    }

    /// The directory holding the user configuration file.
    pub fn config_dir(&self) -> &Path {
        self.config_file.parent().unwrap_or(Path::new("."))
    }

    /// The user plugin directory.
    pub fn plugin_dir(&self) -> PathBuf {
        self.config_dir().join("plugins")
    }

    /// Where generated explanations are cached.
    pub fn explanation_dir(&self) -> PathBuf {
        self.data_home.join("explanations")
    }

    /// Plugin search directories: command-line directories first, then the user plugin
    /// directory, then directories from the configuration.
    pub fn plugin_dirs(&self, configured: &[String], extra: &[PathBuf]) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = extra.to_vec();
        dirs.push(self.plugin_dir());
        for dir in configured {
            let path = PathBuf::from(dir);
            let path = if path.is_relative() {
                self.config_dir().join(path)
            } else {
                path
            };
            if !dirs.contains(&path) {
                dirs.push(path);
            }
        }
        dirs
    }

    /// Opens (creating if needed) the local store.
    pub fn open_store(&self) -> Result<Store, AppError> {
        Ok(Store::open(StorePaths::new(&self.data_home))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_locations_from_one_home() {
        let paths = AppPaths::in_directory("/tmp/rdna");
        assert_eq!(paths.config_file, PathBuf::from("/tmp/rdna/config.toml"));
        assert_eq!(paths.plugin_dir(), PathBuf::from("/tmp/rdna/plugins"));
        assert_eq!(
            paths.explanation_dir(),
            PathBuf::from("/tmp/rdna/explanations")
        );
        let dirs = paths.plugin_dirs(
            &["extra".into(), "/abs/plugins".into()],
            &[PathBuf::from("cli")],
        );
        assert_eq!(
            dirs,
            vec![
                PathBuf::from("cli"),
                PathBuf::from("/tmp/rdna/plugins"),
                PathBuf::from("/tmp/rdna/extra"),
                PathBuf::from("/abs/plugins"),
            ]
        );
    }
}
