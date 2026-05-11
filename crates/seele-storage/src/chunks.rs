//! Sync chunks: dedup ledger for git-sync imports. Composite PK
//! `(target_key, chunk_id)` so re-imports are idempotent.

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::pool::Pool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncChunk {
    pub target_key: String,
    pub chunk_id: String,
    pub imported_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct ChunkStore {
    pool: Pool,
}

impl ChunkStore {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Mark `chunk_id` imported under `target_key`. Returns `true` if newly
    /// recorded, `false` if already present (idempotent).
    pub fn mark_imported(&self, target_key: &str, chunk_id: &str) -> Result<bool> {
        let conn = self.pool.get()?;
        let now = Utc::now().timestamp_millis();
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO sync_chunks(target_key, chunk_id, imported_at) \
             VALUES (?1, ?2, ?3)",
            params![target_key, chunk_id, now],
        )?;
        Ok(inserted == 1)
    }

    /// `mark_imported` variant that runs inside a caller-owned tx so
    /// the ledger write commits or rolls back alongside the observation
    /// saves of the same import operation.
    pub fn mark_imported_in_tx(
        tx: &rusqlite::Transaction<'_>,
        target_key: &str,
        chunk_id: &str,
    ) -> Result<bool> {
        let now = Utc::now().timestamp_millis();
        let inserted = tx.execute(
            "INSERT OR IGNORE INTO sync_chunks(target_key, chunk_id, imported_at) \
             VALUES (?1, ?2, ?3)",
            params![target_key, chunk_id, now],
        )?;
        Ok(inserted == 1)
    }

    pub fn was_imported(&self, target_key: &str, chunk_id: &str) -> Result<bool> {
        let conn = self.pool.get()?;
        let count: i64 = conn.query_row(
            "SELECT count(*) FROM sync_chunks WHERE target_key = ?1 AND chunk_id = ?2",
            params![target_key, chunk_id],
            |r| r.get(0),
        )?;
        Ok(count == 1)
    }

    pub fn list_for_target(&self, target_key: &str) -> Result<Vec<SyncChunk>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT target_key, chunk_id, imported_at FROM sync_chunks \
             WHERE target_key = ?1 ORDER BY imported_at DESC",
        )?;
        let mut out = Vec::new();
        let mut rows = stmt.query([target_key])?;
        while let Some(row) = rows.next()? {
            let target_key: String = row.get(0)?;
            let chunk_id: String = row.get(1)?;
            let ms: i64 = row.get(2)?;
            out.push(SyncChunk {
                target_key,
                chunk_id,
                imported_at: Utc
                    .timestamp_millis_opt(ms)
                    .single()
                    .unwrap_or_else(Utc::now),
            });
        }
        Ok(out)
    }
}
