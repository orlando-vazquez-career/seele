//! SEELE storage — SQLite + FTS5 + sqlite-vec layer.

pub mod chunks;
pub mod error;
pub mod hash;
pub mod links;
pub mod migrations;
pub mod observations;
pub mod pool;
pub mod privacy;
pub mod prompts;
pub mod relations;
pub mod sessions;
pub mod vec0_install;
pub mod vec0_loader;

pub use chunks::{ChunkStore, SyncChunk};
pub use links::{LinkInput, LinkQuery, LinkStore};
pub use observations::{
    EmbeddingMeta, EmbeddingModelCount, EmbeddingProvenance, ObservationPatch, ObservationQuery,
    ObservationStore, RawSaveInput, RawSaveOutcome, SaveInput, SaveOutcome,
};
pub use prompts::{PromptInput, PromptQuery, PromptStore, UserPrompt};
pub use relations::{JudgmentInput, RelationInput, RelationQuery, RelationStore};
pub use sessions::{SessionFilter, SessionInput, SessionStore};

pub use error::{Result, StorageError};
pub use pool::{init_pool, Pool, PoolConfig};

use std::path::Path;

/// Open a DB at `path`, applying canonical PRAGMAs, loading vec0, and
/// running pending migrations. Idempotent.
pub fn init_db(path: impl AsRef<Path>) -> Result<Pool> {
    let pool = init_pool(PoolConfig::with_path(path))?;
    let mut conn = pool.get()?;
    migrations::run_pending(&mut conn)?;
    Ok(pool)
}
