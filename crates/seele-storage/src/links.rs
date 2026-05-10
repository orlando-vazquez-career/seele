//! Generic link CRUD over the `links` table. Used by MNEMA for the
//! derivative graph (counsel→advisor→review→verdict→skill→decision).

use chrono::{TimeZone, Utc};
use rusqlite::{params, Row};
use seele_core::id::SeeleId;
use seele_core::link::Link;
use seele_core::metadata::Metadata;

use crate::error::{Result, StorageError};
use crate::pool::Pool;

#[derive(Debug, Clone)]
pub struct LinkInput {
    pub from_id: SeeleId,
    pub to_id: SeeleId,
    pub link_type: String,
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Default)]
pub struct LinkQuery {
    pub from_id: Option<SeeleId>,
    pub to_id: Option<SeeleId>,
    pub link_type: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone)]
pub struct LinkStore {
    pool: Pool,
}

impl LinkStore {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn create(&self, input: LinkInput) -> Result<Link> {
        let id = SeeleId::new();
        let now = Utc::now();
        let metadata_json = serde_json::to_string(&input.metadata)
            .map_err(|e| StorageError::InvalidInput(format!("metadata: {e}")))?;
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO links(id, from_id, to_id, link_type, metadata, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id.to_string(),
                input.from_id.to_string(),
                input.to_id.to_string(),
                input.link_type,
                metadata_json,
                now.timestamp_millis(),
            ],
        )?;
        Ok(Link {
            id,
            from_id: input.from_id,
            to_id: input.to_id,
            link_type: input.link_type,
            metadata: input.metadata,
            created_at: now,
        })
    }

    pub fn list(&self, q: LinkQuery) -> Result<Vec<Link>> {
        let mut sql = String::from(
            "SELECT id, from_id, to_id, link_type, metadata, created_at \
             FROM links WHERE 1=1",
        );
        let mut bound: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(f) = q.from_id {
            sql.push_str(" AND from_id = ?");
            bound.push(Box::new(f.to_string()));
        }
        if let Some(t) = q.to_id {
            sql.push_str(" AND to_id = ?");
            bound.push(Box::new(t.to_string()));
        }
        if let Some(lt) = q.link_type {
            sql.push_str(" AND link_type = ?");
            bound.push(Box::new(lt));
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
            out.push(parse_link(row)?);
        }
        Ok(out)
    }

    pub fn delete(&self, id: SeeleId) -> Result<()> {
        let conn = self.pool.get()?;
        let removed = conn.execute("DELETE FROM links WHERE id = ?1", [id.to_string()])?;
        if removed == 0 {
            return Err(StorageError::NotFound(format!("link {id}")));
        }
        Ok(())
    }
}

fn parse_link(row: &Row<'_>) -> Result<Link> {
    let id: String = row.get(0)?;
    let from: String = row.get(1)?;
    let to: String = row.get(2)?;
    let link_type: String = row.get(3)?;
    let meta_json: String = row.get(4)?;
    let created_ms: i64 = row.get(5)?;

    Ok(Link {
        id: id
            .parse()
            .map_err(|e| StorageError::InvalidInput(format!("bad ULID '{id}': {e}")))?,
        from_id: from
            .parse()
            .map_err(|e| StorageError::InvalidInput(format!("bad from ULID '{from}': {e}")))?,
        to_id: to
            .parse()
            .map_err(|e| StorageError::InvalidInput(format!("bad to ULID '{to}': {e}")))?,
        link_type,
        metadata: serde_json::from_str(&meta_json)
            .map_err(|e| StorageError::InvalidInput(format!("metadata JSON: {e}")))?,
        created_at: Utc
            .timestamp_millis_opt(created_ms)
            .single()
            .unwrap_or_else(Utc::now),
    })
}
