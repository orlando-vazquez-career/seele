//! Per-subcommand modules. Each file owns its clap `Args` struct and
//! its `run(...)` entry point — splitting them out keeps `app.rs`
//! focused on the dispatch tree.

pub mod backup;
pub mod delete;
pub mod doctor;
pub mod embedder;
#[cfg(feature = "eval")]
pub mod eval;
pub mod import;
pub mod link;
pub mod list;
pub mod mcp;
pub mod projects;
pub mod restore;
pub mod save;
pub mod search;
pub mod serve;
pub mod setup;
pub mod show;
pub mod stats;
pub mod sync;
#[cfg(feature = "tui")]
pub mod tui;
