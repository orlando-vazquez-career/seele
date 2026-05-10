//! SEELE search — hybrid FTS5 + vec0 search via Reciprocal Rank Fusion.

pub mod engine;
pub mod error;
pub mod rrf;

pub use engine::{SearchEngine, SearchHit, SearchQuery};
pub use error::{Result, SearchError};
pub use rrf::{RrfHit, DEFAULT_K};
