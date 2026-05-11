//! Git-friendly compressed-chunk sync between machines.
//!
//! Export selects observations matching a filter (by `project` for v0.1),
//! serializes them to JSON, gzips, and writes to a single
//! `<chunk_id>.json.gz` file. `chunk_id` is the SHA-256 hex of the
//! deterministic part of the payload — two independent exports of the
//! same observation set produce the same chunk, so they dedup naturally
//! when imported.
//!
//! Import reads the chunk, checks `ChunkStore::was_imported(target_key,
//! chunk_id)`, skips it if already imported (idempotent per target),
//! otherwise calls `ObservationStore::save` for each observation.
//!
//! v0.1 ships a single chunk per export. Sprint-05 may add a size-bound
//! splitter (~1 MB per chunk) if exports grow large enough to matter.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use seele_core::memory::Observation;
use seele_storage::{ChunkStore, ObservationQuery, ObservationStore, SaveInput};

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("storage: {0}")]
    Storage(#[from] seele_storage::StorageError),
    #[error("invalid chunk: {0}")]
    InvalidChunk(String),
}

pub type Result<T> = std::result::Result<T, SyncError>;

/// On-disk payload format. Versioned so future format changes can be
/// detected during import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkPayload {
    /// Format version. Bump when the wire layout changes
    /// non-backwards-compatibly. v0.1 = 1.
    pub format_version: u32,
    /// Crate version that produced this chunk. Informational.
    pub seele_version: String,
    pub exported_at: DateTime<Utc>,
    /// `Some` when the export was filtered by project.
    pub project: Option<String>,
    pub observations: Vec<Observation>,
}

impl ChunkPayload {
    pub const CURRENT_FORMAT_VERSION: u32 = 1;
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportReport {
    pub chunk_id: String,
    pub path: PathBuf,
    pub observation_count: usize,
    /// Size of the on-disk gzipped file in bytes.
    pub bytes_on_disk: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub chunk_id: String,
    pub path: PathBuf,
    /// If the chunk was already imported under this target, this is
    /// `ImportOutcome::AlreadyImported` and no observations were saved.
    pub outcome: ImportOutcome,
    pub observation_count_saved: usize,
    pub observation_count_skipped: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportOutcome {
    /// Brand new chunk for this target_key; observations saved.
    Imported,
    /// Chunk was previously imported under this target_key; no-op.
    AlreadyImported,
}

// -------- Export --------

/// Filter passed to `export`. v0.1 only filters by project; extend later
/// for date ranges / scopes / types.
#[derive(Debug, Clone, Default)]
pub struct ExportFilter {
    pub project: Option<String>,
}

/// Build a `ChunkPayload` by querying observations through the store.
/// Public so callers that want to inspect or transform before writing
/// can call this directly.
pub fn build_chunk(observations: &ObservationStore, filter: ExportFilter) -> Result<ChunkPayload> {
    let q = ObservationQuery {
        project: filter.project.clone(),
        include_deleted: false,
        ..Default::default()
    };
    let observations = observations.list(q)?;
    Ok(ChunkPayload {
        format_version: ChunkPayload::CURRENT_FORMAT_VERSION,
        seele_version: env!("CARGO_PKG_VERSION").to_string(),
        exported_at: Utc::now(),
        project: filter.project,
        observations,
    })
}

/// Compute the canonical `chunk_id` for a payload.
///
/// We hash:
/// - `format_version` (little-endian bytes).
/// - `project` (bytes + null terminator).
/// - Observations serialized as JSON, but **sorted by id** first so
///   in-memory ordering does not affect the hash.
///
/// `exported_at` and `seele_version` are excluded so the same set of
/// observations produces the same chunk_id across runs.
pub fn compute_chunk_id(payload: &ChunkPayload) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(payload.format_version.to_le_bytes());
    if let Some(p) = &payload.project {
        hasher.update(p.as_bytes());
    }
    hasher.update([0u8]);
    let mut obs = payload.observations.clone();
    obs.sort_by_key(|o| o.id.to_string());
    let bytes = serde_json::to_vec(&obs)?;
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

/// Write a chunk to `dir` as `<chunk_id>.json.gz`. Returns the chunk_id
/// and the full path. Idempotent at the filesystem layer: if a file
/// with that chunk_id already exists, it's overwritten with content
/// that matches by construction (same chunk_id implies same hashed
/// payload).
pub fn export_to_dir(
    observations: &ObservationStore,
    dir: &Path,
    filter: ExportFilter,
) -> Result<ExportReport> {
    let payload = build_chunk(observations, filter)?;
    let chunk_id = compute_chunk_id(&payload)?;
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{chunk_id}.json.gz"));
    let observation_count = payload.observations.len();
    write_chunk_file(&path, &payload)?;
    let bytes_on_disk = std::fs::metadata(&path)?.len();
    Ok(ExportReport {
        chunk_id,
        path,
        observation_count,
        bytes_on_disk,
    })
}

fn write_chunk_file(path: &Path, payload: &ChunkPayload) -> Result<()> {
    let json = serde_json::to_vec(payload)?;
    let file = File::create(path)?;
    let mut encoder = GzEncoder::new(BufWriter::new(file), Compression::default());
    encoder.write_all(&json)?;
    encoder.finish()?.flush()?;
    Ok(())
}

// -------- Import --------

/// Read a chunk file (gzipped JSON) into a `ChunkPayload`. Verifies the
/// recomputed chunk_id matches the filename's stem when the filename
/// follows the canonical `<chunk_id>.json.gz` shape — a small integrity
/// check that catches silent corruption.
pub fn read_chunk_file(path: &Path) -> Result<(String, ChunkPayload)> {
    let file = File::open(path)?;
    let mut decoder = GzDecoder::new(BufReader::new(file));
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf)?;
    let payload: ChunkPayload = serde_json::from_slice(&buf)?;
    if payload.format_version > ChunkPayload::CURRENT_FORMAT_VERSION {
        return Err(SyncError::InvalidChunk(format!(
            "unsupported format_version {}; max supported = {}",
            payload.format_version,
            ChunkPayload::CURRENT_FORMAT_VERSION,
        )));
    }
    let recomputed = compute_chunk_id(&payload)?;
    let filename_stem = path
        .file_name()
        .and_then(|s| s.to_str())
        .and_then(|s| s.strip_suffix(".json.gz"))
        .unwrap_or("");
    if filename_stem.len() == 64
        && filename_stem.chars().all(|c| c.is_ascii_hexdigit())
        && filename_stem != recomputed
    {
        return Err(SyncError::InvalidChunk(format!(
            "chunk_id mismatch: file says {filename_stem}, recomputed {recomputed}"
        )));
    }
    Ok((recomputed, payload))
}

/// Import a chunk into `observations`. `target_key` identifies this
/// local node (e.g., hostname or stable UUID); the same chunk
/// re-imported under the same target_key is a no-op.
pub fn import_from_file(
    observations: &ObservationStore,
    chunks: &ChunkStore,
    target_key: &str,
    path: &Path,
) -> Result<ImportReport> {
    let (chunk_id, payload) = read_chunk_file(path)?;
    if chunks.was_imported(target_key, &chunk_id)? {
        return Ok(ImportReport {
            chunk_id,
            path: path.to_path_buf(),
            outcome: ImportOutcome::AlreadyImported,
            observation_count_saved: 0,
            observation_count_skipped: payload.observations.len(),
        });
    }

    let mut saved = 0usize;
    for obs in &payload.observations {
        let input = SaveInput {
            session_id: obs.session_id,
            kind: obs.kind.clone(),
            title: obs.title.clone(),
            content: obs.content.clone(),
            tool_name: obs.tool_name.clone(),
            project: obs.project.clone(),
            scope: obs.scope,
            topic_key: obs.topic_key.clone(),
            metadata: obs.metadata.clone(),
        };
        observations.save(input)?;
        saved += 1;
    }
    chunks.mark_imported(target_key, &chunk_id)?;
    Ok(ImportReport {
        chunk_id,
        path: path.to_path_buf(),
        outcome: ImportOutcome::Imported,
        observation_count_saved: saved,
        observation_count_skipped: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use seele_core::id::SeeleId;
    use seele_core::memory::{ObservationType, Scope};
    use seele_core::metadata::Metadata;

    fn fake_observation(title: &str) -> Observation {
        let now = Utc::now();
        Observation {
            id: SeeleId::new(),
            session_id: None,
            kind: ObservationType::Memory,
            title: title.to_string(),
            content: format!("content for {title}"),
            tool_name: None,
            project: Some("p".to_string()),
            scope: Scope::Project,
            topic_key: None,
            normalized_hash: None,
            revision_count: 0,
            duplicate_count: 0,
            last_seen_at: now,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            metadata: Metadata::new(),
        }
    }

    #[test]
    fn chunk_id_is_deterministic_for_same_observations() {
        let a = fake_observation("a");
        let b = fake_observation("b");
        let p1 = ChunkPayload {
            format_version: 1,
            seele_version: "0.0.99".to_string(),
            exported_at: Utc::now(),
            project: Some("p".to_string()),
            observations: vec![a.clone(), b.clone()],
        };
        let mut p2 = p1.clone();
        // exported_at + seele_version differ — must not affect the id.
        p2.exported_at = Utc::now();
        p2.seele_version = "0.0.100".to_string();
        // Reverse observation order — must not affect the id.
        p2.observations.reverse();

        let id1 = compute_chunk_id(&p1).unwrap();
        let id2 = compute_chunk_id(&p2).unwrap();
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 64);
    }

    #[test]
    fn chunk_id_differs_when_observations_differ() {
        let p1 = ChunkPayload {
            format_version: 1,
            seele_version: "0.0.99".to_string(),
            exported_at: Utc::now(),
            project: None,
            observations: vec![fake_observation("a")],
        };
        let mut p2 = p1.clone();
        p2.observations = vec![fake_observation("different")];
        assert_ne!(
            compute_chunk_id(&p1).unwrap(),
            compute_chunk_id(&p2).unwrap()
        );
    }
}
