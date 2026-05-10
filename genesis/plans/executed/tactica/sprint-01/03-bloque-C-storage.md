# Sprint-01 Bloque C — `seele-storage`

**Tema**: SQLite + FTS5 + sqlite-vec storage layer con migrations versionadas, CRUD completo, virtual columns, partial indexes, triggers FTS, soft delete, topic key upserts, normalized hash dedup, privacy stripping.

**Pre-requisitos**: bloques A (workspace) + B (core types) cerrados.

## Estructura del crate

```
crates/seele-storage/
├── Cargo.toml
├── src/
│   ├── lib.rs                # public API + re-exports
│   ├── error.rs              # StorageError + From<rusqlite::Error>
│   ├── pool.rs               # r2d2 SqliteConnectionManager
│   ├── migrations/
│   │   ├── mod.rs            # refinery embedded
│   │   └── V001__initial_schema.sql  # migration SQL
│   ├── sessions.rs           # CRUD sessions
│   ├── observations.rs       # CRUD observations + FTS + vec
│   ├── prompts.rs            # CRUD user_prompts
│   ├── links.rs              # CRUD links
│   ├── relations.rs          # CRUD memory_relations
│   ├── chunks.rs             # CRUD sync_chunks
│   ├── privacy.rs            # strip_private_tags() regex
│   ├── hash.rs               # normalized_hash() helper
│   └── upsert.rs             # topic_key upsert logic
└── tests/
    ├── migration_smoke.rs
    ├── observations_crud.rs
    ├── sessions_crud.rs
    └── fixtures.rs
```

## Migration V001

`crates/seele-storage/src/migrations/V001__initial_schema.sql`:

```sql
-- ============================================================
-- SEELE schema v0.1.0
-- See ADR-02 (genesis/plans/arquitectura/02-schema-sqlite.md).
-- ============================================================

-- 1. Schema version table
CREATE TABLE IF NOT EXISTS schema_version (
    version     INTEGER PRIMARY KEY,
    applied_at  INTEGER NOT NULL,
    description TEXT NOT NULL
);

-- 2. Sessions
CREATE TABLE sessions (
    id          TEXT PRIMARY KEY,           -- ULID
    project     TEXT NOT NULL,
    directory   TEXT,
    started_at  INTEGER NOT NULL,           -- epoch ms
    ended_at    INTEGER,
    summary     TEXT,
    status      TEXT NOT NULL DEFAULT 'active'
                CHECK (status IN ('active', 'ended', 'aborted'))
) WITHOUT ROWID;

CREATE INDEX idx_sessions_project ON sessions(project);
CREATE INDEX idx_sessions_started_at ON sessions(started_at);
CREATE INDEX idx_sessions_status ON sessions(status);

-- 3. Observations (core entity)
CREATE TABLE observations (
    id                TEXT PRIMARY KEY,     -- ULID
    session_id        TEXT REFERENCES sessions(id) ON DELETE SET NULL,
    type              TEXT NOT NULL,
    title             TEXT NOT NULL,
    content           TEXT NOT NULL,
    tool_name         TEXT,
    project           TEXT,
    scope             TEXT NOT NULL DEFAULT 'project'
                      CHECK (scope IN ('project', 'personal')),
    topic_key         TEXT,
    normalized_hash   TEXT,
    revision_count    INTEGER NOT NULL DEFAULT 0,
    duplicate_count   INTEGER NOT NULL DEFAULT 0,
    last_seen_at      INTEGER NOT NULL,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    deleted_at        INTEGER,
    metadata          TEXT NOT NULL DEFAULT '{}'  -- JSON
) WITHOUT ROWID;

-- 3a. Virtual generated columns (5 canonical fields)
ALTER TABLE observations ADD COLUMN meta_kind TEXT
    GENERATED ALWAYS AS (json_extract(metadata, '$.kind')) VIRTUAL;
ALTER TABLE observations ADD COLUMN meta_domain TEXT
    GENERATED ALWAYS AS (json_extract(metadata, '$.domain')) VIRTUAL;
ALTER TABLE observations ADD COLUMN meta_axiomatic INTEGER
    GENERATED ALWAYS AS (json_extract(metadata, '$.axiomatic')) VIRTUAL;
ALTER TABLE observations ADD COLUMN meta_score REAL
    GENERATED ALWAYS AS (json_extract(metadata, '$.score')) VIRTUAL;
ALTER TABLE observations ADD COLUMN meta_context_mode TEXT
    GENERATED ALWAYS AS (json_extract(metadata, '$.context_mode')) VIRTUAL;

-- 3b. int_id derived from ULID for vec0 mapping
ALTER TABLE observations ADD COLUMN int_id INTEGER UNIQUE
    GENERATED ALWAYS AS (
        (CAST(SUBSTR(id, 1, 1) AS INTEGER) * 1099511627776) +
        (CAST(SUBSTR(id, 2, 1) AS INTEGER) * 4294967296) +
        (CAST(SUBSTR(id, 3, 6) AS INTEGER))
    ) VIRTUAL;
-- Note: full ULID-to-int64 mapping is done in Rust (SeeleId::as_i64).
-- The virtual col here is approximate; real mapping uses Rust impl when inserting.
-- We keep this column for queries where we need to JOIN against memories_vec rowid.

-- 3c. Indexes (partial where possible)
CREATE INDEX idx_obs_session ON observations(session_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_obs_project ON observations(project) WHERE deleted_at IS NULL;
CREATE INDEX idx_obs_topic_upsert ON observations(project, scope, topic_key)
    WHERE topic_key IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX idx_obs_dedup ON observations(normalized_hash, project, scope, type, title)
    WHERE deleted_at IS NULL;
CREATE INDEX idx_obs_meta_kind ON observations(meta_kind)
    WHERE meta_kind IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX idx_obs_meta_domain ON observations(meta_domain)
    WHERE meta_domain IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX idx_obs_meta_kind_domain ON observations(meta_kind, meta_domain)
    WHERE deleted_at IS NULL;
CREATE INDEX idx_obs_meta_axiomatic ON observations(meta_axiomatic)
    WHERE meta_axiomatic = 1;
CREATE INDEX idx_obs_meta_score ON observations(meta_score)
    WHERE deleted_at IS NULL;
CREATE INDEX idx_obs_meta_context_mode ON observations(meta_context_mode)
    WHERE meta_context_mode IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX idx_obs_created_at ON observations(created_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_obs_deleted_at ON observations(deleted_at) WHERE deleted_at IS NOT NULL;

-- 4. FTS5 virtual table for observations
CREATE VIRTUAL TABLE observations_fts USING fts5(
    title,
    content,
    tool_name,
    type,
    project,
    content='observations',
    content_rowid='int_id',
    tokenize='porter unicode61 remove_diacritics 2'
);

-- 4a. Triggers to keep FTS in sync
CREATE TRIGGER observations_ai AFTER INSERT ON observations BEGIN
    INSERT INTO observations_fts(rowid, title, content, tool_name, type, project)
    VALUES (new.int_id, new.title, new.content, new.tool_name, new.type, new.project);
END;

CREATE TRIGGER observations_ad AFTER DELETE ON observations BEGIN
    INSERT INTO observations_fts(observations_fts, rowid, title, content, tool_name, type, project)
    VALUES('delete', old.int_id, old.title, old.content, old.tool_name, old.type, old.project);
END;

CREATE TRIGGER observations_au AFTER UPDATE ON observations BEGIN
    INSERT INTO observations_fts(observations_fts, rowid, title, content, tool_name, type, project)
    VALUES('delete', old.int_id, old.title, old.content, old.tool_name, old.type, old.project);
    INSERT INTO observations_fts(rowid, title, content, tool_name, type, project)
    VALUES (new.int_id, new.title, new.content, new.tool_name, new.type, new.project);
END;

-- 5. User prompts
CREATE TABLE user_prompts (
    id          TEXT PRIMARY KEY,
    session_id  TEXT REFERENCES sessions(id) ON DELETE SET NULL,
    content     TEXT NOT NULL,
    project     TEXT,
    created_at  INTEGER NOT NULL
) WITHOUT ROWID;

CREATE INDEX idx_prompts_session ON user_prompts(session_id);
CREATE INDEX idx_prompts_project ON user_prompts(project);
CREATE INDEX idx_prompts_created_at ON user_prompts(created_at);

CREATE VIRTUAL TABLE prompts_fts USING fts5(
    content,
    project,
    content='user_prompts',
    tokenize='porter unicode61 remove_diacritics 2'
);

-- 6. Links (general purpose graph)
CREATE TABLE links (
    id          TEXT PRIMARY KEY,
    from_id     TEXT NOT NULL REFERENCES observations(id) ON DELETE CASCADE,
    to_id       TEXT NOT NULL REFERENCES observations(id) ON DELETE CASCADE,
    link_type   TEXT NOT NULL,
    metadata    TEXT NOT NULL DEFAULT '{}',
    created_at  INTEGER NOT NULL,
    UNIQUE(from_id, to_id, link_type)
) WITHOUT ROWID;

CREATE INDEX idx_links_from ON links(from_id);
CREATE INDEX idx_links_to ON links(to_id);
CREATE INDEX idx_links_type ON links(link_type);

-- 7. Memory relations (judgment lifecycle, ENGRAM-inherited)
CREATE TABLE memory_relations (
    id                  TEXT PRIMARY KEY,
    sync_id             TEXT NOT NULL UNIQUE,
    source_id           TEXT NOT NULL REFERENCES observations(id) ON DELETE CASCADE,
    target_id           TEXT NOT NULL REFERENCES observations(id) ON DELETE CASCADE,
    relation            TEXT NOT NULL,
    judgment_status     TEXT NOT NULL DEFAULT 'pending'
                        CHECK (judgment_status IN ('pending', 'judged', 'orphaned', 'ignored')),
    reason              TEXT,
    evidence            TEXT,
    confidence          REAL,
    marked_by_actor     TEXT,
    marked_by_kind      TEXT,
    marked_by_model     TEXT,
    session_id          TEXT REFERENCES sessions(id) ON DELETE SET NULL,
    created_at          INTEGER NOT NULL
) WITHOUT ROWID;

CREATE INDEX idx_relations_source ON memory_relations(source_id);
CREATE INDEX idx_relations_target ON memory_relations(target_id);
CREATE INDEX idx_relations_status ON memory_relations(judgment_status);
CREATE INDEX idx_relations_relation ON memory_relations(relation);

-- 8. Sync chunks (git sync dedup)
CREATE TABLE sync_chunks (
    target_key  TEXT NOT NULL,
    chunk_id    TEXT NOT NULL,
    imported_at INTEGER NOT NULL,
    PRIMARY KEY (target_key, chunk_id)
) WITHOUT ROWID;

-- 9. Schema version row
INSERT INTO schema_version (version, applied_at, description)
VALUES (1, strftime('%s','now')*1000, 'initial schema v0.1.0');
```

## Funciones públicas clave de `seele-storage`

### `lib.rs`

```rust
pub mod error;
pub mod pool;
pub mod sessions;
pub mod observations;
pub mod prompts;
pub mod links;
pub mod relations;
pub mod chunks;
pub mod privacy;
pub mod hash;
pub mod upsert;

mod migrations;

pub use error::StorageError;
pub use pool::{Pool, PoolConfig, init_pool};
pub use observations::{ObservationStore, SaveOutcome};
pub use sessions::SessionStore;

/// Initialize a fresh DB with schema applied. Idempotent.
pub fn init_db(path: &std::path::Path) -> Result<Pool, StorageError> {
    let pool = pool::init_pool(PoolConfig::with_path(path))?;
    migrations::run_pending(&pool)?;
    Ok(pool)
}
```

### `pool.rs`

```rust
pub type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;

pub struct PoolConfig {
    pub path: PathBuf,
    pub max_size: u32,
    pub load_extensions: Vec<PathBuf>, // sqlite-vec
}

pub fn init_pool(cfg: PoolConfig) -> Result<Pool, StorageError> {
    let manager = SqliteConnectionManager::file(&cfg.path)
        .with_init(|conn| {
            conn.execute_batch("
                PRAGMA journal_mode=WAL;
                PRAGMA synchronous=NORMAL;
                PRAGMA foreign_keys=ON;
                PRAGMA temp_store=MEMORY;
            ")?;
            // Load sqlite-vec extension (path resolved at runtime).
            // Implementación detallada: detectar OS, usar binary correcto.
            Ok(())
        });
    let pool = r2d2::Pool::builder()
        .max_size(cfg.max_size)
        .build(manager)?;
    Ok(pool)
}
```

### `observations.rs` — operaciones clave

```rust
pub struct ObservationStore {
    pool: Pool,
}

#[derive(Debug, Clone)]
pub enum SaveOutcome {
    Created(SeeleId),
    UpsertedTopic { id: SeeleId, revision_count: u32 },
    DuplicateMerged { id: SeeleId, duplicate_count: u32 },
}

impl ObservationStore {
    pub fn save(&self, input: SaveInput) -> Result<SaveOutcome, StorageError> {
        // 1. Apply privacy stripping to title + content.
        // 2. Compute normalized_hash from stripped content.
        // 3. Check topic_key upsert window.
        // 4. Check normalized_hash dedup window.
        // 5. INSERT or UPDATE accordingly.
        // 6. Return SaveOutcome.
    }

    pub fn get(&self, id: SeeleId) -> Result<Option<Observation>, StorageError> { ... }
    pub fn list(&self, query: ObservationQuery) -> Result<Vec<Observation>, StorageError> { ... }
    pub fn update(&self, id: SeeleId, patch: ObservationPatch) -> Result<(), StorageError> { ... }
    pub fn soft_delete(&self, id: SeeleId) -> Result<(), StorageError> { ... }
    pub fn restore(&self, id: SeeleId) -> Result<(), StorageError> { ... }
    pub fn hard_delete(&self, id: SeeleId) -> Result<(), StorageError> { ... }
}
```

### `privacy.rs`

```rust
use once_cell::sync::Lazy;
use regex::Regex;

static PRIVATE_TAG_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?si)<private>.*?</private>").unwrap()
});

pub fn strip_private_tags(s: &str) -> String {
    PRIVATE_TAG_RE.replace_all(s, "").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_private_block() {
        let input = "Public <private>secret data</private> more public.";
        assert_eq!(strip_private_tags(input), "Public  more public.");
    }

    #[test]
    fn strips_multiline() {
        let input = "Hi\n<private>\nlinea 1\nlinea 2\n</private>\nbye";
        let out = strip_private_tags(input);
        assert!(!out.contains("linea 1"));
        assert!(out.contains("Hi"));
        assert!(out.contains("bye"));
    }

    #[test]
    fn no_private_unchanged() {
        let input = "no secrets here";
        assert_eq!(strip_private_tags(input), input);
    }
}
```

### `hash.rs`

```rust
use sha2::{Digest, Sha256};

/// Compute normalized hash for dedup.
/// Strategy: lowercase + collapse whitespace + sha256 hex.
pub fn normalized_hash(content: &str) -> String {
    let normalized: String = content
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let digest = Sha256::digest(normalized.as_bytes());
    format!("{:x}", digest)
}
```

### `upsert.rs`

Logic central de `save()`:

1. Si `topic_key` está presente y existe row activa con mismo `(project, scope, topic_key)`:
   - UPDATE existing: revision_count++, content/metadata reemplazados, last_seen_at = now, updated_at = now.
   - Return `SaveOutcome::UpsertedTopic`.
2. Sino, calcular `normalized_hash` y buscar dup window (último día, mismo `(hash, project, scope, type, title)`):
   - Si existe: UPDATE existing duplicate_count++, last_seen_at = now.
   - Return `SaveOutcome::DuplicateMerged`.
3. Sino, INSERT nuevo: SaveOutcome::Created.

## Tests del bloque C

- `tests/migration_smoke.rs` — verifica que migration crea todas las tablas + indexes + virtual cols + triggers + 1 schema_version row.
- `tests/observations_crud.rs` — save/get/list/update/soft_delete/restore happy paths + edge cases.
- `tests/observations_upsert.rs` — topic_key reuse → revision++; dedup hash → duplicate++.
- `tests/observations_fts.rs` — FTS sync con trigger; soft-deleted excluido por default.
- `tests/sessions_crud.rs` — start/end/summary lifecycle.
- `tests/links_crud.rs` — create/list/delete; UNIQUE constraint funciona.
- `tests/relations_crud.rs` — judgment lifecycle pending → judged.
- `tests/privacy.rs` — strip_private_tags property tests.
- `tests/fixtures.rs` — helper para crear DB temporal con datos.

Total: ~20-30 tests.

## Criterios de aceptación

1. `cargo build -p seele-storage` verde.
2. `cargo test -p seele-storage` verde con 20+ tests.
3. `cargo clippy -p seele-storage -- -D warnings` verde.
4. Migration aplicable contra DB vacía produce todas las estructuras esperadas.
5. Privacy stripping cubre los casos: single-line, multiline, sin tags, anidado, malformed (no <private> close tag → no se strippea).
6. Topic key upsert: la misma topic_key dos veces incrementa `revision_count` a 1, no crea row nuevo.
7. Normalized hash dedup: misma content + misma key dentro de 24h incrementa `duplicate_count`.

## Nota sobre sqlite-vec — vendored

Decisión 2026-05-10 (ADR-11): los binarios `vec0.{so,dylib,dll}` para los 5
targets soportados están **vendorizados** en `crates/seele-storage/vendor/sqlite-vec/`
y se embeben en el ejecutable final via `include_bytes!` (módulo
`vec0_loader`, ya wireado en bloque-A). Esto hace que `cargo install seele`
funcione out-of-the-box sin descargas de runtime.

Plan de carga en bloque-C:

1. En `pool::init_pool`, antes del `with_init`, llamar a un helper
   `vec0_loader::ensure_vec0_extension_path()` que:
   - Obtiene `vec0_loader::vec0_bytes()` (Some/None según target).
   - Si `None` (e.g. android, iOS, 32-bit linux): retorna `StorageError::VecNotSupportedTarget`.
   - Si `Some(bytes)`: escribe a `~/.cache/seele/vec0-<sha256[..16]><suffix>`,
     idempotente con verificación de hash. Retorna ese `PathBuf`.
2. En `with_init`, llamar `conn.load_extension(&path, None)` con el path obtenido.
3. La columna `embedding BLOB` y el `observations_vec` virtual table
   quedan creados en migration V001:

```sql
-- Al final de V001:
CREATE VIRTUAL TABLE observations_vec USING vec0(
    embedding FLOAT[384]
);
```

Para sprint-01 NO populamos `observations_vec` (eso es sprint-02 con embedder).
Pero el vec0 virtual table debe crearse en la migration porque carga la
extensión durante el init y verifica que funcione end-to-end.

Override para usuarios avanzados: env var `SEELE_VEC_PATH=/path/to/vec0.so`
fuerza el uso de un binary externo en vez del vendored. Útil para debugging
o targets no soportados.

Tests requeridos en bloque C:

- `tests/vec0_extension.rs` — verifica que `init_db` carga la extensión y
  el `observations_vec` virtual table responde a `SELECT vec_version()`.

## Commit del bloque C

```
git add -A
git commit -m "sprint-01 bloque-C — seele-storage (SQLite + FTS5 + vec0 + CRUD)"
git push
```
