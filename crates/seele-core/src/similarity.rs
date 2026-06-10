//! Similarity thresholds shared by near-duplicate detection (Q4),
//! `find_similar` / `seele_compare` suggest mode (Q6) and, in v0.4, the
//! consolidate cycle.
//!
//! Calibration is faithful to GRAIL's `MemoryConfig` (compendium 07):
//! signal thresholds are strict because **false merges are worse than
//! missed merges**; the 0.85 floor belongs to the suggest→confirm cycle
//! (minimum confidence for a caller-confirmed relation), not to the
//! signals themselves. v0.4 (E8, prompt/config registry) moves these to
//! a user-editable config file; until then they live here as the single
//! source of truth so every surface agrees.

/// Minimum Jaro-Winkler similarity between titles for the title signal
/// to fire (GRAIL: `alias_min_name_similarity`-class strictness).
pub const SIGNAL_JW_MIN: f64 = 0.92;

/// Minimum cosine similarity between stored embeddings for the vector
/// signal to fire (GRAIL: `alias_min_embedding_cosine` = 0.93).
pub const SIGNAL_COS_MIN: f64 = 0.93;

/// Confidence floor for relations created through suggest→confirm.
pub const CONFIRM_CONFIDENCE_FLOOR: f64 = 0.85;

/// Convert a vec0 L2 distance over unit-normalized vectors to cosine
/// similarity: `l2² = 2·(1 − cos)` ⇒ `cos = 1 − l2²/2`.
pub fn cosine_from_unit_l2(l2: f64) -> f64 {
    1.0 - (l2 * l2) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_from_unit_l2_roundtrips_known_points() {
        assert!((cosine_from_unit_l2(0.0) - 1.0).abs() < 1e-12);
        // l2 = sqrt(2) → orthogonal.
        assert!(cosine_from_unit_l2(std::f64::consts::SQRT_2).abs() < 1e-12);
        // The Q4 threshold: 0.374… ≈ cos 0.93.
        let l2 = (2.0f64 * (1.0 - 0.93)).sqrt();
        assert!((cosine_from_unit_l2(l2) - 0.93).abs() < 1e-12);
    }
}
