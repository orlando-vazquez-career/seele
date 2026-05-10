//! Embedder trait — abstracts the production ONNX path from in-memory fakes
//! used by downstream tests in `seele-search` and `seele-storage`.

use crate::error::Result;

/// A pluggable text embedder. All vectors returned MUST be L2-normalized
/// (norm = 1) and of length `dim()`.
pub trait Embedder: Send + Sync {
    /// Embed a single text. Returns a vector of length `dim()`.
    fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Embed a batch. Default impl iterates `embed`. Override for runtime
    /// efficiency (single ONNX session call vs. N).
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// Dimensionality of returned vectors. For all-MiniLM-L6-v2 this is 384.
    fn dim(&self) -> usize;

    /// Human-readable model identifier, for logging/telemetry.
    fn model_id(&self) -> &str;

    /// Hex-encoded SHA256 of the model weights, when the embedder is backed
    /// by a verified file. Used by upstream tooling to detect model swaps
    /// (re-embedding required). `None` for fakes or unverified models.
    fn expected_sha256(&self) -> Option<&str> {
        None
    }
}
