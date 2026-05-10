//! Embedded SQL migrations driven by `refinery`.
//!
//! New migrations go in `crates/seele-storage/src/migrations/` as
//! `V{N}__{description}.sql`. Refinery picks them up at compile time via
//! `embed_migrations!`.

use refinery::embed_migrations;

embed_migrations!("./src/migrations");

use crate::error::Result;

/// Run all pending migrations against `conn`.
pub fn run_pending(conn: &mut rusqlite::Connection) -> Result<()> {
    migrations::runner().run(conn)?;
    Ok(())
}
