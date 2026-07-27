-- ============================================================
-- ADR-16 (decision 2): drop `user_prompts` + `prompts_fts`.
--
-- The prompts pipeline was write-dead in production — `PromptStore` was
-- instantiated by the service layer and never called outside tests —
-- and `prompts_fts` was inert: declared external-content without
-- `content_rowid` and with no _ai/_ad/_au triggers, so nothing ever
-- populated it. Both are removed together with the Rust `PromptStore`.
-- V001 is NOT edited (already applied to live DBs; refinery tracks
-- checksums) — this migration carries the DROPs instead.
-- ============================================================

-- FTS virtual table first: it references `user_prompts` as external
-- content. Dropping it also removes its FTS5 shadow tables
-- (prompts_fts_data/_idx/_content/_docsize/_config).
DROP TABLE IF EXISTS prompts_fts;

-- Base table; its indexes (idx_prompts_*) go with it.
DROP TABLE IF EXISTS user_prompts;
