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
}
