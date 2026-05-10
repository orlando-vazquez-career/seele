//! SEELE core — types, errors, IDs, filters shared across all crates.
//!
//! This crate has no dependencies on storage or runtime concerns.
//! It only defines the data shapes and contracts.
//!
//! See ADR-02 (schema-sqlite) and ADR-10 (mapping-mnema-seele) for rationale.

pub mod error;
pub mod filter;
pub mod id;
pub mod link;
pub mod memory;
pub mod metadata;
pub mod relation;
pub mod session;

pub use error::{Result, SeeleError};
pub use filter::{MetadataFilter, ObservationQuery};
pub use id::SeeleId;
pub use link::{link_types, Link};
pub use memory::{Observation, ObservationType, Scope};
pub use metadata::Metadata;
pub use relation::{JudgmentStatus, MemoryRelation, RelationKind};
pub use session::{Session, SessionStatus};
