//! Sessions CRUD over the `sessions` table.

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Row};
use seele_core::id::SeeleId;
use seele_core::session::{Session, SessionStatus};

use crate::error::{Result, StorageError};
use crate::pool::Pool;

#[derive(Debug, Clone)]
pub struct SessionInput {
    pub project: String,
    pub directory: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SessionFilter {
    pub project: Option<String>,
    pub status: Option<SessionStatus>,
    pub limit: Option<u32>,
}

pub struct SessionStore {
    pool: Pool,
}

impl SessionStore {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn start(&self, input: SessionInput) -> Result<Session> {
        let now = Utc::now();
        let id = SeeleId::new();
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO sessions(id, project, directory, started_at, status) \
             VALUES (?1, ?2, ?3, ?4, 'active')",
            params![
                id.to_string(),
                input.project,
                input.directory,
                now.timestamp_millis(),
            ],
        )?;
        Ok(Session {
            id,
            project: input.project,
            directory: input.directory,
            started_at: now,
            ended_at: None,
            summary: None,
            status: SessionStatus::Active,
        })
    }

    pub fn end(&self, id: SeeleId, summary: Option<String>) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        let conn = self.pool.get()?;
        let updated = conn.execute(
            "UPDATE sessions SET status='ended', ended_at=?1, summary=?2 \
             WHERE id=?3 AND status='active'",
            params![now, summary, id.to_string()],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound(format!(
                "session {id} (active) not found"
            )));
        }
        Ok(())
    }

    pub fn abort(&self, id: SeeleId) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        let conn = self.pool.get()?;
        let updated = conn.execute(
            "UPDATE sessions SET status='aborted', ended_at=?1 \
             WHERE id=?2 AND status='active'",
            params![now, id.to_string()],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound(format!(
                "session {id} (active) not found"
            )));
        }
        Ok(())
    }

    pub fn get(&self, id: SeeleId) -> Result<Option<Session>> {
        let conn = self.pool.get()?;
        let row = conn
            .query_row(
                "SELECT id, project, directory, started_at, ended_at, summary, status \
                 FROM sessions WHERE id=?1",
                [id.to_string()],
                row_to_session,
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        row.transpose().map_err(StorageError::from)
    }

    pub fn list(&self, filter: SessionFilter) -> Result<Vec<Session>> {
        let mut sql = String::from(
            "SELECT id, project, directory, started_at, ended_at, summary, status \
             FROM sessions WHERE 1=1",
        );
        let mut bound: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(project) = filter.project {
            sql.push_str(" AND project = ?");
            bound.push(Box::new(project));
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            bound.push(Box::new(status.as_str().to_string()));
        }
        sql.push_str(" ORDER BY started_at DESC");
        if let Some(limit) = filter.limit {
            sql.push_str(" LIMIT ?");
            bound.push(Box::new(limit as i64));
        }

        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(bound.iter().map(|b| b.as_ref())),
            row_to_session,
        )?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r??);
        }
        Ok(out)
    }
}

fn row_to_session(row: &Row<'_>) -> rusqlite::Result<rusqlite::Result<Session>> {
    let id_str: String = row.get(0)?;
    let project: String = row.get(1)?;
    let directory: Option<String> = row.get(2)?;
    let started_ms: i64 = row.get(3)?;
    let ended_ms: Option<i64> = row.get(4)?;
    let summary: Option<String> = row.get(5)?;
    let status_str: String = row.get(6)?;

    Ok((|| -> rusqlite::Result<Session> {
        let id = id_str.parse::<SeeleId>().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let status = SessionStatus::from_str_strict(&status_str).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                6,
                rusqlite::types::Type::Text,
                format!("unknown session status '{status_str}'").into(),
            )
        })?;
        Ok(Session {
            id,
            project,
            directory,
            started_at: ms_to_dt(started_ms),
            ended_at: ended_ms.map(ms_to_dt),
            summary,
            status,
        })
    })())
}

fn ms_to_dt(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(Utc::now)
}
