//! The store: repositories, scans, metrics, and findings, with artifacts on disk.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use repodna_core::config::AnalysisProfile;
use repodna_core::hash::stable_id;
use repodna_core::io::{read_artifact, write_artifact};
use repodna_core::model::artifact::RepositoryDna;
use repodna_core::model::metadata::InputKind;
use repodna_core::severity::Severity;
use repodna_core::time::Timestamp;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::schema::{configure, migrate};
use crate::{StoreError, StorePaths};

/// Where an analyzed repository lives. It identifies the repository across scans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryLocation {
    /// How the repository was provided.
    pub kind: InputKind,
    /// The canonical local path, or the URL without credentials.
    pub location: String,
}

impl RepositoryLocation {
    /// A stable identifier. Local directories with and without Git metadata share one
    /// identity, so switching profiles does not split a repository's history.
    pub fn id(&self) -> String {
        let group = match self.kind {
            InputKind::LocalDirectory | InputKind::GitRepository => "local",
            InputKind::GitUrl => "url",
            InputKind::Archive => "archive",
            InputKind::Artifact => "artifact",
        };
        stable_id(&["repository", group, self.location.as_str()])
    }
}

/// A repository known to the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryRecord {
    /// Identifier.
    pub id: String,
    /// Name from the latest scan.
    pub name: String,
    /// Canonical local path or URL.
    pub location: String,
    /// Input kind of the latest scan.
    pub kind: InputKind,
    /// Generation time of the earliest stored scan.
    pub first_scanned_at: Timestamp,
    /// Generation time of the latest stored scan.
    pub last_scanned_at: Timestamp,
    /// Stored scans.
    pub scans: u64,
}

/// A stored scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRecord {
    /// Analysis identifier.
    pub id: String,
    /// Repository identifier.
    pub repository_id: String,
    /// When the analysis was generated.
    pub generated_at: Timestamp,
    /// Profile used.
    pub profile: AnalysisProfile,
    /// Commit analyzed, when known.
    pub revision: Option<String>,
    /// DNA hash of the snapshot.
    pub dna_hash: String,
    /// Artifact schema version.
    pub schema_version: String,
    /// RepoDNA version that produced the artifact.
    pub tool_version: String,
    /// Files discovered.
    pub files: u64,
    /// Code lines.
    pub code_lines: u64,
    /// Commits in the history.
    pub commits: u64,
    /// Contributors in the history.
    pub contributors: u64,
    /// Active critical findings.
    pub critical: u64,
    /// Active warnings.
    pub warning: u64,
    /// Active attention signals.
    pub attention: u64,
    /// Active informational findings.
    pub info: u64,
    /// Suppressed findings.
    pub suppressed: u64,
    /// Artifact path relative to the storage directory.
    pub artifact: String,
    /// Artifact size in bytes.
    pub artifact_bytes: u64,
}

/// A finding as indexed in the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFinding {
    /// Finding identifier.
    pub id: String,
    /// Rule identifier.
    pub rule: String,
    /// Severity.
    pub severity: Severity,
    /// Title.
    pub title: String,
    /// Suppressed by configuration.
    pub suppressed: bool,
}

/// Findings that appeared or disappeared between two scans.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FindingChanges {
    /// Present in the newer scan only.
    pub added: Vec<StoredFinding>,
    /// Present in the older scan only.
    pub resolved: Vec<StoredFinding>,
}

/// Local storage. It is safe to share between threads.
#[derive(Debug)]
pub struct Store {
    pub(crate) connection: Mutex<Connection>,
    pub(crate) paths: StorePaths,
    /// Cache keys read since the last bookkeeping pass.
    pub(crate) touched: Mutex<Vec<String>>,
    /// Size limit of the analysis cache in compressed bytes.
    pub(crate) cache_limit: u64,
}

pub(crate) fn severity_id(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "critical",
        Severity::Warning => "warning",
        Severity::Attention => "attention",
        Severity::Info => "info",
    }
}

fn parse_severity(id: &str) -> Severity {
    match id {
        "critical" => Severity::Critical,
        "warning" => Severity::Warning,
        "attention" => Severity::Attention,
        _ => Severity::Info,
    }
}

fn kind_id(kind: InputKind) -> &'static str {
    match kind {
        InputKind::LocalDirectory => "local-directory",
        InputKind::GitRepository => "git-repository",
        InputKind::GitUrl => "git-url",
        InputKind::Archive => "archive",
        InputKind::Artifact => "artifact",
    }
}

fn parse_kind(id: &str) -> InputKind {
    match id {
        "git-repository" => InputKind::GitRepository,
        "git-url" => InputKind::GitUrl,
        "archive" => InputKind::Archive,
        "artifact" => InputKind::Artifact,
        _ => InputKind::LocalDirectory,
    }
}

fn count(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn unsigned(row: &Row<'_>, column: &str) -> rusqlite::Result<u64> {
    Ok(u64::try_from(row.get::<_, i64>(column)?).unwrap_or(0))
}

const SCAN_COLUMNS: &str = "id, repository_id, generated_at, profile, revision, dna_hash, schema_version, tool_version, files, code_lines, commits, contributors, critical, warning, attention, info, suppressed, artifact, artifact_bytes";

fn scan_from_row(row: &Row<'_>) -> rusqlite::Result<ScanRecord> {
    let profile: String = row.get("profile")?;
    Ok(ScanRecord {
        id: row.get("id")?,
        repository_id: row.get("repository_id")?,
        generated_at: Timestamp::from_unix(row.get("generated_at")?),
        profile: profile.parse().unwrap_or_default(),
        revision: row.get("revision")?,
        dna_hash: row.get("dna_hash")?,
        schema_version: row.get("schema_version")?,
        tool_version: row.get("tool_version")?,
        files: unsigned(row, "files")?,
        code_lines: unsigned(row, "code_lines")?,
        commits: unsigned(row, "commits")?,
        contributors: unsigned(row, "contributors")?,
        critical: unsigned(row, "critical")?,
        warning: unsigned(row, "warning")?,
        attention: unsigned(row, "attention")?,
        info: unsigned(row, "info")?,
        suppressed: unsigned(row, "suppressed")?,
        artifact: row.get("artifact")?,
        artifact_bytes: unsigned(row, "artifact_bytes")?,
    })
}

fn repository_from_row(row: &Row<'_>) -> rusqlite::Result<RepositoryRecord> {
    let kind: String = row.get("kind")?;
    Ok(RepositoryRecord {
        id: row.get("id")?,
        name: row.get("name")?,
        location: row.get("location")?,
        kind: parse_kind(&kind),
        first_scanned_at: Timestamp::from_unix(row.get("first_scanned_at")?),
        last_scanned_at: Timestamp::from_unix(row.get("last_scanned_at")?),
        scans: unsigned(row, "scans")?,
    })
}

fn finding_from_row(row: &Row<'_>) -> rusqlite::Result<StoredFinding> {
    let severity: String = row.get("severity")?;
    Ok(StoredFinding {
        id: row.get("finding_id")?,
        rule: row.get("rule")?,
        severity: parse_severity(&severity),
        title: row.get("title")?,
        suppressed: row.get("suppressed")?,
    })
}

const REPOSITORY_QUERY: &str = "SELECT r.id, r.name, r.location, r.kind, r.first_scanned_at, r.last_scanned_at, (SELECT COUNT(*) FROM scans s WHERE s.repository_id = r.id) AS scans FROM repositories r";

impl Store {
    /// Opens (creating if needed) the store in `paths`, upgrading its schema.
    pub fn open(paths: StorePaths) -> Result<Self, StoreError> {
        fs::create_dir_all(paths.home()).map_err(|source| StoreError::io(paths.home(), source))?;
        let mut connection = Connection::open(paths.database())?;
        configure(&connection)?;
        migrate(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
            paths,
            touched: Mutex::new(Vec::new()),
            cache_limit: crate::cache::DEFAULT_CACHE_LIMIT,
        })
    }

    /// Sets the size limit of the analysis cache in compressed bytes.
    #[must_use]
    pub fn with_cache_limit(mut self, bytes: u64) -> Self {
        self.cache_limit = bytes;
        self
    }

    /// The storage locations.
    pub fn paths(&self) -> &StorePaths {
        &self.paths
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Connection> {
        // A panic while holding the lock cannot leave SQLite inconsistent (statements are
        // atomic), so a poisoned lock is still safe to use.
        self.connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Absolute path of a stored artifact.
    pub fn artifact_path(&self, scan: &ScanRecord) -> PathBuf {
        self.paths.home().join(&scan.artifact)
    }

    /// Stores an analysis: writes the artifact atomically and indexes the scan, its metrics,
    /// and its findings. Storing the same analysis again replaces the earlier entry.
    pub fn record(
        &self,
        dna: &RepositoryDna,
        location: &RepositoryLocation,
    ) -> Result<ScanRecord, StoreError> {
        let repository_id = location.id();
        let metadata = &dna.analysis_metadata;
        let scan_id = if metadata.id.is_empty() {
            stable_id(&[
                dna.fingerprint.dna_hash.as_str(),
                &metadata.generated_at.to_rfc3339(),
            ])
        } else {
            metadata.id.clone()
        };
        let artifact = format!("artifacts/{repository_id}/{scan_id}.json");
        let path = self.paths.home().join(&artifact);
        write_artifact(&path, dna, false)?;
        let artifact_bytes = fs::metadata(&path)
            .map_err(|source| StoreError::io(&path, source))?
            .len();
        let counts = dna.finding_counts();
        let record = ScanRecord {
            id: scan_id,
            repository_id,
            generated_at: metadata.generated_at,
            profile: metadata.profile,
            revision: metadata.revision.clone(),
            dna_hash: dna.fingerprint.dna_hash.clone(),
            schema_version: dna.schema_version.clone(),
            tool_version: dna.tool.version.clone(),
            files: dna.structure.total_files,
            code_lines: dna.structure.code_lines,
            commits: dna.git.commit_count,
            contributors: dna.git.contributors.len() as u64,
            critical: counts.critical as u64,
            warning: counts.warning as u64,
            attention: counts.attention as u64,
            info: counts.info as u64,
            suppressed: counts.suppressed as u64,
            artifact,
            artifact_bytes,
        };
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        let generated = record.generated_at.unix();
        transaction.execute(
            "INSERT INTO repositories (id, name, location, kind, first_scanned_at, last_scanned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(id) DO UPDATE SET
                 name = excluded.name,
                 kind = excluded.kind,
                 first_scanned_at = MIN(first_scanned_at, excluded.first_scanned_at),
                 last_scanned_at = MAX(last_scanned_at, excluded.last_scanned_at)",
            params![
                record.repository_id,
                dna.identity.name,
                location.location,
                kind_id(location.kind),
                generated
            ],
        )?;
        transaction.execute("DELETE FROM metrics WHERE scan_id = ?1", [&record.id])?;
        transaction.execute("DELETE FROM findings WHERE scan_id = ?1", [&record.id])?;
        transaction.execute("DELETE FROM scans WHERE id = ?1", [&record.id])?;
        transaction.execute(
            &format!(
                "INSERT INTO scans ({SCAN_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)"
            ),
            params![
                record.id,
                record.repository_id,
                generated,
                record.profile.id(),
                record.revision,
                record.dna_hash,
                record.schema_version,
                record.tool_version,
                i64::try_from(record.files).unwrap_or(i64::MAX),
                i64::try_from(record.code_lines).unwrap_or(i64::MAX),
                i64::try_from(record.commits).unwrap_or(i64::MAX),
                i64::try_from(record.contributors).unwrap_or(i64::MAX),
                count(counts.critical),
                count(counts.warning),
                count(counts.attention),
                count(counts.info),
                count(counts.suppressed),
                record.artifact,
                i64::try_from(record.artifact_bytes).unwrap_or(i64::MAX),
            ],
        )?;
        {
            let mut insert = transaction.prepare(
                "INSERT OR REPLACE INTO metrics (scan_id, metric, value) VALUES (?1, ?2, ?3)",
            )?;
            for metric in &dna.metrics.raw {
                insert.execute(params![record.id, metric.id, metric.value])?;
            }
            let mut insert = transaction.prepare(
                "INSERT OR REPLACE INTO findings (scan_id, finding_id, rule, severity, title, suppressed) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for finding in &dna.findings {
                insert.execute(params![
                    record.id,
                    finding.id,
                    finding.rule,
                    severity_id(finding.severity),
                    finding.title,
                    finding.is_suppressed()
                ])?;
            }
        }
        transaction.commit()?;
        Ok(record)
    }

    /// Every repository, most recently scanned first.
    pub fn repositories(&self) -> Result<Vec<RepositoryRecord>, StoreError> {
        let connection = self.lock();
        let mut statement = connection.prepare(&format!(
            "{REPOSITORY_QUERY} ORDER BY r.last_scanned_at DESC, r.id"
        ))?;
        let rows = statement.query_map([], repository_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Finds a repository by identifier, location, or (case-insensitive) name. When several
    /// repositories share a name, the most recently scanned one is returned.
    pub fn find_repository(&self, query: &str) -> Result<Option<RepositoryRecord>, StoreError> {
        let connection = self.lock();
        let mut statement = connection.prepare(&format!(
            "{REPOSITORY_QUERY} WHERE r.id = ?1 OR r.location = ?1 OR lower(r.name) = lower(?1)
             ORDER BY (r.id = ?1) DESC, (r.location = ?1) DESC, r.last_scanned_at DESC LIMIT 1"
        ))?;
        Ok(statement
            .query_row([query], repository_from_row)
            .optional()?)
    }

    /// Scans of a repository, newest first.
    pub fn scans(&self, repository_id: &str, limit: usize) -> Result<Vec<ScanRecord>, StoreError> {
        let connection = self.lock();
        let mut statement = connection.prepare(&format!(
            "SELECT {SCAN_COLUMNS} FROM scans WHERE repository_id = ?1
             ORDER BY generated_at DESC, id DESC LIMIT ?2"
        ))?;
        let rows = statement.query_map(params![repository_id, count(limit)], scan_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// A scan by identifier (or a unique prefix of at least four characters).
    pub fn scan(&self, id: &str) -> Result<Option<ScanRecord>, StoreError> {
        let connection = self.lock();
        let mut statement =
            connection.prepare(&format!("SELECT {SCAN_COLUMNS} FROM scans WHERE id = ?1"))?;
        if let Some(scan) = statement.query_row([id], scan_from_row).optional()? {
            return Ok(Some(scan));
        }
        if id.len() < 4 {
            return Ok(None);
        }
        let mut statement = connection.prepare(&format!(
            "SELECT {SCAN_COLUMNS} FROM scans WHERE substr(id, 1, length(?1)) = ?1 LIMIT 2"
        ))?;
        let matches: Vec<ScanRecord> = statement
            .query_map([id], scan_from_row)?
            .collect::<Result<_, _>>()?;
        Ok(match <[ScanRecord; 1]>::try_from(matches) {
            Ok([scan]) => Some(scan),
            Err(_) => None,
        })
    }

    /// Loads a stored artifact.
    pub fn load(&self, scan: &ScanRecord) -> Result<RepositoryDna, StoreError> {
        Ok(read_artifact(&self.artifact_path(scan))?.artifact)
    }

    /// The values of one metric across a repository's scans, oldest first.
    pub fn metric_series(
        &self,
        repository_id: &str,
        metric: &str,
    ) -> Result<Vec<(Timestamp, f64)>, StoreError> {
        let connection = self.lock();
        let mut statement = connection.prepare(
            "SELECT s.generated_at, m.value FROM metrics m JOIN scans s ON s.id = m.scan_id
             WHERE s.repository_id = ?1 AND m.metric = ?2 ORDER BY s.generated_at, s.id",
        )?;
        let rows = statement.query_map(params![repository_id, metric], |row| {
            Ok((Timestamp::from_unix(row.get(0)?), row.get(1)?))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Findings added and resolved between an older and a newer scan.
    pub fn finding_changes(&self, older: &str, newer: &str) -> Result<FindingChanges, StoreError> {
        let connection = self.lock();
        let query = "SELECT finding_id, rule, severity, title, suppressed FROM findings
             WHERE scan_id = ?1 AND finding_id NOT IN (SELECT finding_id FROM findings WHERE scan_id = ?2)
             ORDER BY CASE severity WHEN 'critical' THEN 0 WHEN 'warning' THEN 1 WHEN 'attention' THEN 2 ELSE 3 END, rule, finding_id";
        let mut statement = connection.prepare(query)?;
        let added = statement
            .query_map([newer, older], finding_from_row)?
            .collect::<Result<_, _>>()?;
        let resolved = statement
            .query_map([older, newer], finding_from_row)?
            .collect::<Result<_, _>>()?;
        Ok(FindingChanges { added, resolved })
    }

    /// Removes a scan and its artifact. Returns `false` when it did not exist.
    pub fn delete_scan(&self, id: &str) -> Result<bool, StoreError> {
        let Some(scan) = self.scan(id)? else {
            return Ok(false);
        };
        self.lock()
            .execute("DELETE FROM scans WHERE id = ?1", [&scan.id])?;
        remove_file(&self.artifact_path(&scan))?;
        Ok(true)
    }

    /// Removes a repository, its scans, and their artifacts. Returns `false` when it did
    /// not exist.
    pub fn delete_repository(&self, id: &str) -> Result<bool, StoreError> {
        let removed = self
            .lock()
            .execute("DELETE FROM repositories WHERE id = ?1", [id])?;
        let directory = self.paths.artifacts().join(id);
        if directory.is_dir() {
            fs::remove_dir_all(&directory).map_err(|source| StoreError::io(&directory, source))?;
        }
        Ok(removed > 0)
    }
}

fn remove_file(path: &Path) -> Result<(), StoreError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(StoreError::io(path, source)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repodna_core::confidence::Confidence;
    use repodna_core::finding::{Finding, FindingCategory};
    use repodna_core::metric::Metric;
    use repodna_core::model::identity::RepositoryIdentity;
    use repodna_core::model::metadata::AnalysisMetadata;

    pub(crate) fn artifact(id: &str, day: u32, findings: &[&str]) -> RepositoryDna {
        let mut dna = RepositoryDna::new(
            RepositoryIdentity {
                name: "widget".into(),
                ..RepositoryIdentity::default()
            },
            AnalysisMetadata::default(),
        );
        dna.analysis_metadata.id = id.into();
        dna.analysis_metadata.generated_at = Timestamp::from_ymd(2024, 1, day).unwrap();
        dna.fingerprint.dna_hash = format!("rdna1-{id}");
        dna.structure.total_files = u64::from(day);
        dna.metrics.raw = vec![Metric::new(
            "structure.files",
            "Files",
            f64::from(day),
            "files",
        )];
        dna.findings = findings
            .iter()
            .map(|subject| {
                Finding::new(
                    "quality.large-file",
                    subject,
                    FindingCategory::Complexity,
                    Severity::Attention,
                    Confidence::High,
                    format!("{subject} is large"),
                )
            })
            .collect();
        dna
    }

    fn location() -> RepositoryLocation {
        RepositoryLocation {
            kind: InputKind::GitRepository,
            location: "/work/widget".into(),
        }
    }

    #[test]
    fn records_and_queries_scans() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(StorePaths::new(dir.path())).unwrap();
        let first = store
            .record(&artifact("aaaa1111", 1, &["a.rs", "b.rs"]), &location())
            .unwrap();
        let second = store
            .record(&artifact("bbbb2222", 5, &["b.rs", "c.rs"]), &location())
            .unwrap();
        assert!(store.artifact_path(&first).is_file());

        let repositories = store.repositories().unwrap();
        assert_eq!(repositories.len(), 1);
        assert_eq!(repositories[0].scans, 2);
        assert_eq!(repositories[0].name, "widget");
        assert_eq!(repositories[0].first_scanned_at, first.generated_at);
        assert_eq!(repositories[0].last_scanned_at, second.generated_at);
        assert_eq!(
            store.find_repository("WIDGET").unwrap().unwrap().id,
            location().id()
        );
        assert_eq!(
            store.find_repository("/work/widget").unwrap().unwrap().id,
            location().id()
        );
        assert!(store.find_repository("other").unwrap().is_none());

        let scans = store.scans(&location().id(), 10).unwrap();
        let ids: Vec<&str> = scans.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["bbbb2222", "aaaa1111"]);
        assert_eq!(scans[0].attention, 2);
        assert_eq!(store.scan("bbbb").unwrap().unwrap().id, "bbbb2222");
        assert!(store.scan("bb").unwrap().is_none());
        assert_eq!(
            store.load(&scans[0]).unwrap().analysis_metadata.id,
            "bbbb2222"
        );

        let series = store
            .metric_series(&location().id(), "structure.files")
            .unwrap();
        assert_eq!(
            series.iter().map(|(_, v)| *v).collect::<Vec<_>>(),
            vec![1.0, 5.0]
        );

        let changes = store.finding_changes("aaaa1111", "bbbb2222").unwrap();
        assert_eq!(changes.added.len(), 1);
        assert_eq!(changes.added[0].title, "c.rs is large");
        assert_eq!(changes.resolved[0].title, "a.rs is large");
    }

    #[test]
    fn replaces_repeated_scans_and_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(StorePaths::new(dir.path())).unwrap();
        store
            .record(&artifact("aaaa1111", 1, &["a.rs"]), &location())
            .unwrap();
        let again = store
            .record(&artifact("aaaa1111", 1, &["a.rs", "b.rs"]), &location())
            .unwrap();
        assert_eq!(store.scans(&location().id(), 10).unwrap().len(), 1);
        assert_eq!(again.attention, 2);

        assert!(store.delete_scan("aaaa1111").unwrap());
        assert!(!store.artifact_path(&again).exists());
        assert!(!store.delete_scan("aaaa1111").unwrap());
        store
            .record(&artifact("cccc3333", 2, &[]), &location())
            .unwrap();
        assert!(store.delete_repository(&location().id()).unwrap());
        assert!(store.repositories().unwrap().is_empty());
        assert!(!store.paths().artifacts().join(location().id()).exists());
    }

    #[test]
    fn local_directories_keep_one_identity_across_profiles() {
        let plain = RepositoryLocation {
            kind: InputKind::LocalDirectory,
            location: "/work/widget".into(),
        };
        assert_eq!(plain.id(), location().id());
        let url = RepositoryLocation {
            kind: InputKind::GitUrl,
            location: "/work/widget".into(),
        };
        assert_ne!(url.id(), location().id());
    }

    #[test]
    fn reopening_keeps_data() {
        let dir = tempfile::tempdir().unwrap();
        {
            let store = Store::open(StorePaths::new(dir.path())).unwrap();
            store
                .record(&artifact("aaaa1111", 1, &[]), &location())
                .unwrap();
        }
        let store = Store::open(StorePaths::new(dir.path())).unwrap();
        assert_eq!(store.repositories().unwrap().len(), 1);
    }
}
