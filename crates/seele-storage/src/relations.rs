//! Memory relations CRUD with judgment lifecycle.

use chrono::{TimeZone, Utc};
use rusqlite::{params, Row};
use seele_core::id::SeeleId;
use seele_core::relation::{JudgmentStatus, MemoryRelation, RelationKind};

use crate::error::{Result, StorageError};
use crate::pool::Pool;

#[derive(Debug, Clone)]
pub struct RelationInput {
    pub sync_id: String,
    pub source_id: SeeleId,
    pub target_id: SeeleId,
    pub relation: RelationKind,
    pub reason: Option<String>,
    pub evidence: Option<String>,
    pub confidence: Option<f64>,
    pub marked_by_actor: Option<String>,
    pub marked_by_kind: Option<String>,
    pub marked_by_model: Option<String>,
    pub session_id: Option<SeeleId>,
}

#[derive(Debug, Clone, Default)]
pub struct RelationQuery {
    pub source_id: Option<SeeleId>,
    pub target_id: Option<SeeleId>,
    pub relation: Option<RelationKind>,
    pub status: Option<JudgmentStatus>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct JudgmentInput {
    pub status: JudgmentStatus,
    pub reason: Option<String>,
    pub evidence: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Clone)]
pub struct RelationStore {
    pool: Pool,
}

impl RelationStore {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn create(&self, input: RelationInput) -> Result<MemoryRelation> {
        let id = SeeleId::new();
        let now = Utc::now();
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO memory_relations(id, sync_id, source_id, target_id, relation, \
                                          judgment_status, reason, evidence, confidence, \
                                          marked_by_actor, marked_by_kind, marked_by_model, \
                                          session_id, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                id.to_string(),
                input.sync_id,
                input.source_id.to_string(),
                input.target_id.to_string(),
                input.relation.as_str(),
                input.reason,
                input.evidence,
                input.confidence,
                input.marked_by_actor,
                input.marked_by_kind,
                input.marked_by_model,
                input.session_id.map(|s| s.to_string()),
                now.timestamp_millis(),
            ],
        )?;
        Ok(MemoryRelation {
            id,
            sync_id: input.sync_id,
            source_id: input.source_id,
            target_id: input.target_id,
            relation: input.relation,
            judgment_status: JudgmentStatus::Pending,
            reason: input.reason,
            evidence: input.evidence,
            confidence: input.confidence,
            marked_by_actor: input.marked_by_actor,
            marked_by_kind: input.marked_by_kind,
            marked_by_model: input.marked_by_model,
            session_id: input.session_id,
            created_at: now,
        })
    }

    pub fn judge(&self, id: SeeleId, judgment: JudgmentInput) -> Result<()> {
        let conn = self.pool.get()?;
        let updated = conn.execute(
            "UPDATE memory_relations SET \
                judgment_status = ?1, \
                reason = COALESCE(?2, reason), \
                evidence = COALESCE(?3, evidence), \
                confidence = COALESCE(?4, confidence) \
             WHERE id = ?5",
            params![
                judgment.status.as_str(),
                judgment.reason,
                judgment.evidence,
                judgment.confidence,
                id.to_string(),
            ],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound(format!("memory_relation {id}")));
        }
        Ok(())
    }

    pub fn get(&self, id: SeeleId) -> Result<Option<MemoryRelation>> {
        let conn = self.pool.get()?;
        match conn.query_row(SQL_SELECT_BY_ID, [id.to_string()], |r| {
            Ok(parse_relation(r))
        }) {
            Ok(parsed) => parsed.map(Some),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::from(e)),
        }
    }

    pub fn list(&self, q: RelationQuery) -> Result<Vec<MemoryRelation>> {
        let mut sql = String::from(SQL_SELECT_PREFIX);
        sql.push_str(" WHERE 1=1");
        let mut bound: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(s) = q.source_id {
            sql.push_str(" AND source_id = ?");
            bound.push(Box::new(s.to_string()));
        }
        if let Some(t) = q.target_id {
            sql.push_str(" AND target_id = ?");
            bound.push(Box::new(t.to_string()));
        }
        if let Some(r) = q.relation {
            sql.push_str(" AND relation = ?");
            bound.push(Box::new(r.as_str().to_string()));
        }
        if let Some(s) = q.status {
            sql.push_str(" AND judgment_status = ?");
            bound.push(Box::new(s.as_str().to_string()));
        }
        sql.push_str(" ORDER BY created_at DESC");
        if let Some(lim) = q.limit {
            sql.push_str(" LIMIT ?");
            bound.push(Box::new(lim as i64));
        }
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(&sql)?;
        let mut out = Vec::new();
        let mut rows = stmt.query(rusqlite::params_from_iter(bound.iter().map(|b| b.as_ref())))?;
        while let Some(row) = rows.next()? {
            out.push(parse_relation(row)?);
        }
        Ok(out)
    }
}

const SQL_SELECT_PREFIX: &str = "SELECT id, sync_id, source_id, target_id, relation, \
                                          judgment_status, reason, evidence, confidence, \
                                          marked_by_actor, marked_by_kind, marked_by_model, \
                                          session_id, created_at \
                                 FROM memory_relations";
const SQL_SELECT_BY_ID: &str = "SELECT id, sync_id, source_id, target_id, relation, \
                                          judgment_status, reason, evidence, confidence, \
                                          marked_by_actor, marked_by_kind, marked_by_model, \
                                          session_id, created_at \
                                 FROM memory_relations WHERE id = ?1";

fn parse_relation(row: &Row<'_>) -> Result<MemoryRelation> {
    let id_str: String = row.get(0)?;
    let sync_id: String = row.get(1)?;
    let source_str: String = row.get(2)?;
    let target_str: String = row.get(3)?;
    let relation_str: String = row.get(4)?;
    let status_str: String = row.get(5)?;
    let reason: Option<String> = row.get(6)?;
    let evidence: Option<String> = row.get(7)?;
    let confidence: Option<f64> = row.get(8)?;
    let actor: Option<String> = row.get(9)?;
    let kind: Option<String> = row.get(10)?;
    let model: Option<String> = row.get(11)?;
    let session_str: Option<String> = row.get(12)?;
    let created_ms: i64 = row.get(13)?;

    let id = id_str
        .parse()
        .map_err(|e| StorageError::InvalidInput(format!("bad ULID '{id_str}': {e}")))?;
    let source_id = source_str
        .parse()
        .map_err(|e| StorageError::InvalidInput(format!("bad source ULID '{source_str}': {e}")))?;
    let target_id = target_str
        .parse()
        .map_err(|e| StorageError::InvalidInput(format!("bad target ULID '{target_str}': {e}")))?;
    let relation = RelationKind::from_str_strict(&relation_str)
        .ok_or_else(|| StorageError::InvalidInput(format!("unknown relation '{relation_str}'")))?;
    let judgment_status = JudgmentStatus::from_str_strict(&status_str).ok_or_else(|| {
        StorageError::InvalidInput(format!("unknown judgment status '{status_str}'"))
    })?;
    let session_id = session_str
        .map(|s| {
            s.parse::<SeeleId>()
                .map_err(|e| StorageError::InvalidInput(format!("bad session ULID '{s}': {e}")))
        })
        .transpose()?;

    Ok(MemoryRelation {
        id,
        sync_id,
        source_id,
        target_id,
        relation,
        judgment_status,
        reason,
        evidence,
        confidence,
        marked_by_actor: actor,
        marked_by_kind: kind,
        marked_by_model: model,
        session_id,
        created_at: Utc
            .timestamp_millis_opt(created_ms)
            .single()
            .unwrap_or_else(Utc::now),
    })
}
