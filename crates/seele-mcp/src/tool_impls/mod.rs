//! Tool handler implementations grouped by domain.
//!
//! Each handler is a `fn(&SeeleService, Value) -> Result<Value, ToolError>`
//! and lives in a domain module (`memories`, `sessions`, `relations`,
//! `meta`). The signatures are intentionally uniform so the registry can
//! treat them as plain function pointers.

pub mod memories;
pub mod meta;
pub mod relations;
pub mod sessions;
