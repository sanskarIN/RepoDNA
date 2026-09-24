//! Database schema and migrations.
//!
//! The schema version is kept in SQLite's `user_version`. Each migration runs in its own
//! transaction together with the version update, so a failed upgrade rolls back and leaves
//! the previous version's data untouched.

use rusqlite::Connection;

use crate::StoreError;

/// Migrations in order: entry `i` upgrades the schema from version `i` to `i + 1`.
const MIGRATIONS: &[&str] = &[r"
CREATE TABLE repositories (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    location TEXT NOT NULL,
    kind TEXT NOT NULL,
    first_scanned_at INTEGER NOT NULL,
    last_scanned_at INTEGER NOT NULL
);

CREATE TABLE scans (
    id TEXT PRIMARY KEY,
    repository_id TEXT NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    generated_at INTEGER NOT NULL,
    profile TEXT NOT NULL,
    revision TEXT,
    dna_hash TEXT NOT NULL,
    schema_version TEXT NOT NULL,
    tool_version TEXT NOT NULL,
    files INTEGER NOT NULL,
    code_lines INTEGER NOT NULL,
    commits INTEGER NOT NULL,
    contributors INTEGER NOT NULL,
    critical INTEGER NOT NULL,
    warning INTEGER NOT NULL,
    attention INTEGER NOT NULL,
    info INTEGER NOT NULL,
    suppressed INTEGER NOT NULL,
    artifact TEXT NOT NULL,
    artifact_bytes INTEGER NOT NULL
);
CREATE INDEX scans_by_repository ON scans(repository_id, generated_at);

CREATE TABLE metrics (
    scan_id TEXT NOT NULL REFERENCES scans(id) ON DELETE CASCADE,
    metric TEXT NOT NULL,
    value REAL NOT NULL,
    PRIMARY KEY (scan_id, metric)
);

CREATE TABLE findings (
    scan_id TEXT NOT NULL REFERENCES scans(id) ON DELETE CASCADE,
    finding_id TEXT NOT NULL,
    rule TEXT NOT NULL,
    severity TEXT NOT NULL,
    title TEXT NOT NULL,
    suppressed INTEGER NOT NULL,
    PRIMARY KEY (scan_id, finding_id)
);

CREATE TABLE cache_entries (
    key TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    value BLOB NOT NULL,
    bytes INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER NOT NULL
);
CREATE INDEX cache_by_use ON cache_entries(last_used_at);
"];

/// The newest schema version this build understands.
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

/// Reads the schema version recorded in the database.
pub fn version(connection: &Connection) -> Result<i64, StoreError> {
    Ok(connection.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

/// Applies connection settings: foreign keys, a busy timeout for concurrent RepoDNA
/// processes, and write-ahead logging.
pub fn configure(connection: &Connection) -> Result<(), StoreError> {
    connection.busy_timeout(std::time::Duration::from_secs(10))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    // WAL is unavailable for in-memory databases; the reported mode is then `memory`.
    let _mode: String =
        connection.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
    Ok(())
}

/// Upgrades the database to [`SCHEMA_VERSION`] and returns the version it started from.
pub fn migrate(connection: &mut Connection) -> Result<i64, StoreError> {
    let current = version(connection)?;
    if current > SCHEMA_VERSION {
        return Err(StoreError::NewerSchema {
            found: current,
            supported: SCHEMA_VERSION,
        });
    }
    let applied = usize::try_from(current).unwrap_or(0);
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(applied) {
        let transaction = connection.transaction()?;
        transaction.execute_batch(sql)?;
        transaction.pragma_update(None, "user_version", index as i64 + 1)?;
        transaction.commit()?;
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_new_databases_once() {
        let mut connection = Connection::open_in_memory().unwrap();
        configure(&connection).unwrap();
        assert_eq!(migrate(&mut connection).unwrap(), 0);
        assert_eq!(version(&connection).unwrap(), SCHEMA_VERSION);
        assert_eq!(migrate(&mut connection).unwrap(), SCHEMA_VERSION);
        let tables: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables, 5);
    }

    #[test]
    fn refuses_databases_from_newer_versions() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        assert!(matches!(
            migrate(&mut connection),
            Err(StoreError::NewerSchema { .. })
        ));
    }
}
