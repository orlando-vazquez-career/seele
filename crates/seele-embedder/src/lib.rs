//! SEELE embedder — ONNX-based local embeddings.
//!
//! Default: `sentence-transformers/all-MiniLM-L6-v2` (384-dim,
//! L2-normalized). Auto-downloads from Hugging Face on first construction
//! and caches under `~/.seele/embedder/<model>/` (override with the
//! `SEELE_EMBEDDER_DIR` env var). INT8 quantized weights are preferred by
//! default; full precision is opt-in via [`OnnxConfig::quantized`].
//!
//! [`FakeEmbedder`] is provided for downstream tests that should not depend
//! on a real model.
//!
//! Long-running servers (HTTP, MCP) should call [`init_global`] once and
//! read via [`global`] / [`try_global`] from request handlers; one-shot CLI
//! invocations and unit tests should build their own embedder and pass it
//! explicitly.

pub mod embedder;
pub mod error;
pub mod fake;
pub mod onnx;
pub mod singleton;

pub use embedder::Embedder;
pub use error::{EmbedderError, Result};
pub use fake::FakeEmbedder;
pub use onnx::{resolve_cache_dir, OnnxConfig, OnnxEmbedder};
pub use singleton::{global, init_global, try_global};
