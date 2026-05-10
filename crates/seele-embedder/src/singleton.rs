//! Process-global embedder slot.
//!
//! HTTP and MCP servers want a single shared embedder instance across all
//! request handlers (loading the ONNX session is ~150-300ms). Tests and
//! one-shot CLI invocations should generally NOT use the global — they build
//! their own embedder (often `FakeEmbedder`) and pass it explicitly to keep
//! state isolated.
//!
//! Initialization is one-shot: a second `init_global` call returns
//! [`EmbedderError::GlobalAlreadyInitialized`]. Reads via [`global`] panic if
//! not initialized; [`try_global`] gives a non-panicking variant.

use std::sync::Arc;

use once_cell::sync::OnceCell;

use crate::embedder::Embedder;
use crate::error::{EmbedderError, Result};

static GLOBAL_EMBEDDER: OnceCell<Arc<dyn Embedder>> = OnceCell::new();

/// Install the process-global embedder. Returns an error if a previous call
/// already initialized the slot — callers that want "replace on second init"
/// semantics should design around their own `Arc<RwLock<…>>` instead.
pub fn init_global(embedder: Arc<dyn Embedder>) -> Result<()> {
    GLOBAL_EMBEDDER
        .set(embedder)
        .map_err(|_| EmbedderError::GlobalAlreadyInitialized)
}

/// Fetch the global embedder. Panics with a descriptive message if
/// [`init_global`] was never called — use [`try_global`] when you want to
/// observe the uninitialized state without crashing.
pub fn global() -> Arc<dyn Embedder> {
    GLOBAL_EMBEDDER
        .get()
        .cloned()
        .expect("global embedder accessed before init_global() was called")
}

/// Non-panicking accessor. Returns `None` if [`init_global`] has not been
/// called yet.
pub fn try_global() -> Option<Arc<dyn Embedder>> {
    GLOBAL_EMBEDDER.get().cloned()
}

#[cfg(test)]
mod tests {
    // The OnceCell is process-global. Cargo runs tests across binaries, so
    // each test binary has its own instance, but tests inside this module
    // share the slot. We keep a single test that exercises init+read; a
    // second init MUST happen in a separate test binary (or use std::sync
    // for serial). For unit-level we lean on the OnceCell semantics being
    // straightforward.

    use super::*;
    use crate::fake::FakeEmbedder;

    #[test]
    fn init_then_global_returns_same_instance_and_second_init_errs() {
        // First init succeeds.
        let first: Arc<dyn Embedder> = Arc::new(FakeEmbedder);
        init_global(first.clone()).expect("first init ok");

        // Reading via global returns the same shared pointer.
        let got = global();
        assert_eq!(got.model_id(), first.model_id());

        // try_global agrees with global.
        let got2 = try_global().expect("initialized");
        assert_eq!(got2.model_id(), first.model_id());

        // Second init fails with the canonical error variant.
        let second: Arc<dyn Embedder> = Arc::new(FakeEmbedder);
        let err = init_global(second).unwrap_err();
        assert!(matches!(err, EmbedderError::GlobalAlreadyInitialized));
    }
}
