//! SEELE embedder — ONNX-based local embeddings.
//!
//! Default: `sentence-transformers/all-MiniLM-L6-v2` (384-dim,
//! L2-normalized). Auto-downloads from Hugging Face on first construction.
//! `FakeEmbedder` is provided for downstream tests that should not depend
//! on a real model.

pub mod embedder;
pub mod error;
pub mod fake;
pub mod onnx;

pub use embedder::Embedder;
pub use error::{EmbedderError, Result};
pub use fake::FakeEmbedder;
pub use onnx::{OnnxConfig, OnnxEmbedder};
