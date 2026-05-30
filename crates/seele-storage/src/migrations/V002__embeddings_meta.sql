-- ============================================================
-- ADR-14: embeddings_meta — provenance of each observation's embedding.
--
-- Hard prerequisite of A2 (embedder swap) and B3 (Contextual Retrieval):
-- heterogeneous vectors (different model_id / dim, or contextualized vs
-- not) must never be mixed silently in the same vec0 table. This migration
-- ships the TABLE + BACKFILL only; search does NOT consume it yet (that
-- arrives with A2/B3 in later sprints).
-- ============================================================

CREATE TABLE embeddings_meta (
    observation_id  TEXT PRIMARY KEY REFERENCES observations(id) ON DELETE CASCADE,
    model_id        TEXT NOT NULL,               -- e.g. 'all-MiniLM-L6-v2'
    dim             INTEGER NOT NULL,             -- 384, 768, ...
    contextualized  INTEGER NOT NULL DEFAULT 0,   -- 1 if B3 rewrote the body before embedding
    created_at      INTEGER NOT NULL              -- unix ms, mirrors observations.created_at
) WITHOUT ROWID;

CREATE INDEX idx_embeddings_meta_model ON embeddings_meta(model_id);

-- Backfill: every observation present at upgrade time was embedded with the
-- v0.1/v0.2 default (all-MiniLM-L6-v2, 384-dim, non-contextualized) — the
-- only embedder shipped to date. Rows for an observation that happens to
-- lack a real vector are harmless provenance and are corrected on the next
-- re-embed (A2). On a fresh DB this inserts nothing.
INSERT INTO embeddings_meta (observation_id, model_id, dim, contextualized, created_at)
SELECT id, 'all-MiniLM-L6-v2', 384, 0, created_at
FROM observations;
