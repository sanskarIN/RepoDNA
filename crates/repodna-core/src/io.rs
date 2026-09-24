//! Reading and writing artifacts safely.
//!
//! Writes are atomic: data goes to a temporary file in the destination directory, is
//! flushed to disk, and is then renamed over the destination. An interrupted export can
//! therefore never leave a truncated artifact behind.
//!
//! Reads check the schema version before deserializing. Artifacts with the same major
//! version are accepted (newer minor versions only add optional fields, which are ignored);
//! older major versions are upgraded through the registered migrations; newer major
//! versions are rejected with an actionable error.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{CoreError, Result};
use crate::model::artifact::RepositoryDna;
use crate::model::metadata::{SCHEMA_MAJOR, SCHEMA_MINOR, SCHEMA_VERSION};

/// Largest artifact accepted by [`read_artifact`] (1 GiB), a guard against resource exhaustion.
pub const MAX_ARTIFACT_BYTES: u64 = 1024 * 1024 * 1024;

/// A migration upgrades a JSON document from one schema major version to the next.
type Migration = fn(&mut serde_json::Value) -> std::result::Result<(), String>;

/// Registered migrations, keyed by the major version they upgrade *from*.
///
/// Schema 1 is the first public schema, so the list is empty. When schema 2 is introduced,
/// a `(1, migrate_1_to_2)` entry keeps version-1 artifacts importable.
const MIGRATIONS: &[(u32, Migration)] = &[];

/// An artifact loaded from JSON, with compatibility notes.
#[derive(Debug, Clone)]
pub struct LoadedArtifact {
    /// The artifact, migrated to the current schema if necessary.
    pub artifact: RepositoryDna,
    /// Notes such as "written by a newer RepoDNA; unknown fields were ignored".
    pub warnings: Vec<String>,
}

/// Parses a schema version string such as `1.0` into `(major, minor)`.
pub fn parse_schema_version(version: &str) -> Option<(u32, u32)> {
    let mut parts = version.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().map_or(Some(0), |minor| minor.parse().ok())?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor))
}

/// Parses an artifact from JSON text.
pub fn parse_artifact(json: &str) -> Result<LoadedArtifact> {
    let mut value: serde_json::Value = serde_json::from_str(json)?;
    let object = value.as_object().ok_or_else(|| {
        CoreError::InvalidArtifact("the document is not a JSON object".to_owned())
    })?;
    let version = object
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            CoreError::InvalidArtifact(
                "missing schemaVersion; this does not look like a RepoDNA artifact".to_owned(),
            )
        })?
        .to_owned();
    let (mut major, minor) = parse_schema_version(&version).ok_or_else(|| {
        CoreError::InvalidArtifact(format!("malformed schemaVersion {version:?}"))
    })?;

    let mut warnings = Vec::new();
    if major > SCHEMA_MAJOR {
        return Err(CoreError::UnsupportedSchema {
            found: version,
            supported: SCHEMA_VERSION.to_owned(),
        });
    }
    while major < SCHEMA_MAJOR {
        let migration = MIGRATIONS
            .iter()
            .find(|(from, _)| *from == major)
            .map(|(_, migration)| *migration)
            .ok_or_else(|| CoreError::UnsupportedSchema {
                found: version.clone(),
                supported: SCHEMA_VERSION.to_owned(),
            })?;
        migration(&mut value).map_err(CoreError::InvalidArtifact)?;
        major += 1;
        warnings.push(format!(
            "Upgraded the artifact from schema {} to schema {major}.",
            major - 1
        ));
    }
    if major == SCHEMA_MAJOR && minor > SCHEMA_MINOR {
        warnings.push(format!(
            "The artifact uses schema {version}, newer than this build ({SCHEMA_VERSION}); fields this version does not understand were ignored."
        ));
    }
    let artifact: RepositoryDna = serde_json::from_value(value)?;
    Ok(LoadedArtifact { artifact, warnings })
}

/// Reads and parses an artifact file.
pub fn read_artifact(path: &Path) -> Result<LoadedArtifact> {
    let metadata = fs::metadata(path).map_err(|source| CoreError::io(path, source))?;
    if metadata.len() > MAX_ARTIFACT_BYTES {
        return Err(CoreError::InvalidArtifact(format!(
            "{} is larger than the {} MiB artifact limit",
            path.display(),
            MAX_ARTIFACT_BYTES / (1024 * 1024)
        )));
    }
    let text = fs::read_to_string(path).map_err(|source| CoreError::io(path, source))?;
    parse_artifact(&text)
}

/// Serializes an artifact to JSON.
pub fn to_json(artifact: &RepositoryDna, pretty: bool) -> Result<String> {
    Ok(if pretty {
        serde_json::to_string_pretty(artifact)?
    } else {
        serde_json::to_string(artifact)?
    })
}

/// Writes an artifact atomically.
pub fn write_artifact(path: &Path, artifact: &RepositoryDna, pretty: bool) -> Result<()> {
    let mut json = to_json(artifact, pretty)?;
    json.push('\n');
    write_atomic(path, json.as_bytes())
}

/// Writes bytes to `path` atomically, creating parent directories as needed.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => std::env::current_dir().map_err(|source| CoreError::io(path, source))?,
    };
    fs::create_dir_all(&parent).map_err(|source| CoreError::io(&parent, source))?;
    let file_name = path.file_name().map_or_else(
        || "output".into(),
        |name| name.to_string_lossy().into_owned(),
    );
    let temporary = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));

    let result = (|| -> std::io::Result<()> {
        let mut file = File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if let Err(source) = result {
        // Best effort: never leave temporary files behind.
        let _ = fs::remove_file(&temporary);
        return Err(CoreError::io(path, source));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::identity::RepositoryIdentity;
    use crate::model::metadata::AnalysisMetadata;

    fn artifact() -> RepositoryDna {
        RepositoryDna::new(
            RepositoryIdentity {
                name: "sample".into(),
                ..RepositoryIdentity::default()
            },
            AnalysisMetadata::default(),
        )
    }

    #[test]
    fn parses_schema_versions() {
        assert_eq!(parse_schema_version("1.0"), Some((1, 0)));
        assert_eq!(parse_schema_version("2"), Some((2, 0)));
        assert_eq!(parse_schema_version("1.x"), None);
        assert_eq!(parse_schema_version("1.0.0"), None);
    }

    #[test]
    fn round_trips_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("scan.repodna.json");
        write_artifact(&path, &artifact(), true).unwrap();
        let loaded = read_artifact(&path).unwrap();
        assert_eq!(loaded.artifact, artifact());
        assert!(loaded.warnings.is_empty());
        let leftovers: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(
            leftovers.len(),
            1,
            "no temporary files remain: {leftovers:?}"
        );
    }

    #[test]
    fn accepts_newer_minor_versions_with_a_warning() {
        let mut value = serde_json::to_value(artifact()).unwrap();
        value["schemaVersion"] = "1.7".into();
        value["someFutureSection"] = serde_json::json!({"added": true});
        let loaded = parse_artifact(&value.to_string()).unwrap();
        assert_eq!(loaded.warnings.len(), 1);
        assert!(loaded.warnings[0].contains("1.7"));
    }

    #[test]
    fn rejects_newer_major_versions() {
        let mut value = serde_json::to_value(artifact()).unwrap();
        value["schemaVersion"] = "2.0".into();
        let error = parse_artifact(&value.to_string()).unwrap_err();
        assert!(matches!(error, CoreError::UnsupportedSchema { .. }));
    }

    #[test]
    fn rejects_documents_that_are_not_artifacts() {
        assert!(matches!(
            parse_artifact("[1, 2, 3]").unwrap_err(),
            CoreError::InvalidArtifact(_)
        ));
        assert!(matches!(
            parse_artifact("{\"name\": \"x\"}").unwrap_err(),
            CoreError::InvalidArtifact(_)
        ));
        assert!(matches!(
            parse_artifact("not json").unwrap_err(),
            CoreError::Json(_)
        ));
    }

    #[test]
    fn older_majors_without_migrations_are_rejected() {
        let mut value = serde_json::to_value(artifact()).unwrap();
        value["schemaVersion"] = "0.9".into();
        assert!(matches!(
            parse_artifact(&value.to_string()).unwrap_err(),
            CoreError::UnsupportedSchema { .. }
        ));
    }

    #[test]
    fn atomic_writes_replace_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.txt");
        write_atomic(&path, b"first").unwrap();
        write_atomic(&path, b"second").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
    }
}
