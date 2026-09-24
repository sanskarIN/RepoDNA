//! Finding plugins on disk.
//!
//! A plugin is a directory holding a `repodna-plugin.toml` manifest. Each search directory
//! may be a plugin itself or contain plugin directories one level down. Discovery only
//! reads manifests; it never runs anything.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::manifest::{MANIFEST_FILE, MAX_MANIFEST_BYTES, PluginManifest};

/// A plugin found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    /// Its manifest.
    pub manifest: PluginManifest,
    /// Its directory.
    pub directory: PathBuf,
    /// Problems that prevent it from running.
    pub problems: Vec<String>,
}

/// Plugins found in a set of directories.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Discovery {
    /// Plugins, in search order.
    pub plugins: Vec<Plugin>,
    /// Manifests that could not be read and names that were shadowed.
    pub warnings: Vec<String>,
}

impl Discovery {
    /// Looks up a plugin by name.
    pub fn get(&self, name: &str) -> Option<&Plugin> {
        self.plugins
            .iter()
            .find(|plugin| plugin.manifest.name == name)
    }
}

fn read_manifest(path: &Path) -> Result<PluginManifest, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut text = String::new();
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|error| error.to_string())?;
    if text.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(format!("larger than {MAX_MANIFEST_BYTES} bytes"));
    }
    PluginManifest::parse(&text)
}

fn candidates(directory: &Path) -> Vec<PathBuf> {
    if directory.join(MANIFEST_FILE).is_file() {
        return vec![directory.to_path_buf()];
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join(MANIFEST_FILE).is_file())
        .collect();
    found.sort();
    found
}

/// Finds the plugins in `directories`. Earlier directories win when names repeat.
pub fn discover(directories: &[PathBuf]) -> Discovery {
    let mut discovery = Discovery::default();
    for directory in directories {
        for candidate in candidates(directory) {
            let manifest_path = candidate.join(MANIFEST_FILE);
            let manifest = match read_manifest(&manifest_path) {
                Ok(manifest) => manifest,
                Err(error) => {
                    discovery
                        .warnings
                        .push(format!("{}: {error}", manifest_path.display()));
                    continue;
                }
            };
            if let Some(existing) = discovery.get(&manifest.name) {
                discovery.warnings.push(format!(
                    "plugin `{}` in {} is shadowed by the one in {}",
                    manifest.name,
                    candidate.display(),
                    existing.directory.display()
                ));
                continue;
            }
            let problems = manifest.problems();
            discovery.plugins.push(Plugin {
                manifest,
                directory: candidate,
                problems,
            });
        }
    }
    discovery
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_plugin(dir: &Path, name: &str, extra: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join(MANIFEST_FILE),
            format!(
                "name = \"{name}\"\nversion = \"1.0.0\"\napi = 1\ncommand = [\"true\"]\n{extra}"
            ),
        )
        .unwrap();
    }

    #[test]
    fn discovers_plugins_and_reports_problems() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        write_plugin(&first.join("alpha"), "alpha", "");
        write_plugin(&first.join("broken"), "broken", "api = 1\n");
        write_plugin(&first.join("future"), "future-plugin", "");
        std::fs::write(
            first.join("future").join(MANIFEST_FILE),
            "name = \"future-plugin\"\nversion = \"2.0.0\"\napi = 9\ncommand = [\"x\"]\n",
        )
        .unwrap();
        write_plugin(&second.join("alpha-copy"), "alpha", "");
        write_plugin(&second.join("beta"), "beta", "");
        let single = root.path().join("single");
        write_plugin(&single, "gamma", "");

        let discovery = discover(&[first, second, single, root.path().join("missing")]);
        let names: Vec<&str> = discovery
            .plugins
            .iter()
            .map(|p| p.manifest.name.as_str())
            .collect();
        assert_eq!(names, vec!["alpha", "future-plugin", "beta", "gamma"]);
        assert!(discovery.get("future-plugin").unwrap().problems[0].contains("api 9"));
        assert!(discovery.get("alpha").unwrap().problems.is_empty());
        assert_eq!(discovery.warnings.len(), 2, "{:?}", discovery.warnings);
        assert!(
            discovery
                .warnings
                .iter()
                .any(|w| w.contains("duplicate key"))
        );
        assert!(discovery.warnings.iter().any(|w| w.contains("shadowed")));
    }
}
