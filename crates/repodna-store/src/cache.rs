//! The per-file analysis cache.
//!
//! Entries are compressed JSON keyed by [`repodna_engine::scan::cache_key`]. Cache problems
//! never fail an analysis: an unreadable entry is treated as a miss, and a failed write only
//! means the file is analyzed again next time. When the cache grows beyond its limit, the
//! least recently used entries are evicted.

use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use repodna_core::time::Timestamp;
use repodna_engine::AnalysisCache;
use repodna_parser::FileAnalysis;
use rusqlite::{OptionalExtension, params};

use crate::{Store, StoreError};

/// Kind recorded for per-file analysis entries.
const FILE_ANALYSIS: &str = "file-analysis";

/// Default size limit of the cache (compressed bytes).
pub const DEFAULT_CACHE_LIMIT: u64 = 256 * 1024 * 1024;

/// Largest decompressed entry accepted, a guard against corrupted data.
const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;

/// Size of the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CacheStats {
    /// Entries stored.
    pub entries: u64,
    /// Compressed bytes stored.
    pub bytes: u64,
}

fn encode(analysis: &FileAnalysis) -> Option<Vec<u8>> {
    let json = serde_json::to_vec(analysis).ok()?;
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&json).ok()?;
    encoder.finish().ok()
}

fn decode(bytes: &[u8]) -> Option<FileAnalysis> {
    let mut json = Vec::new();
    DeflateDecoder::new(bytes)
        .take(MAX_ENTRY_BYTES)
        .read_to_end(&mut json)
        .ok()?;
    serde_json::from_slice(&json).ok()
}

impl Store {
    /// Size of the per-file analysis cache.
    pub fn cache_stats(&self) -> Result<CacheStats, StoreError> {
        let connection = self.lock();
        let (entries, bytes): (i64, i64) = connection.query_row(
            "SELECT COUNT(*), COALESCE(SUM(bytes), 0) FROM cache_entries",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(CacheStats {
            entries: u64::try_from(entries).unwrap_or(0),
            bytes: u64::try_from(bytes).unwrap_or(0),
        })
    }

    /// Removes every cache entry and returns what was removed. Scans and artifacts are kept.
    pub fn clear_cache(&self) -> Result<CacheStats, StoreError> {
        let stats = self.cache_stats()?;
        let connection = self.lock();
        connection.execute("DELETE FROM cache_entries", [])?;
        // Return the freed pages to the file system.
        connection.execute_batch("VACUUM")?;
        Ok(stats)
    }

    /// Evicts the least recently used entries until the cache is within `limit` bytes.
    /// Returns the number of entries evicted.
    pub fn evict_cache(&self, limit: u64) -> Result<u64, StoreError> {
        let stats = self.cache_stats()?;
        if stats.bytes <= limit {
            return Ok(0);
        }
        let excess = stats.bytes - limit;
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        let victims: Vec<String> = {
            let mut statement = transaction.prepare(
                "SELECT key, bytes FROM cache_entries ORDER BY last_used_at, created_at, key",
            )?;
            let mut rows = statement.query([])?;
            let mut freed = 0u64;
            let mut victims = Vec::new();
            while freed < excess {
                let Some(row) = rows.next()? else {
                    break;
                };
                victims.push(row.get::<_, String>(0)?);
                freed += u64::try_from(row.get::<_, i64>(1)?).unwrap_or(0);
            }
            victims
        };
        {
            let mut delete = transaction.prepare("DELETE FROM cache_entries WHERE key = ?1")?;
            for key in &victims {
                delete.execute([key])?;
            }
        }
        transaction.commit()?;
        Ok(victims.len() as u64)
    }

    fn touch_used(&self) -> Result<(), StoreError> {
        let keys = std::mem::take(
            &mut *self
                .touched
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        if keys.is_empty() {
            return Ok(());
        }
        let now = Timestamp::now().unix();
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        {
            let mut update =
                transaction.prepare("UPDATE cache_entries SET last_used_at = ?1 WHERE key = ?2")?;
            for key in &keys {
                update.execute(params![now, key])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    fn store_entries(&self, entries: &[(String, &FileAnalysis)]) -> Result<(), StoreError> {
        let encoded: Vec<(&str, Vec<u8>)> = entries
            .iter()
            .filter_map(|(key, analysis)| encode(analysis).map(|bytes| (key.as_str(), bytes)))
            .collect();
        let now = Timestamp::now().unix();
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        {
            let mut insert = transaction.prepare(
                "INSERT OR REPLACE INTO cache_entries (key, kind, value, bytes, created_at, last_used_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            )?;
            for (key, bytes) in &encoded {
                insert.execute(params![
                    key,
                    FILE_ANALYSIS,
                    bytes,
                    i64::try_from(bytes.len()).unwrap_or(i64::MAX),
                    now
                ])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }
}

impl AnalysisCache for Store {
    fn get(&self, key: &str) -> Option<FileAnalysis> {
        let bytes: Vec<u8> = self
            .lock()
            .query_row(
                "SELECT value FROM cache_entries WHERE key = ?1 AND kind = ?2",
                params![key, FILE_ANALYSIS],
                |row| row.get(0),
            )
            .optional()
            .ok()??;
        let analysis = decode(&bytes)?;
        self.touched
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(key.to_owned());
        Some(analysis)
    }

    fn put(&self, entries: &[(String, &FileAnalysis)]) {
        // A failed write only means the files are analyzed again next time.
        let _ = self.store_entries(entries);
    }

    fn finish(&self) {
        let _ = self.touch_used();
        let _ = self.evict_cache(self.cache_limit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StorePaths;
    use repodna_parser::{LanguageRegistry, analyze_source};

    fn analysis(text: &str) -> FileAnalysis {
        let spec = LanguageRegistry::builtin().get("rust").unwrap();
        analyze_source(spec, text)
    }

    #[test]
    fn stores_and_returns_analyses() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(StorePaths::new(dir.path())).unwrap();
        let a = analysis("use std::fmt;\npub fn a() { if x { } }\n");
        assert!(store.get("k1").is_none());
        store.put(&[("k1".to_owned(), &a)]);
        assert_eq!(store.get("k1"), Some(a));
        store.finish();
        let stats = store.cache_stats().unwrap();
        assert_eq!(stats.entries, 1);
        assert!(stats.bytes > 0);
        assert_eq!(store.clear_cache().unwrap().entries, 1);
        assert_eq!(store.cache_stats().unwrap(), CacheStats::default());
    }

    #[test]
    fn treats_corrupt_entries_as_misses() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(StorePaths::new(dir.path())).unwrap();
        store
            .lock()
            .execute(
                "INSERT INTO cache_entries (key, kind, value, bytes, created_at, last_used_at) VALUES ('bad', ?1, x'00ff', 2, 0, 0)",
                [FILE_ANALYSIS],
            )
            .unwrap();
        assert!(store.get("bad").is_none());
    }

    #[test]
    fn evicts_least_recently_used_entries() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(StorePaths::new(dir.path())).unwrap();
        let a = analysis("pub fn a() {}\n");
        let entries: Vec<(String, &FileAnalysis)> = (0..5).map(|i| (format!("k{i}"), &a)).collect();
        store.put(&entries);
        {
            let connection = store.lock();
            for i in 0..5 {
                connection
                    .execute(
                        "UPDATE cache_entries SET last_used_at = ?1 WHERE key = ?2",
                        params![i, format!("k{i}")],
                    )
                    .unwrap();
            }
        }
        let one = store.cache_stats().unwrap().bytes / 5;
        assert_eq!(store.evict_cache(one * 2).unwrap(), 3);
        let remaining: Vec<String> = store
            .lock()
            .prepare("SELECT key FROM cache_entries ORDER BY key")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(remaining, vec!["k3", "k4"]);
        assert_eq!(store.evict_cache(u64::MAX).unwrap(), 0);
        assert_eq!(store.evict_cache(0).unwrap(), 2);
        assert_eq!(store.cache_stats().unwrap().entries, 0);
    }

    #[test]
    fn speeds_up_a_second_analysis() {
        use repodna_core::cancel::CancellationToken;
        use repodna_core::config::{Config, Stage, StageSet};
        use repodna_engine::Progress;
        use repodna_engine::scan::{ScanOptions, scan_files};

        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join("src")).unwrap();
        std::fs::write(repo.path().join("src/lib.rs"), "pub fn a() {}\n").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(StorePaths::new(dir.path())).unwrap();
        let config = Config::default();
        let options = ScanOptions {
            config: &config,
            stages: StageSet::of(&Stage::ALL),
            registry: LanguageRegistry::builtin(),
            progress: &Progress::default(),
            cache: Some(&store),
        };
        let cancel = CancellationToken::new();
        let first = scan_files(repo.path(), &options, &cancel).unwrap();
        let second = scan_files(repo.path(), &options, &cancel).unwrap();
        assert_eq!(first.cache_misses, 1);
        assert_eq!(second.cache_hits, 1);
        assert_eq!(first.files[0].analysis, second.files[0].analysis);
    }
}
