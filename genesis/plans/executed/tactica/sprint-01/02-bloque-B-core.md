# Sprint-01 Bloque B — `seele-core`

**Tema**: Tipos canónicos, errores, ULID, filtros. Es el bottom del workspace; nadie depende hacia abajo de él.

**Pre-requisitos**: bloque A cerrado (workspace compila).

## Archivos a implementar

### `crates/seele-core/src/lib.rs`

```rust
//! SEELE core — types, errors, IDs, filters shared across all crates.
//!
//! This crate has no dependencies on storage or runtime concerns.
//! It only defines the data shapes and contracts.

pub mod error;
pub mod id;
pub mod memory;
pub mod link;
pub mod relation;
pub mod session;
pub mod filter;
pub mod metadata;

pub use error::{SeeleError, Result};
pub use id::SeeleId;
pub use memory::{Observation, ObservationType, Scope};
pub use session::{Session, SessionStatus};
pub use link::{Link, LinkType};
pub use relation::{MemoryRelation, JudgmentStatus, RelationKind};
pub use filter::{MetadataFilter, ObservationQuery};
pub use metadata::Metadata;
```

### `error.rs`

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SeeleError {
    #[error("storage error: {0}")]
    Storage(String),

    #[error("embedder error: {0}")]
    Embedder(String),

    #[error("search error: {0}")]
    Search(String),

    #[error("mcp error: {0}")]
    Mcp(String),

    #[error("http error: {0}")]
    Http(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, SeeleError>;
```

### `id.rs`

```rust
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::error::{Result, SeeleError};

/// SEELE-wide identifier. Wraps ULID for typed contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SeeleId(Ulid);

impl SeeleId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    pub fn from_ulid(u: Ulid) -> Self {
        Self(u)
    }

    pub fn as_ulid(&self) -> Ulid {
        self.0
    }

    /// First 48 bits of ULID as i64 — used to map ULID PK to vec0 INTEGER rowid.
    /// Collision probability is astronomically low (timestamp + 16 random bits).
    pub fn as_i64(&self) -> i64 {
        let bytes = self.0.to_bytes();
        let mut int_bytes = [0u8; 8];
        int_bytes[..6].copy_from_slice(&bytes[..6]);
        i64::from_be_bytes(int_bytes)
    }

    pub fn timestamp_ms(&self) -> u64 {
        self.0.timestamp_ms()
    }
}

impl Default for SeeleId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SeeleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SeeleId {
    type Err = SeeleError;

    fn from_str(s: &str) -> Result<Self> {
        Ulid::from_str(s)
            .map(Self)
            .map_err(|e| SeeleError::InvalidInput(format!("invalid ULID '{s}': {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ids_unique() {
        let a = SeeleId::new();
        let b = SeeleId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn roundtrip_string() {
        let id = SeeleId::new();
        let s = id.to_string();
        let parsed: SeeleId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn invalid_string_errors() {
        let bad: Result<SeeleId> = "not-a-ulid".parse();
        assert!(bad.is_err());
    }
}
```

### `metadata.rs`

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Metadata is a typed wrapper over `serde_json::Value`.
/// SEELE doesn't enforce schema — consumers (MNEMA, etc) define their own.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Metadata(pub Value);

impl Metadata {
    pub fn new() -> Self {
        Self(Value::Object(serde_json::Map::new()))
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn set(&mut self, key: &str, value: Value) {
        if let Value::Object(map) = &mut self.0 {
            map.insert(key.to_string(), value);
        }
    }

    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl From<Value> for Metadata {
    fn from(v: Value) -> Self {
        Self(v)
    }
}
```

### `memory.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::SeeleId;
use crate::metadata::Metadata;

/// Observation = single memory unit. The core entity of SEELE.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: SeeleId,
    pub session_id: Option<SeeleId>,
    pub r#type: ObservationType,
    pub title: String,
    pub content: String,
    pub tool_name: Option<String>,
    pub project: Option<String>,
    pub scope: Scope,
    pub topic_key: Option<String>,
    pub normalized_hash: Option<String>,
    pub revision_count: u32,
    pub duplicate_count: u32,
    pub last_seen_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub metadata: Metadata,
}

/// Observation type — extensible TEXT field, but with canonical values.
/// First 7 are ENGRAM-inherited (coding agents); last 5 are MNEMA-extended.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationType {
    Decision,
    Architecture,
    Bugfix,
    Pattern,
    Config,
    Discovery,
    Learning,
    Memory,
    Skill,
    AdvisorOutput,
    Review,
    Verdict,
    Other(String),
}

impl ObservationType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Decision => "decision",
            Self::Architecture => "architecture",
            Self::Bugfix => "bugfix",
            Self::Pattern => "pattern",
            Self::Config => "config",
            Self::Discovery => "discovery",
            Self::Learning => "learning",
            Self::Memory => "memory",
            Self::Skill => "skill",
            Self::AdvisorOutput => "advisor_output",
            Self::Review => "review",
            Self::Verdict => "verdict",
            Self::Other(s) => s.as_str(),
        }
    }

    pub fn from_str_relaxed(s: &str) -> Self {
        match s {
            "decision" => Self::Decision,
            "architecture" => Self::Architecture,
            "bugfix" => Self::Bugfix,
            "pattern" => Self::Pattern,
            "config" => Self::Config,
            "discovery" => Self::Discovery,
            "learning" => Self::Learning,
            "memory" => Self::Memory,
            "skill" => Self::Skill,
            "advisor_output" => Self::AdvisorOutput,
            "review" => Self::Review,
            "verdict" => Self::Verdict,
            other => Self::Other(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Project,
    Personal,
}

impl Default for Scope {
    fn default() -> Self {
        Self::Project
    }
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Personal => "personal",
        }
    }
}
```

### `session.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::SeeleId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SeeleId,
    pub project: String,
    pub directory: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub summary: Option<String>,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Active,
    Ended,
    Aborted,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Ended => "ended",
            Self::Aborted => "aborted",
        }
    }
}
```

### `link.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::SeeleId;
use crate::metadata::Metadata;

/// General-purpose link between two observations (graph edge, no judgment lifecycle).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub id: SeeleId,
    pub from_id: SeeleId,
    pub to_id: SeeleId,
    pub link_type: String,
    pub metadata: Metadata,
    pub created_at: DateTime<Utc>,
}

/// Recommended link types — TEXT is free, these are conventions.
pub mod LinkType {
    pub const DERIVES_FROM: &str = "derives_from";
    pub const SUPERSEDES: &str = "supersedes";
    pub const RELATED_TO: &str = "related_to";
    pub const CONTRADICTS: &str = "contradicts";
    pub const EVIDENCE_FOR: &str = "evidence_for";
    pub const PART_OF_VERDICT: &str = "part_of_verdict";
}
```

### `relation.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::SeeleId;

/// Memory relation — for conflict / judgment lifecycle (heredado de ENGRAM).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRelation {
    pub id: SeeleId,
    pub sync_id: String,
    pub source_id: SeeleId,
    pub target_id: SeeleId,
    pub relation: RelationKind,
    pub judgment_status: JudgmentStatus,
    pub reason: Option<String>,
    pub evidence: Option<String>,
    pub confidence: Option<f64>,
    pub marked_by_actor: Option<String>,
    pub marked_by_kind: Option<String>,
    pub marked_by_model: Option<String>,
    pub session_id: Option<SeeleId>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    Supersedes,
    ConflictsWith,
    Scoped,
    Related,
    Compatible,
    NotConflict,
}

impl RelationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Supersedes => "supersedes",
            Self::ConflictsWith => "conflicts_with",
            Self::Scoped => "scoped",
            Self::Related => "related",
            Self::Compatible => "compatible",
            Self::NotConflict => "not_conflict",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JudgmentStatus {
    Pending,
    Judged,
    Orphaned,
    Ignored,
}

impl JudgmentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Judged => "judged",
            Self::Orphaned => "orphaned",
            Self::Ignored => "ignored",
        }
    }
}
```

### `filter.rs`

```rust
use serde::{Deserialize, Serialize};

use crate::memory::Scope;

/// Filter for observation queries. Translates to SQL WHERE clauses in the storage layer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataFilter {
    pub kind: Option<String>,
    pub project: Option<String>,
    pub scope: Option<Scope>,
    pub topic_key: Option<String>,
    pub axiomatic: Option<bool>,
    pub include_deleted: bool,
    pub include_purist: bool,  // default false: excluye context_mode='purist'
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationQuery {
    pub query: Option<String>,
    pub filter: MetadataFilter,
    pub limit: u32,
    pub offset: u32,
}

impl Default for ObservationQuery {
    fn default() -> Self {
        Self {
            query: None,
            filter: MetadataFilter::default(),
            limit: 10,
            offset: 0,
        }
    }
}
```

## Tests del bloque B

- Property tests para `SeeleId::as_i64` (verifica determinismo + uniqueness para 10K samples).
- Roundtrip tests `Observation` JSON serde.
- Roundtrip tests `Session`, `Link`, `MemoryRelation`.
- `ObservationType::from_str_relaxed` cubre los 12 valores conocidos + un valor `Other`.
- `Scope` default = Project.

Test file: `crates/seele-core/tests/types_roundtrip.rs`.

## Criterios de aceptación

1. `cargo build -p seele-core` verde.
2. `cargo test -p seele-core` verde (~10-20 tests).
3. `cargo clippy -p seele-core -- -D warnings` verde.
4. Todos los tipos públicos derivan `Serialize + Deserialize + Debug + Clone`.
5. `SeeleError` sirve como error type unificado para todo el workspace (re-exportado).

## Commit del bloque B

```
git add -A
git commit -m "sprint-01 bloque-B — seele-core (types, errors, IDs, filters)"
git push
```
