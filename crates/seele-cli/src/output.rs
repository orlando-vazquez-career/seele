//! Output formatting: JSON vs human-friendly text.
//!
//! Every command emits via these helpers so `--json` is honored
//! uniformly across the CLI surface.

use serde::Serialize;

/// Emit the JSON-serialized `value` when `json` is true, else the
/// `human()` text. Splitting the two representations lets each command
/// craft a readable plain-text form without giving up the machine-
/// readable JSON.
pub fn emit_split<T: Serialize>(
    value: &T,
    human: impl FnOnce() -> String,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{}", human());
    }
    Ok(())
}

/// Print a status / confirmation line. Suppressed when `--json` is on
/// (the JSON payload is the only stdout output).
pub fn status(line: &str, json: bool) {
    if !json {
        println!("{line}");
    }
}
