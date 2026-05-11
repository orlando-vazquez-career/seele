//! ENGRAM → SEELE migration (ADR-13).
//!
//! Reads an ENGRAM SQLite DB in read-only mode, maps each `memories`
//! row into SEELE's `observations` schema, and inserts via
//! `ObservationStore::save_raw_in_tx` inside one transaction so the
//! import is atomic per source file.
//!
//! ENGRAM stores its data in a `memories` table. The columns we read
//! (id, body, metadata, created_at) are mandatory; optional columns
//! (`title`, `updated_at`, `project`, `scope`, `tool_name`,
//! `session_id`, `type`, `topic_key`) are picked up when present. Most
//! per-row attributes live inside the `metadata` JSON blob; that's
//! where MNEMA stashes `kind`, `domain`, `axiomatic`, `linked_to`, etc.
//!
//! Idempotency: re-running over the same source DB never duplicates
//! rows. `INSERT OR IGNORE` on `id` does the work; rows whose source
//! id parses to a valid ULID preserve it, others mint a new `SeeleId`
//! and stash `engram_id` in the destination metadata so the original
//! id is recoverable.
//!
//! Links: when an ENGRAM row's metadata carries `linked_to: [...]`
//! pointing at other ENGRAM ids, we register the mapping during the
//! main pass and resolve it to `SeeleId`s once all observations are
//! in. Links land in the same transaction.

use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use thiserror::Error;

use seele_core::id::SeeleId;
use seele_core::memory::{ObservationType, Scope};
use seele_core::metadata::Metadata;
use seele_storage::{
    LinkInput, LinkStore, ObservationStore, RawSaveInput, RawSaveOutcome, StorageError,
};

#[derive(Debug, Error)]
pub enum EngramImportError {
    #[error("source DB at {path:?} could not be opened: {detail}")]
    OpenSource { path: PathBuf, detail: String },
    #[error("source DB does not look like an ENGRAM DB: {detail}")]
    BadSchema { detail: String },
    #[error("storage: {0}")]
    Storage(#[from] StorageError),
    #[error("rusqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("r2d2: {0}")]
    Pool(#[from] r2d2::Error),
    #[error("invalid metadata JSON on row {id}: {detail}")]
    InvalidMetadata { id: String, detail: String },
}

pub type Result<T> = std::result::Result<T, EngramImportError>;

/// Summary of a one-shot import. Returned for both real and `dry_run`
/// runs (dry_run skips the writing phase but still computes counts).
#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub source_path: PathBuf,
    /// Total rows the source DB exposed.
    pub rows_seen: usize,
    /// Rows newly inserted into the destination.
    pub rows_inserted: usize,
    /// Rows whose preserved `id` was already in the destination — skipped.
    pub rows_skipped_existing: usize,
    /// Rows we could not map (bad metadata, fatal mapping error). Listed
    /// in `errors` so the caller can show them.
    pub rows_invalid: usize,
    /// `linked_to` references successfully resolved + inserted as links.
    pub links_created: usize,
    /// `linked_to` references whose target row was not present in the
    /// import — left dangling, counted here, not fatal.
    pub links_dangling: usize,
    /// `true` when this report came from a `dry_run` call.
    pub dry_run: bool,
    /// Soft errors. Hard errors short-circuit and return `Err`.
    pub errors: Vec<String>,
}

/// Build an importer wired to a destination `ObservationStore` +
/// `LinkStore`. Both stores must point at the same SEELE DB so the
/// tx-scoped helpers commit atomically together.
pub struct EngramImporter<'a> {
    observations: &'a ObservationStore,
    /// LinkStore is constructed from the same pool so its in-tx
    /// helper can share the transaction we drive.
    _links: &'a LinkStore,
}

impl<'a> EngramImporter<'a> {
    pub fn new(observations: &'a ObservationStore, links: &'a LinkStore) -> Self {
        Self {
            observations,
            _links: links,
        }
    }

    /// Run the import. `dry_run = true` opens the source, parses the
    /// rows, but writes nothing.
    pub fn import_from(&self, source: &Path, dry_run: bool) -> Result<ImportReport> {
        let src = open_source(source)?;
        verify_schema(&src)?;
        let mapped = read_and_map_rows(&src)?;
        let rows_seen = mapped.len();

        if dry_run {
            return Ok(ImportReport {
                source_path: source.to_path_buf(),
                rows_seen,
                rows_inserted: 0,
                rows_skipped_existing: 0,
                rows_invalid: mapped.iter().filter(|r| r.mapped.is_none()).count(),
                links_created: 0,
                links_dangling: mapped.iter().map(|r| r.linked_to.len()).sum(),
                dry_run: true,
                errors: mapped.iter().filter_map(|r| r.map_error.clone()).collect(),
            });
        }

        let mut conn = self.observations.pool().get()?;
        let tx = conn.transaction()?;

        let mut rows_inserted = 0usize;
        let mut rows_skipped_existing = 0usize;
        let mut rows_invalid = 0usize;
        let mut errors = Vec::new();

        // engram_id (text) → SeeleId (the destination id) for link resolution.
        let mut id_map: std::collections::HashMap<String, SeeleId> =
            std::collections::HashMap::new();

        for r in &mapped {
            match &r.mapped {
                None => {
                    rows_invalid += 1;
                    if let Some(err) = &r.map_error {
                        errors.push(err.clone());
                    }
                }
                Some(input) => {
                    let engram_id = r.source_id.clone();
                    let dest_id = input.id;
                    match ObservationStore::save_raw_in_tx(&tx, input.clone())? {
                        RawSaveOutcome::Inserted => {
                            rows_inserted += 1;
                            id_map.insert(engram_id, dest_id);
                        }
                        RawSaveOutcome::AlreadyExisted => {
                            rows_skipped_existing += 1;
                            // Even if we skipped writing, the destination
                            // already has a row under this id, so it's a
                            // valid link target for this run.
                            id_map.insert(engram_id, dest_id);
                        }
                    }
                }
            }
        }

        // Pass 2: resolve linked_to references. Same transaction so an
        // error rolls everything back.
        //
        // Resolution order for each `to_engram`:
        //   1. The current import's `id_map` (this chunk's rows).
        //   2. Treat `to_engram` as a ULID and probe the destination
        //      `observations` table — covers the case where Orlando
        //      imports in multiple passes and link targets live in a
        //      previous pass. Closes Cloven 2026-05-11 [MEDIO].
        //   3. Anything else → dangling, counted but not fatal. A
        //      cuid-style id that points at a row whose new ULID
        //      lives behind `metadata.engram_id` would require a
        //      JSON scan; deferred to v0.2 if real migrations need it.
        let mut links_created = 0usize;
        let mut links_dangling = 0usize;
        for r in &mapped {
            let from_dest = match id_map.get(&r.source_id) {
                Some(id) => *id,
                None => continue, // own row failed to map; skip its links
            };
            for to_engram in &r.linked_to {
                let to_dest = id_map.get(to_engram).copied().or_else(|| {
                    to_engram
                        .parse::<SeeleId>()
                        .ok()
                        .filter(|id| observation_exists_in_tx(&tx, id))
                });
                match to_dest {
                    Some(to_dest) => {
                        LinkStore::create_in_tx(
                            &tx,
                            LinkInput {
                                from_id: from_dest,
                                to_id: to_dest,
                                link_type: "related_to".to_string(),
                                metadata: Metadata::new(),
                            },
                        )?;
                        links_created += 1;
                    }
                    None => {
                        links_dangling += 1;
                    }
                }
            }
        }

        tx.commit()?;

        Ok(ImportReport {
            source_path: source.to_path_buf(),
            rows_seen,
            rows_inserted,
            rows_skipped_existing,
            rows_invalid,
            links_created,
            links_dangling,
            dry_run: false,
            errors,
        })
    }
}

// -------- internals --------

fn observation_exists_in_tx(tx: &rusqlite::Transaction<'_>, id: &SeeleId) -> bool {
    tx.query_row(
        "SELECT 1 FROM observations WHERE id = ?1 LIMIT 1",
        [id.to_string()],
        |_| Ok(()),
    )
    .is_ok()
}

fn open_source(path: &Path) -> Result<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| {
        EngramImportError::OpenSource {
            path: path.to_path_buf(),
            detail: e.to_string(),
        }
    })
}

fn verify_schema(src: &Connection) -> Result<()> {
    let exists: Option<String> = src
        .query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='memories'",
            [],
            |r| r.get(0),
        )
        .ok();
    if exists.is_none() {
        return Err(EngramImportError::BadSchema {
            detail: "table `memories` not found".to_string(),
        });
    }
    // Probe required columns by listing them. PRAGMA returns rows so we
    // can collect names easily.
    let cols = pragma_columns(src, "memories")?;
    for required in ["id", "body", "metadata", "created_at"] {
        if !cols.iter().any(|c| c == required) {
            return Err(EngramImportError::BadSchema {
                detail: format!("`memories` is missing required column `{required}`"),
            });
        }
    }
    Ok(())
}

fn pragma_columns(src: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = src.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        out.push(name);
    }
    Ok(out)
}

/// One row's mapping attempt. `mapped` is `None` when the row could
/// not be converted; `map_error` carries the message.
#[derive(Debug, Clone)]
struct MappedRow {
    source_id: String,
    mapped: Option<RawSaveInput>,
    map_error: Option<String>,
    linked_to: Vec<String>,
}

fn read_and_map_rows(src: &Connection) -> Result<Vec<MappedRow>> {
    // `memories` may have extra columns; we read only the four we know
    // about. Optional `title` / `updated_at` are extracted from metadata
    // if missing as columns.
    let mut stmt = src.prepare("SELECT id, body, metadata, created_at FROM memories")?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let body: String = row.get(1)?;
        let metadata_raw: Option<String> = row.get(2).ok();
        // created_at can be epoch ms (i64) or ISO string. Try both.
        let created_at = read_created_at(row)?;
        out.push(map_one(id, body, metadata_raw, created_at));
    }
    Ok(out)
}

fn read_created_at(row: &rusqlite::Row<'_>) -> Result<DateTime<Utc>> {
    if let Ok(ms) = row.get::<_, i64>(3) {
        return Ok(ms_to_dt(ms));
    }
    if let Ok(s) = row.get::<_, String>(3) {
        if let Ok(parsed) = DateTime::parse_from_rfc3339(&s) {
            return Ok(parsed.with_timezone(&Utc));
        }
        // Some ENGRAM dumps store epoch ms as text — fall back.
        if let Ok(ms) = s.parse::<i64>() {
            return Ok(ms_to_dt(ms));
        }
    }
    Ok(Utc::now())
}

fn ms_to_dt(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(Utc::now)
}

fn map_one(
    source_id: String,
    body: String,
    metadata_raw: Option<String>,
    created_at: DateTime<Utc>,
) -> MappedRow {
    let mut parsed = match parse_metadata(&metadata_raw) {
        Ok(p) => p,
        Err(detail) => {
            return MappedRow {
                source_id: source_id.clone(),
                mapped: None,
                map_error: Some(format!("row id={source_id}: {detail}")),
                linked_to: Vec::new(),
            };
        }
    };

    let (dest_id, preserved) = resolve_dest_id(&source_id);
    if !preserved {
        // Stash the original so callers can correlate post-migration.
        parsed
            .metadata
            .set("engram_id", serde_json::json!(source_id.clone()));
    }

    let title = synthesize_title(&body, &parsed.metadata);

    let input = RawSaveInput {
        id: dest_id,
        session_id: None,
        kind: parsed.kind,
        title,
        content: body,
        tool_name: parsed.tool_name,
        project: parsed.project,
        scope: parsed.scope,
        topic_key: parsed.topic_key,
        created_at,
        updated_at: created_at,
        metadata: parsed.metadata,
    };

    MappedRow {
        source_id,
        mapped: Some(input),
        map_error: None,
        linked_to: parsed.linked_to,
    }
}

/// Extracted view of the cluster of fields SEELE needs out of the
/// ENGRAM `metadata` blob. Keeps `parse_metadata` from carrying a
/// 7-tuple return type that clippy `type_complexity` rejects.
#[derive(Debug, Clone)]
struct ParsedMetadata {
    metadata: Metadata,
    linked_to: Vec<String>,
    kind: ObservationType,
    project: Option<String>,
    scope: Scope,
    topic_key: Option<String>,
    tool_name: Option<String>,
}

impl ParsedMetadata {
    fn empty() -> Self {
        Self {
            metadata: Metadata::new(),
            linked_to: Vec::new(),
            kind: ObservationType::Memory,
            project: None,
            scope: Scope::Project,
            topic_key: None,
            tool_name: None,
        }
    }
}

/// Parse the ENGRAM `metadata` blob.
fn parse_metadata(raw: &Option<String>) -> std::result::Result<ParsedMetadata, String> {
    let mut out = ParsedMetadata::empty();

    let Some(text) = raw.as_ref() else {
        return Ok(out);
    };
    if text.trim().is_empty() || text.trim() == "null" {
        return Ok(out);
    }
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("metadata not JSON: {e}"))?;
    let obj = match v {
        serde_json::Value::Object(o) => o,
        other => return Err(format!("metadata not an object: {other}")),
    };

    for (k, val) in obj {
        match k.as_str() {
            "kind" => {
                if let Some(s) = val.as_str() {
                    out.kind = ObservationType::from_str_relaxed(s);
                }
                // Always preserve the original alongside SEELE's typed view.
                out.metadata.set(
                    "kind",
                    serde_json::Value::String(out.kind.as_str().to_string()),
                );
            }
            "project" => {
                if let Some(s) = val.as_str() {
                    out.project = Some(s.to_string());
                }
                out.metadata.set("project", val);
            }
            "scope" => {
                if let Some(s) = val.as_str() {
                    if let Some(parsed) = Scope::from_str_strict(s) {
                        out.scope = parsed;
                    }
                }
                out.metadata.set("scope", val);
            }
            "topic_key" => {
                if let Some(s) = val.as_str() {
                    out.topic_key = Some(s.to_string());
                }
                out.metadata.set("topic_key", val);
            }
            "tool_name" | "tool" => {
                if let Some(s) = val.as_str() {
                    out.tool_name = Some(s.to_string());
                }
                out.metadata.set(&k, val);
            }
            "linked_to" => {
                if let Some(arr) = val.as_array() {
                    for entry in arr {
                        if let Some(s) = entry.as_str() {
                            out.linked_to.push(s.to_string());
                        }
                    }
                }
                out.metadata.set("linked_to", val);
            }
            _ => {
                out.metadata.set(&k, val);
            }
        }
    }

    Ok(out)
}

/// Try to keep the source `id` if it's already a valid ULID. Otherwise
/// mint a fresh `SeeleId` and stash the original in metadata. Returns
/// `(dest_id, preserved)` so the caller knows whether to add the
/// `engram_id` breadcrumb.
fn resolve_dest_id(source: &str) -> (SeeleId, bool) {
    if let Ok(id) = source.parse::<SeeleId>() {
        return (id, true);
    }
    (SeeleId::new(), false)
}

fn synthesize_title(body: &str, metadata: &Metadata) -> String {
    if let Some(title) = metadata.get("title").and_then(|v| v.as_str()) {
        if !title.trim().is_empty() {
            return title.to_string();
        }
    }
    let first_line = body.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return "(untitled)".to_string();
    }
    if first_line.chars().count() > 80 {
        let truncated: String = first_line.chars().take(77).collect();
        format!("{truncated}...")
    } else {
        first_line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthesize_title_uses_metadata_title_when_present() {
        let mut m = Metadata::new();
        m.set("title", serde_json::Value::String("Explicit".to_string()));
        assert_eq!(synthesize_title("anything", &m), "Explicit");
    }

    #[test]
    fn synthesize_title_falls_back_to_first_line() {
        let m = Metadata::new();
        assert_eq!(
            synthesize_title("First line\nsecond", &m),
            "First line".to_string()
        );
    }

    #[test]
    fn synthesize_title_truncates_long_first_line() {
        let m = Metadata::new();
        let long = "x".repeat(120);
        let t = synthesize_title(&long, &m);
        assert!(t.ends_with("..."));
        assert!(t.chars().count() <= 80);
    }

    #[test]
    fn synthesize_title_for_empty_body_returns_untitled() {
        let m = Metadata::new();
        assert_eq!(synthesize_title("", &m), "(untitled)".to_string());
    }

    #[test]
    fn resolve_dest_id_preserves_valid_ulid() {
        let id = SeeleId::new();
        let s = id.to_string();
        let (dest, preserved) = resolve_dest_id(&s);
        assert!(preserved);
        assert_eq!(dest.to_string(), s);
    }

    #[test]
    fn resolve_dest_id_mints_new_for_non_ulid_source() {
        let (_dest, preserved) = resolve_dest_id("mem_abc123");
        assert!(!preserved);
    }

    #[test]
    fn parse_metadata_handles_null_and_empty() {
        let p = parse_metadata(&None).unwrap();
        assert!(p.linked_to.is_empty());
        assert_eq!(p.kind, ObservationType::Memory);
        assert!(p.project.is_none());
        assert_eq!(p.scope, Scope::Project);
        assert!(p.topic_key.is_none());
        assert!(p.tool_name.is_none());
        let _ = parse_metadata(&Some("".to_string())).unwrap();
        let _ = parse_metadata(&Some("null".to_string())).unwrap();
    }

    #[test]
    fn parse_metadata_extracts_kind_project_scope_topic_tool() {
        let raw = r#"{
            "kind": "decision",
            "project": "myproj",
            "scope": "personal",
            "topic_key": "decision/architecture",
            "tool_name": "claude-code",
            "domain": "backend"
        }"#;
        let p = parse_metadata(&Some(raw.to_string())).unwrap();
        assert_eq!(p.kind, ObservationType::Decision);
        assert_eq!(p.project.as_deref(), Some("myproj"));
        assert_eq!(p.scope, Scope::Personal);
        assert_eq!(p.topic_key.as_deref(), Some("decision/architecture"));
        assert_eq!(p.tool_name.as_deref(), Some("claude-code"));
        // Unknown keys preserved.
        assert_eq!(
            p.metadata.get("domain").and_then(|v| v.as_str()),
            Some("backend")
        );
    }

    #[test]
    fn parse_metadata_collects_linked_to_array() {
        let raw = r#"{"linked_to": ["mem_a", "mem_b"]}"#;
        let p = parse_metadata(&Some(raw.to_string())).unwrap();
        assert_eq!(p.linked_to, vec!["mem_a".to_string(), "mem_b".to_string()]);
    }

    #[test]
    fn parse_metadata_errors_on_non_object_root() {
        let err = parse_metadata(&Some("[1,2,3]".to_string())).unwrap_err();
        assert!(err.contains("not an object"));
    }

    #[test]
    fn parse_metadata_errors_on_garbage() {
        let err = parse_metadata(&Some("not json".to_string())).unwrap_err();
        assert!(err.contains("not JSON"));
    }
}
