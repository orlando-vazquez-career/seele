//! User prompts: short text records of the user's input. FTS-indexed.

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Row};
use seele_core::id::SeeleId;
use serde::{Deserialize, Serialize};

use crate::error::{Result, StorageError};
use crate::pool::Pool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPrompt {
    pub id: SeeleId,
    pub session_id: Option<SeeleId>,
    pub content: String,
    pub project: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PromptInput {
    pub session_id: Option<SeeleId>,
    pub content: String,
    pub project: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PromptQuery {
    pub project: Option<String>,
    pub session_id: Option<SeeleId>,
    pub limit: Option<u32>,
}

#[derive(Clone)]
pub struct PromptStore {
    pool: Pool,
}

impl PromptStore {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn save(&self, input: PromptInput) -> Result<UserPrompt> {
        let id = SeeleId::new();
        let now = Utc::now();
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO user_prompts(id, session_id, content, project, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id.to_string(),
                input.session_id.map(|s| s.to_string()),
                input.content,
                input.project,
                now.timestamp_millis(),
            ],
        )?;
        Ok(UserPrompt {
            id,
            session_id: input.session_id,
            content: input.content,
            project: input.project,
            created_at: now,
        })
    }

    pub fn get(&self, id: SeeleId) -> Result<Option<UserPrompt>> {
        let conn = self.pool.get()?;
        match conn.query_row(
            "SELECT id, session_id, content, project, created_at \
             FROM user_prompts WHERE id = ?1",
            [id.to_string()],
            |r| Ok(parse_prompt(r)),
        ) {
            Ok(parsed) => parsed.map(Some),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::from(e)),
        }
    }

    pub fn list(&self, q: PromptQuery) -> Result<Vec<UserPrompt>> {
        let mut sql = String::from(
            "SELECT id, session_id, content, project, created_at \
             FROM user_prompts WHERE 1=1",
        );
        let mut bound: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(p) = q.project {
            sql.push_str(" AND project = ?");
            bound.push(Box::new(p));
        }
        if let Some(s) = q.session_id {
            sql.push_str(" AND session_id = ?");
            bound.push(Box::new(s.to_string()));
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
            out.push(parse_prompt(row)?);
        }
        Ok(out)
    }

    pub fn delete(&self, id: SeeleId) -> Result<()> {
        let conn = self.pool.get()?;
        let removed = conn.execute("DELETE FROM user_prompts WHERE id = ?1", [id.to_string()])?;
        if removed == 0 {
            return Err(StorageError::NotFound(format!("user_prompt {id}")));
        }
        Ok(())
    }
}

fn parse_prompt(row: &Row<'_>) -> Result<UserPrompt> {
    let id_str: String = row.get(0)?;
    let session_str: Option<String> = row.get(1)?;
    let content: String = row.get(2)?;
    let project: Option<String> = row.get(3)?;
    let created_ms: i64 = row.get(4)?;

    let id = id_str
        .parse::<SeeleId>()
        .map_err(|e| StorageError::InvalidInput(format!("bad ULID '{id_str}': {e}")))?;
    let session_id = session_str
        .map(|s| {
            s.parse::<SeeleId>()
                .map_err(|e| StorageError::InvalidInput(format!("bad session ULID '{s}': {e}")))
        })
        .transpose()?;

    Ok(UserPrompt {
        id,
        session_id,
        content,
        project,
        created_at: Utc
            .timestamp_millis_opt(created_ms)
            .single()
            .unwrap_or_else(Utc::now),
    })
}
