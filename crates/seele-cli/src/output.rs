//! Output formatting: JSON vs human-friendly text.
//!
//! Every command emits via these helpers so `--json` is honored
//! uniformly across the CLI surface. With `--json`, every one-shot
//! command speaks the same envelope contract:
//!
//! - success: `{"ok": true, "data": <payload>, "warnings": [..]}`
//! - error:   `{"ok": false, "error": "<display>", "kind": "<class>"}`
//!
//! Errors go to **stdout** (stderr stays free for human prose), and the
//! exit code stays coupled to `ok` — agents and scripts branch on `ok`
//! without ever parsing stderr. `warnings` carries in-band, machine-
//! readable degradation signals (e.g. `fake-embedder-fallback`) that
//! previously existed only as stderr prose.

use std::sync::Mutex;

use serde::Serialize;

/// Warning codes accumulated during command execution and drained into
/// the next JSON envelope. Process-wide because warnings originate
/// below the command layer (embedder selection happens before any
/// command body runs) and the CLI is one command per process.
static WARNINGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// In-band warning code: ONNX init failed and the CLI silently degraded
/// to the hash-based FakeEmbedder. Consumers should treat vec-search
/// quality as non-semantic until the model downloads.
pub const WARN_FAKE_EMBEDDER_FALLBACK: &str = "fake-embedder-fallback";

#[derive(Serialize)]
struct Envelope<'a, T: Serialize> {
    ok: bool,
    data: &'a T,
    warnings: Vec<String>,
}

#[derive(Serialize)]
struct ErrorEnvelope {
    ok: bool,
    error: String,
    kind: String,
}

/// Record an in-band warning code for the current invocation.
pub fn push_warning(code: impl Into<String>) {
    if let Ok(mut w) = WARNINGS.lock() {
        w.push(code.into());
    }
}

fn drain_warnings() -> Vec<String> {
    WARNINGS
        .lock()
        .map(|mut w| std::mem::take(&mut *w))
        .unwrap_or_default()
}

/// Emit the JSON envelope around `value` when `json` is true, else the
/// `human()` text. Splitting the two representations lets each command
/// craft a readable plain-text form without giving up the machine-
/// readable JSON.
pub fn emit_split<T: Serialize>(
    value: &T,
    human: impl FnOnce() -> String,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        let envelope = Envelope {
            ok: true,
            data: value,
            warnings: drain_warnings(),
        };
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    } else {
        println!("{}", human());
    }
    Ok(())
}

/// Emit the error half of the envelope contract to stdout. The caller
/// still exits non-zero; `ok:false` and the exit code stay coupled.
pub fn emit_error_json(err: &anyhow::Error) {
    let envelope = ErrorEnvelope {
        ok: false,
        error: format!("{err:#}"),
        kind: error_kind(err),
    };
    match serde_json::to_string_pretty(&envelope) {
        Ok(s) => println!("{s}"),
        // Three plain string fields can't realistically fail to
        // serialize; keep a hand-built object so the contract holds
        // even if they somehow do.
        Err(_) => println!(
            "{{\"ok\":false,\"error\":\"internal: error envelope serialization failed\",\"kind\":\"other\"}}"
        ),
    }
}

/// Best-effort machine-readable error class: the enum variant name of
/// the typed root cause when it belongs to a SEELE crate, else `other`.
fn error_kind(err: &anyhow::Error) -> String {
    let root = err.root_cause();
    macro_rules! try_kind {
        ($ty:ty) => {
            if let Some(t) = root.downcast_ref::<$ty>() {
                return variant_token(&format!("{t:?}"));
            }
        };
    }
    try_kind!(seele_http::ApiError);
    try_kind!(seele_storage::StorageError);
    try_kind!(seele_search::error::SearchError);
    try_kind!(seele_setup::SetupError);
    try_kind!(seele_sync::SyncError);
    try_kind!(seele_engram_import::EngramImportError);
    "other".to_string()
}

/// First token of a Debug-formatted enum value — the variant name for
/// unit, tuple and struct variants alike.
fn variant_token(debug: &str) -> String {
    debug
        .split(|c: char| c == '(' || c == '{' || c.is_whitespace())
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("other")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_token_handles_unit_tuple_and_struct_debug_shapes() {
        assert_eq!(variant_token("NotFound"), "NotFound");
        assert_eq!(variant_token("Io(Os { code: 2 })"), "Io");
        assert_eq!(variant_token("NotFound { id: \"x\" }"), "NotFound");
        assert_eq!(variant_token(""), "other");
    }

    #[test]
    fn warnings_drain_into_next_envelope_and_reset() {
        push_warning("fake-embedder-fallback");
        let drained = drain_warnings();
        assert_eq!(drained, vec!["fake-embedder-fallback".to_string()]);
        assert!(drain_warnings().is_empty(), "drain must reset the sink");
    }

    #[test]
    fn error_kind_extracts_variant_from_typed_root_cause() {
        let storage_err = seele_storage::StorageError::NotFound("01ARZ".to_string());
        let chained = anyhow::Error::from(storage_err).context("while showing");
        assert_eq!(error_kind(&chained), "NotFound");

        let plain = anyhow::anyhow!("free-form failure");
        assert_eq!(error_kind(&plain), "other");
    }
}
