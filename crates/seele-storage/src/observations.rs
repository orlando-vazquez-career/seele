//! Observations CRUD + upsert + dedup.
//!
//! `save()` is the heart of SEELE's write path. It applies privacy stripping,
//! computes the normalized hash, runs a topic_key upsert if present, falls
//! through to a normalized-hash dedup window, and otherwise creates a new
//! row. FTS5 sync happens automatically via triggers in migration V001.

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Row, Transaction};
use seele_core::id::SeeleId;
use seele_core::memory::{Observation, ObservationType, Scope};
use seele_core::metadata::Metadata;

use crate::error::{Result, StorageError};
use crate::hash::normalized_hash;
use crate::pool::Pool;
use crate::privacy::strip_private_tags;

/// Window for normalized_hash dedup. Within this window, a duplicate
/// content+title under the same (project, scope, type) increments
/// `duplicate_count` instead of creating a new row. Inherited from ENGRAM.
const DEDUP_WINDOW_MS: i64 = 24 * 60 * 60 * 1000;
/// Max retries on int_id collision (bytes 9..16 of ULID).
const ID_COLLISION_RETRIES: usize = 5;

#[derive(Debug, Clone)]
pub struct SaveInput {
    pub session_id: Option<SeeleId>,
    pub kind: ObservationType,
    pub title: String,
    pub content: String,
    pub tool_name: Option<String>,
    pub project: Option<String>,
    pub scope: Scope,
    pub topic_key: Option<String>,
    pub metadata: Metadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveOutcome {
    Created(SeeleId),
    UpsertedTopic { id: SeeleId, revision_count: u32 },
    DuplicateMerged { id: SeeleId, duplicate_count: u32 },
}

impl SaveOutcome {
    pub fn id(&self) -> SeeleId {
        match self {
            Self::Created(id) => *id,
            Self::UpsertedTopic { id, .. } => *id,
            Self::DuplicateMerged { id, .. } => *id,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ObservationQuery {
    pub project: Option<String>,
    pub scope: Option<Scope>,
    pub kind: Option<ObservationType>,
    pub session_id: Option<SeeleId>,
    pub topic_key: Option<String>,
    pub include_deleted: bool,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct ObservationPatch {
    pub title: Option<String>,
    pub content: Option<String>,
    pub tool_name: Option<Option<String>>,
    pub topic_key: Option<Option<String>>,
    pub metadata: Option<Metadata>,
}

#[derive(Clone)]
pub struct ObservationStore {
    pool: Pool,
}

impl ObservationStore {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn save(&self, input: SaveInput) -> Result<SaveOutcome> {
        let mut conn = self.pool.get()?;
        let tx = conn.transaction()?;
        let outcome = save_in_tx(&tx, input)?;
        tx.commit()?;
        Ok(outcome)
    }

    pub fn get(&self, id: SeeleId) -> Result<Option<Observation>> {
        let conn = self.pool.get()?;
        match conn.query_row(SQL_SELECT_BY_ID, [id.to_string()], |row| {
            Ok(parse_observation(row))
        }) {
            Ok(parsed) => parsed.map(Some),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::from(e)),
        }
    }

    pub fn list(&self, q: ObservationQuery) -> Result<Vec<Observation>> {
        let mut sql = String::from(SQL_SELECT_PREFIX);
        sql.push_str(" WHERE 1=1");
        let mut bound: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if !q.include_deleted {
            sql.push_str(" AND deleted_at IS NULL");
        }
        if let Some(p) = q.project {
            sql.push_str(" AND project = ?");
            bound.push(Box::new(p));
        }
        if let Some(s) = q.scope {
            sql.push_str(" AND scope = ?");
            bound.push(Box::new(s.as_str().to_string()));
        }
        if let Some(k) = q.kind {
            sql.push_str(" AND type = ?");
            bound.push(Box::new(k.as_str().to_string()));
        }
        if let Some(sid) = q.session_id {
            sql.push_str(" AND session_id = ?");
            bound.push(Box::new(sid.to_string()));
        }
        if let Some(tk) = q.topic_key {
            sql.push_str(" AND topic_key = ?");
            bound.push(Box::new(tk));
        }
        sql.push_str(" ORDER BY created_at DESC");
        if let Some(limit) = q.limit {
            sql.push_str(" LIMIT ?");
            bound.push(Box::new(limit as i64));
        }

        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(&sql)?;
        let mut out = Vec::new();
        let mut rows = stmt.query(rusqlite::params_from_iter(bound.iter().map(|b| b.as_ref())))?;
        while let Some(row) = rows.next()? {
            out.push(parse_observation(row)?);
        }
        Ok(out)
    }

    pub fn update(&self, id: SeeleId, patch: ObservationPatch) -> Result<()> {
        let mut conn = self.pool.get()?;
        let tx = conn.transaction()?;
        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT title, content FROM observations WHERE id=?1 AND deleted_at IS NULL",
                [id.to_string()],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        let (cur_title, cur_content) =
            existing.ok_or_else(|| StorageError::NotFound(format!("observation {id}")))?;

        let new_title = patch.title.unwrap_or(cur_title);
        let new_content_raw = patch.content.unwrap_or(cur_content);
        let new_content = strip_private_tags(&new_content_raw);
        let new_hash = normalized_hash(&new_content);

        let now_ms = Utc::now().timestamp_millis();
        let mut sets: Vec<&'static str> = Vec::new();
        let mut bound: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        sets.push("title = ?");
        bound.push(Box::new(new_title));
        sets.push("content = ?");
        bound.push(Box::new(new_content));
        sets.push("normalized_hash = ?");
        bound.push(Box::new(new_hash));
        if let Some(tn) = patch.tool_name {
            sets.push("tool_name = ?");
            bound.push(Box::new(tn));
        }
        if let Some(tk) = patch.topic_key {
            sets.push("topic_key = ?");
            bound.push(Box::new(tk));
        }
        if let Some(meta) = patch.metadata {
            sets.push("metadata = ?");
            bound.push(Box::new(serde_json::to_string(&meta).map_err(|e| {
                StorageError::InvalidInput(format!("metadata not serializable: {e}"))
            })?));
        }
        sets.push("updated_at = ?");
        bound.push(Box::new(now_ms));
        sets.push("last_seen_at = ?");
        bound.push(Box::new(now_ms));

        let sql = format!("UPDATE observations SET {} WHERE id = ?", sets.join(", "));
        bound.push(Box::new(id.to_string()));
        tx.execute(
            &sql,
            rusqlite::params_from_iter(bound.iter().map(|b| b.as_ref())),
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn soft_delete(&self, id: SeeleId) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        let conn = self.pool.get()?;
        let updated = conn.execute(
            "UPDATE observations SET deleted_at = ?1, updated_at = ?1 \
             WHERE id = ?2 AND deleted_at IS NULL",
            params![now, id.to_string()],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound(format!(
                "active observation {id} not found"
            )));
        }
        Ok(())
    }

    pub fn restore(&self, id: SeeleId) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        let conn = self.pool.get()?;
        let updated = conn.execute(
            "UPDATE observations SET deleted_at = NULL, updated_at = ?1 \
             WHERE id = ?2 AND deleted_at IS NOT NULL",
            params![now, id.to_string()],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound(format!(
                "soft-deleted observation {id} not found"
            )));
        }
        Ok(())
    }

    pub fn hard_delete(&self, id: SeeleId) -> Result<()> {
        let conn = self.pool.get()?;
        let removed = conn.execute("DELETE FROM observations WHERE id = ?1", [id.to_string()])?;
        if removed == 0 {
            return Err(StorageError::NotFound(format!("observation {id}")));
        }
        // Also clear the vec0 row so a future ID collision doesn't surface
        // a stale embedding.
        let _ = conn.execute(
            "DELETE FROM observations_vec WHERE rowid IN \
             (SELECT int_id FROM observations WHERE id = ?1)",
            [id.to_string()],
        );
        Ok(())
    }

    /// Insert (or replace) the embedding vector for `id` into `observations_vec`.
    /// `embedding` length must match the column dim (currently 384). Caller
    /// is responsible for normalization.
    pub fn set_embedding(&self, id: SeeleId, embedding: &[f32]) -> Result<()> {
        let conn = self.pool.get()?;
        let int_id: i64 = conn
            .query_row(
                "SELECT int_id FROM observations WHERE id = ?1 AND deleted_at IS NULL",
                [id.to_string()],
                |r| r.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    StorageError::NotFound(format!("active observation {id}"))
                }
                other => StorageError::from(other),
            })?;
        let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
        conn.execute(
            "INSERT OR REPLACE INTO observations_vec(rowid, embedding) VALUES (?1, ?2)",
            params![int_id, bytes],
        )?;
        Ok(())
    }

    /// Remove the embedding row for `id`. No-op if no embedding present.
    pub fn delete_embedding(&self, id: SeeleId) -> Result<()> {
        let conn = self.pool.get()?;
        conn.execute(
            "DELETE FROM observations_vec WHERE rowid IN \
             (SELECT int_id FROM observations WHERE id = ?1)",
            [id.to_string()],
        )?;
        Ok(())
    }
}

fn save_in_tx(tx: &Transaction<'_>, input: SaveInput) -> Result<SaveOutcome> {
    let stripped_title = strip_private_tags(&input.title);
    let stripped_content = strip_private_tags(&input.content);
    let hash = normalized_hash(&stripped_content);
    let now_ms = Utc::now().timestamp_millis();
    let metadata_json = serde_json::to_string(&input.metadata)
        .map_err(|e| StorageError::InvalidInput(format!("metadata not serializable: {e}")))?;

    // 1. Topic key upsert.
    if let Some(tk) = input.topic_key.as_ref() {
        let existing: Option<(String, u32)> = tx
            .query_row(
                "SELECT id, revision_count FROM observations \
                 WHERE project IS ?1 AND scope = ?2 AND topic_key = ?3 \
                       AND deleted_at IS NULL \
                 LIMIT 1",
                params![input.project, input.scope.as_str(), tk],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?)),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        if let Some((id_str, rev)) = existing {
            let new_rev = rev + 1;
            tx.execute(
                "UPDATE observations SET \
                    title = ?1, content = ?2, normalized_hash = ?3, \
                    revision_count = ?4, last_seen_at = ?5, updated_at = ?5, \
                    metadata = ?6, type = ?7, tool_name = ?8 \
                 WHERE id = ?9",
                params![
                    stripped_title,
                    stripped_content,
                    hash,
                    new_rev,
                    now_ms,
                    metadata_json,
                    input.kind.as_str(),
                    input.tool_name,
                    id_str,
                ],
            )?;
            let id = id_str
                .parse::<SeeleId>()
                .map_err(|e| StorageError::InvalidInput(format!("bad ULID '{id_str}': {e}")))?;
            return Ok(SaveOutcome::UpsertedTopic {
                id,
                revision_count: new_rev,
            });
        }
    }

    // 2. Normalized hash dedup window.
    let dedup_threshold = now_ms - DEDUP_WINDOW_MS;
    let dup: Option<(String, u32)> = tx
        .query_row(
            "SELECT id, duplicate_count FROM observations \
             WHERE normalized_hash = ?1 \
                AND ((project IS NULL AND ?2 IS NULL) OR project = ?2) \
                AND scope = ?3 \
                AND type = ?4 \
                AND title = ?5 \
                AND last_seen_at >= ?6 \
                AND deleted_at IS NULL \
             LIMIT 1",
            params![
                hash,
                input.project,
                input.scope.as_str(),
                input.kind.as_str(),
                stripped_title,
                dedup_threshold,
            ],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?)),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })?;
    if let Some((id_str, dup_count)) = dup {
        let new_count = dup_count + 1;
        tx.execute(
            "UPDATE observations SET duplicate_count = ?1, last_seen_at = ?2 \
             WHERE id = ?3",
            params![new_count, now_ms, id_str],
        )?;
        let id = id_str
            .parse::<SeeleId>()
            .map_err(|e| StorageError::InvalidInput(format!("bad ULID '{id_str}': {e}")))?;
        return Ok(SaveOutcome::DuplicateMerged {
            id,
            duplicate_count: new_count,
        });
    }

    // 3. Insert new row, retry on int_id collision.
    for _ in 0..ID_COLLISION_RETRIES {
        let id = SeeleId::new();
        let int_id = id.as_i64();
        let res = tx.execute(
            "INSERT INTO observations(id, int_id, session_id, type, title, content, \
                                      tool_name, project, scope, topic_key, \
                                      normalized_hash, revision_count, duplicate_count, \
                                      last_seen_at, created_at, updated_at, metadata) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, 0, ?12, ?12, ?12, ?13)",
            params![
                id.to_string(),
                int_id,
                input.session_id.map(|s| s.to_string()),
                input.kind.as_str(),
                stripped_title,
                stripped_content,
                input.tool_name,
                input.project,
                input.scope.as_str(),
                input.topic_key,
                hash,
                now_ms,
                metadata_json,
            ],
        );
        match res {
            Ok(_) => return Ok(SaveOutcome::Created(id)),
            Err(rusqlite::Error::SqliteFailure(e, msg))
                if matches!(e.code, rusqlite::ErrorCode::ConstraintViolation)
                    && msg
                        .as_deref()
                        .is_some_and(|m| m.contains("observations.int_id")) =>
            {
                // Collision on the random tail mapping; regenerate.
                continue;
            }
            Err(other) => return Err(StorageError::from(other)),
        }
    }
    Err(StorageError::Conflict(format!(
        "exhausted {ID_COLLISION_RETRIES} retries on int_id collision"
    )))
}

const SQL_SELECT_PREFIX: &str = "SELECT id, int_id, session_id, type, title, content, \
                                          tool_name, project, scope, topic_key, \
                                          normalized_hash, revision_count, duplicate_count, \
                                          last_seen_at, created_at, updated_at, deleted_at, \
                                          metadata \
                                 FROM observations";
const SQL_SELECT_BY_ID: &str = "SELECT id, int_id, session_id, type, title, content, \
                                          tool_name, project, scope, topic_key, \
                                          normalized_hash, revision_count, duplicate_count, \
                                          last_seen_at, created_at, updated_at, deleted_at, \
                                          metadata \
                                 FROM observations WHERE id = ?1";

fn parse_observation(row: &Row<'_>) -> Result<Observation> {
    let id_str: String = row.get(0)?;
    let _int_id: i64 = row.get(1)?;
    let session_str: Option<String> = row.get(2)?;
    let type_str: String = row.get(3)?;
    let title: String = row.get(4)?;
    let content: String = row.get(5)?;
    let tool_name: Option<String> = row.get(6)?;
    let project: Option<String> = row.get(7)?;
    let scope_str: String = row.get(8)?;
    let topic_key: Option<String> = row.get(9)?;
    let normalized_hash: Option<String> = row.get(10)?;
    let revision_count: u32 = row.get(11)?;
    let duplicate_count: u32 = row.get(12)?;
    let last_seen_ms: i64 = row.get(13)?;
    let created_ms: i64 = row.get(14)?;
    let updated_ms: i64 = row.get(15)?;
    let deleted_ms: Option<i64> = row.get(16)?;
    let metadata_json: String = row.get(17)?;

    let id = id_str
        .parse::<SeeleId>()
        .map_err(|e| StorageError::InvalidInput(format!("bad ULID '{id_str}': {e}")))?;
    let session_id = session_str
        .map(|s| {
            s.parse::<SeeleId>()
                .map_err(|e| StorageError::InvalidInput(format!("bad session ULID '{s}': {e}")))
        })
        .transpose()?;
    let kind = ObservationType::from_str_relaxed(&type_str);
    let scope = Scope::from_str_strict(&scope_str)
        .ok_or_else(|| StorageError::InvalidInput(format!("unknown scope '{scope_str}'")))?;
    let metadata: Metadata = serde_json::from_str(&metadata_json).map_err(|e| {
        StorageError::InvalidInput(format!("metadata JSON '{metadata_json}' invalid: {e}"))
    })?;

    Ok(Observation {
        id,
        session_id,
        kind,
        title,
        content,
        tool_name,
        project,
        scope,
        topic_key,
        normalized_hash,
        revision_count,
        duplicate_count,
        last_seen_at: ms_to_dt(last_seen_ms),
        created_at: ms_to_dt(created_ms),
        updated_at: ms_to_dt(updated_ms),
        deleted_at: deleted_ms.map(ms_to_dt),
        metadata,
    })
}

fn ms_to_dt(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(Utc::now)
}
