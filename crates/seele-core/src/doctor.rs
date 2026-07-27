//! FTS5 index health heuristic (T-11) — the pure half of `seele doctor`'s
//! FTS check. The CLI executes the SQL below against a pooled connection;
//! this module owns the SQL surface, the threshold, and the recommendation
//! decision so they stay testable without a database.
//!
//! Honesty note: the segment estimate reads the FTS5 shadow table
//! `<name>_data`, whose rowid layout SQLite documents as internal and
//! version-specific. It is exact for the SQLite bundled with this
//! workspace (locked by e2e tests in seele-cli) and, if a future SQLite
//! changed the packing, it would degrade to a wrong *recommendation* —
//! never to wrong data: FTS5 'optimize' is always safe to run.

/// Segment count above which doctor recommends running FTS5 'optimize'.
///
/// FTS5 writes one on-disk b-tree segment per committed write transaction
/// and merges them incrementally (default `automerge=4`), so a healthy
/// index under write churn oscillates in the low single digits; a count
/// past this threshold means many commits landed since the last full
/// merge. Empirically: a freshly 'optimize'd index has exactly 1 segment.
pub const FTS_OPTIMIZE_SEGMENT_THRESHOLD: u64 = 8;

/// SQL: estimated number of on-disk FTS5 segments in `observations_fts`.
///
/// In the `<name>_data` shadow table, rowid 1 is the averages record and
/// rowid 10 the structure record — bookkeeping, not segments. Everything
/// above is segment b-tree storage with the segment id packed into the
/// high rowid bits, so `DISTINCT id >> 32` approximates the segment count
/// (one per b-tree page region; see the module-level honesty note).
pub const FTS_SEGMENTS_SQL: &str =
    "SELECT count(DISTINCT id >> 32) FROM observations_fts_data WHERE id > 10";

/// SQL: documents in the FTS index. This is an index-size signal, not an
/// application-level count: it includes soft-deleted observations until
/// their pending 'delete' entries are merged away.
pub const FTS_DOCS_SQL: &str = "SELECT count(*) FROM observations_fts";

/// SQL: FTS5 'optimize' — merge all index segments into a single b-tree.
/// Idempotent and always safe (runs as one transaction); the remediation
/// behind `seele doctor --fix`.
pub const FTS_OPTIMIZE_SQL: &str =
    "INSERT INTO observations_fts(observations_fts) VALUES('optimize')";

/// FTS health snapshot plus the recommendation derived from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct FtsHealth {
    /// Documents in the index (see [`FTS_DOCS_SQL`]).
    pub docs: u64,
    /// Estimated on-disk segment count (see [`FTS_SEGMENTS_SQL`]).
    pub segments: u64,
}

impl FtsHealth {
    /// True when the segment count is past
    /// [`FTS_OPTIMIZE_SEGMENT_THRESHOLD`] — i.e. enough write transactions
    /// landed since the last full merge that an 'optimize' pass is worth
    /// its cost.
    pub fn optimize_recommended(&self) -> bool {
        self.segments > FTS_OPTIMIZE_SEGMENT_THRESHOLD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optimize_recommended_only_above_threshold() {
        let at = FtsHealth {
            docs: 100,
            segments: FTS_OPTIMIZE_SEGMENT_THRESHOLD,
        };
        assert!(!at.optimize_recommended(), "threshold itself is healthy");
        let above = FtsHealth {
            docs: 100,
            segments: FTS_OPTIMIZE_SEGMENT_THRESHOLD + 1,
        };
        assert!(above.optimize_recommended());
    }

    #[test]
    fn fresh_and_optimized_indexes_are_healthy() {
        // A fresh DB has no segments; a just-optimized one has exactly 1.
        for segments in [0, 1] {
            let h = FtsHealth { docs: 0, segments };
            assert!(!h.optimize_recommended());
        }
    }

    #[test]
    fn sql_targets_the_canonical_fts_table() {
        // The three statements must stay pinned to the schema's FTS table
        // name (V001__initial_schema.sql) — a rename there must break a
        // test here, not production silently.
        for sql in [FTS_SEGMENTS_SQL, FTS_DOCS_SQL, FTS_OPTIMIZE_SQL] {
            assert!(sql.contains("observations_fts"), "{sql}");
        }
        assert!(FTS_OPTIMIZE_SQL.contains("VALUES('optimize')"));
    }
}
