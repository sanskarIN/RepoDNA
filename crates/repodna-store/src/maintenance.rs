//! Inspecting, checking, repairing, and resetting the store.
//!
//! Nothing here touches analyzed repositories. Repair never deletes a damaged database: it
//! is renamed next to the original and a new index is built from the stored artifacts.
//! Reset removes only what RepoDNA created (the database and the artifacts), keeping the
//! user configuration file.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use repodna_core::io::read_artifact;
use repodna_core::time::Timestamp;

use crate::cache::CacheStats;
use crate::store::{REPOSITORY_FILE, parse_kind};
use crate::{RepositoryLocation, Store, StoreError, StorePaths};

/// What the store holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreStats {
    /// The storage directory.
    pub home: PathBuf,
    /// Size of the database files (including the write-ahead log).
    pub database_bytes: u64,
    /// Stored artifact files.
    pub artifacts: u64,
    /// Size of the stored artifacts.
    pub artifact_bytes: u64,
    /// Repositories indexed.
    pub repositories: u64,
    /// Scans indexed.
    pub scans: u64,
    /// The per-file analysis cache.
    pub cache: CacheStats,
}

/// Outcome of [`repair`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairReport {
    /// Where a damaged database was moved, if one was found.
    pub moved_database: Option<PathBuf>,
    /// Artifacts indexed.
    pub indexed: usize,
    /// Artifacts that could not be read.
    pub skipped: Vec<String>,
}

/// Outcome of [`reset`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResetReport {
    /// Files and directories removed.
    pub removed: Vec<PathBuf>,
}

fn database_files(paths: &StorePaths) -> Vec<PathBuf> {
    let database = paths.database();
    let mut files = vec![database.clone()];
    for suffix in ["-wal", "-shm"] {
        let mut name = database.clone().into_os_string();
        name.push(suffix);
        files.push(PathBuf::from(name));
    }
    files
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map_or(0, |metadata| metadata.len())
}

/// Stored artifact files as `(repository directory, relative artifact path)`.
fn artifact_files(paths: &StorePaths) -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    let Ok(repositories) = fs::read_dir(paths.artifacts()) else {
        return files;
    };
    for repository in repositories.flatten() {
        let directory = repository.path();
        if !directory.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".json") && name != REPOSITORY_FILE && !name.starts_with('.') {
                let relative = format!(
                    "artifacts/{}/{name}",
                    repository.file_name().to_string_lossy()
                );
                files.push((directory.clone(), relative));
            }
        }
    }
    files.sort_by(|a, b| a.1.cmp(&b.1));
    files
}

fn read_location(directory: &Path) -> Option<(RepositoryLocation, String)> {
    let text = fs::read_to_string(directory.join(REPOSITORY_FILE)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let location = RepositoryLocation {
        kind: parse_kind(value.get("kind")?.as_str()?),
        location: value.get("location")?.as_str()?.to_owned(),
    };
    let name = value.get("name")?.as_str()?.to_owned();
    Some((location, name))
}

impl Store {
    /// Summarizes what the store holds.
    pub fn stats(&self) -> Result<StoreStats, StoreError> {
        let (repositories, scans): (i64, i64) = self.lock().query_row(
            "SELECT (SELECT COUNT(*) FROM repositories), (SELECT COUNT(*) FROM scans)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let artifacts = artifact_files(&self.paths);
        Ok(StoreStats {
            home: self.paths.home().to_path_buf(),
            database_bytes: database_files(&self.paths)
                .iter()
                .map(|path| file_size(path))
                .sum(),
            artifacts: artifacts.len() as u64,
            artifact_bytes: artifacts
                .iter()
                .map(|(_, relative)| file_size(&self.paths.home().join(relative)))
                .sum(),
            repositories: u64::try_from(repositories).unwrap_or(0),
            scans: u64::try_from(scans).unwrap_or(0),
            cache: self.cache_stats()?,
        })
    }

    /// Lists problems: database integrity errors, scans whose artifact is missing, and
    /// stored artifacts that are not indexed. An empty list means the store is healthy.
    pub fn check(&self) -> Result<Vec<String>, StoreError> {
        let mut problems = Vec::new();
        let indexed: Vec<(String, String)> = {
            let connection = self.lock();
            let mut integrity = connection.prepare("PRAGMA integrity_check")?;
            for row in integrity.query_map([], |row| row.get::<_, String>(0))? {
                let row = row?;
                if row != "ok" {
                    problems.push(format!("Database integrity: {row}"));
                }
            }
            let mut scans = connection.prepare("SELECT id, artifact FROM scans ORDER BY id")?;
            scans
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<_, _>>()?
        };
        for (id, artifact) in &indexed {
            if !self.paths.home().join(artifact).is_file() {
                problems.push(format!(
                    "The artifact of scan {id} is missing ({artifact})."
                ));
            }
        }
        let known: HashSet<&str> = indexed
            .iter()
            .map(|(_, artifact)| artifact.as_str())
            .collect();
        let unindexed = artifact_files(&self.paths)
            .iter()
            .filter(|(_, relative)| !known.contains(relative.as_str()))
            .count();
        if unindexed > 0 {
            problems.push(format!(
                "{unindexed} stored artifacts are not indexed; `repodna cache repair` indexes them."
            ));
        }
        Ok(problems)
    }

    /// Indexes every stored artifact again. Returns the number indexed and the artifacts
    /// that could not be read.
    pub fn rebuild_index(&self) -> Result<(usize, Vec<String>), StoreError> {
        let mut indexed = 0;
        let mut skipped = Vec::new();
        for (directory, relative) in artifact_files(&self.paths) {
            let Some((location, _name)) = read_location(&directory) else {
                skipped.push(relative);
                continue;
            };
            match read_artifact(&self.paths.home().join(&relative)) {
                Ok(loaded) => {
                    let scan_id = Path::new(&relative)
                        .file_stem()
                        .map(|stem| stem.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let scan_id = if loaded.artifact.analysis_metadata.id.is_empty() {
                        scan_id
                    } else {
                        loaded.artifact.analysis_metadata.id.clone()
                    };
                    self.index(&loaded.artifact, &location, relative, scan_id)?;
                    indexed += 1;
                }
                Err(_) => skipped.push(relative),
            }
        }
        Ok((indexed, skipped))
    }
}

/// Repairs the store: a database that cannot be opened or fails its integrity check is
/// moved aside (never deleted), and the index is rebuilt from the stored artifacts.
pub fn repair(paths: &StorePaths) -> Result<RepairReport, StoreError> {
    let database = paths.database();
    let mut moved_database = None;
    if database.exists() {
        let healthy = match Store::open(paths.clone()) {
            Ok(store) => store.check().is_ok_and(|problems| {
                !problems
                    .iter()
                    .any(|problem| problem.starts_with("Database integrity"))
            }),
            Err(StoreError::NewerSchema { found, supported }) => {
                return Err(StoreError::NewerSchema { found, supported });
            }
            Err(_) => false,
        };
        if !healthy {
            let stamp = Timestamp::now().unix();
            for file in database_files(paths) {
                if file.exists() {
                    let mut target = file.clone().into_os_string();
                    target.push(format!(".damaged-{stamp}"));
                    let target = PathBuf::from(target);
                    fs::rename(&file, &target).map_err(|source| StoreError::io(&file, source))?;
                    if file == database {
                        moved_database = Some(target);
                    }
                }
            }
        }
    }
    let store = Store::open(paths.clone())?;
    let (indexed, skipped) = store.rebuild_index()?;
    Ok(RepairReport {
        moved_database,
        indexed,
        skipped,
    })
}

/// Removes the database and all stored artifacts. The storage directory itself and the
/// user configuration file are kept.
pub fn reset(paths: &StorePaths) -> Result<ResetReport, StoreError> {
    let mut report = ResetReport::default();
    for file in database_files(paths) {
        if file.exists() {
            fs::remove_file(&file).map_err(|source| StoreError::io(&file, source))?;
            report.removed.push(file);
        }
    }
    let artifacts = paths.artifacts();
    if artifacts.exists() {
        fs::remove_dir_all(&artifacts).map_err(|source| StoreError::io(&artifacts, source))?;
        report.removed.push(artifacts);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::artifact;
    use repodna_core::model::metadata::InputKind;

    fn location() -> RepositoryLocation {
        RepositoryLocation {
            kind: InputKind::GitRepository,
            location: "/work/widget".into(),
        }
    }

    #[test]
    fn reports_statistics_and_health() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(StorePaths::new(dir.path())).unwrap();
        store
            .record(&artifact("aaaa1111", 1, &["a.rs"]), &location())
            .unwrap();
        let stats = store.stats().unwrap();
        assert_eq!(
            (stats.repositories, stats.scans, stats.artifacts),
            (1, 1, 1)
        );
        assert!(stats.database_bytes > 0);
        assert!(stats.artifact_bytes > 0);
        assert!(store.check().unwrap().is_empty());

        let scan = store.scan("aaaa1111").unwrap().unwrap();
        fs::remove_file(store.artifact_path(&scan)).unwrap();
        let problems = store.check().unwrap();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("missing"));
    }

    #[test]
    fn repairs_a_damaged_database_from_the_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StorePaths::new(dir.path());
        {
            let store = Store::open(paths.clone()).unwrap();
            store
                .record(&artifact("aaaa1111", 1, &["a.rs"]), &location())
                .unwrap();
            store
                .record(&artifact("bbbb2222", 2, &[]), &location())
                .unwrap();
        }
        for file in database_files(&paths) {
            let _ = fs::remove_file(file);
        }
        fs::write(paths.database(), b"this is not a database").unwrap();
        assert!(Store::open(paths.clone()).is_err());

        let report = repair(&paths).unwrap();
        let moved = report.moved_database.unwrap();
        assert_eq!(fs::read(&moved).unwrap(), b"this is not a database");
        assert_eq!(report.indexed, 2);
        assert!(report.skipped.is_empty());

        let store = Store::open(paths.clone()).unwrap();
        let repositories = store.repositories().unwrap();
        assert_eq!(repositories.len(), 1);
        assert_eq!(repositories[0].location, "/work/widget");
        assert_eq!(repositories[0].name, "widget");
        assert_eq!(store.scans(&location().id(), 10).unwrap().len(), 2);
        assert!(store.check().unwrap().is_empty());

        // A healthy database is kept and simply re-indexed.
        drop(store);
        let again = repair(&paths).unwrap();
        assert!(again.moved_database.is_none());
        assert_eq!(again.indexed, 2);
    }

    #[test]
    fn reset_keeps_the_user_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StorePaths::new(dir.path());
        fs::write(dir.path().join("config.toml"), "[analysis]\n").unwrap();
        {
            let store = Store::open(paths.clone()).unwrap();
            store
                .record(&artifact("aaaa1111", 1, &[]), &location())
                .unwrap();
        }
        let report = reset(&paths).unwrap();
        assert!(report.removed.contains(&paths.database()));
        assert!(report.removed.contains(&paths.artifacts()));
        assert!(!paths.database().exists());
        assert!(dir.path().join("config.toml").is_file());
        let store = Store::open(paths).unwrap();
        assert!(store.repositories().unwrap().is_empty());
    }
}
