-- ============================================================
-- SEELE schema v0.1.0 — initial migration
-- See ADR-02 (genesis/plans/arquitectura/02-schema-sqlite.md)
--      ADR-10 (mapping MNEMA↔SEELE, context_mode)
--      ADR-11 (sqlite-vec vendored)
--
-- NOTE on int_id:
--   The arquitectura plan originally proposed a virtual generated column
--   computing int_id from SUBSTR(id, ...) as INTEGER. ULIDs are base32
--   strings, so CAST(letter AS INTEGER) yields 0 — the formula was broken.
--   We instead store int_id as a regular INTEGER column populated at
--   INSERT time from Rust (SeeleId::as_i64). FTS5 and vec0 both reference
--   it for stable rowid mapping.
-- ============================================================

-- 1. Sessions
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

CREATE INDEX idx_sessions_project    ON sessions(project);
CREATE INDEX idx_sessions_started_at ON sessions(started_at);
CREATE INDEX idx_sessions_status     ON sessions(status);

-- 2. Observations (core entity)
CREATE TABLE observations (
    id                TEXT PRIMARY KEY,     -- ULID
    int_id            INTEGER NOT NULL UNIQUE,  -- ULID random tail mapped to i64
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

-- 2a. Virtual generated columns over metadata JSON
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

-- 2b. Indexes (partial where useful)
CREATE INDEX idx_obs_session    ON observations(session_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_obs_project    ON observations(project)    WHERE deleted_at IS NULL;
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

-- 3. FTS5 virtual table over observations
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

-- 3a. Triggers to keep FTS in sync with INSERT/UPDATE/DELETE
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

-- 4. Vector table for embeddings (sqlite-vec)
CREATE VIRTUAL TABLE observations_vec USING vec0(
    embedding FLOAT[384]
);

-- 5. User prompts
CREATE TABLE user_prompts (
    id          TEXT PRIMARY KEY,
    session_id  TEXT REFERENCES sessions(id) ON DELETE SET NULL,
    content     TEXT NOT NULL,
    project     TEXT,
    created_at  INTEGER NOT NULL
) WITHOUT ROWID;

CREATE INDEX idx_prompts_session    ON user_prompts(session_id);
CREATE INDEX idx_prompts_project    ON user_prompts(project);
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
CREATE INDEX idx_links_to   ON links(to_id);
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

CREATE INDEX idx_relations_source   ON memory_relations(source_id);
CREATE INDEX idx_relations_target   ON memory_relations(target_id);
CREATE INDEX idx_relations_status   ON memory_relations(judgment_status);
CREATE INDEX idx_relations_relation ON memory_relations(relation);

-- 8. Sync chunks (git sync dedup)
CREATE TABLE sync_chunks (
    target_key  TEXT NOT NULL,
    chunk_id    TEXT NOT NULL,
    imported_at INTEGER NOT NULL,
    PRIMARY KEY (target_key, chunk_id)
) WITHOUT ROWID;
